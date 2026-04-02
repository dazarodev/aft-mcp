//! MCP JSON-RPC 2.0 server wrapping aft's code intelligence engine.
//!
//! Unified single-tool mode: one MCP tool "aft" with a "command" parameter.
//! Read-only commands only — write operations handled by Claude Code's own tools.
//!
//! Protocol: newline-delimited JSON-RPC 2.0 over stdin/stdout (MCP stdio transport).
//! All logging goes to stderr — stdout is reserved for the protocol.

use std::collections::HashSet;
use std::io::{self, BufRead, BufWriter, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use aft::config::{AftConfig, Config};
use aft::context::AppContext;
use aft::lang::LangRegistry;
use aft::parser::{init_lang_registry, init_lifecycle_config, TreeSitterProvider};
use aft::protocol::{RawRequest, Response};

use serde_json::{json, Value};

/// Commands exposed via MCP.
/// Read-only: outline, zoom, callers, call_tree, impact, trace_to, trace_data, ast_search, read
/// Write: ast_replace, add_import, edit_match, edit_symbol, write, delete_file, batch, transaction,
///        checkpoint, restore_checkpoint, undo, add_member, add_decorator, add_derive, add_struct_tags,
///        move_file, move_symbol, extract_function, inline_symbol, wrap_try_catch, organize_imports,
///        remove_import, lsp_* (LSP protocol commands)
const ALLOWED_COMMANDS: &[&str] = &[
    // System commands
    "ping",
    "version",
    "echo",
    // Read-only commands
    "outline",
    "zoom",
    "callers",
    "call_tree",
    "impact",
    "trace_to",
    "trace_data",
    "ast_search",
    "read",
    "configure",
    // Write commands
    "ast_replace",
    "add_import",
    "edit_match",
    "edit_symbol",
    "write",
    "delete_file",
    "batch",
    "transaction",
    "checkpoint",
    "snapshot",
    "restore_checkpoint",
    "undo",
    "list_checkpoints",
    "edit_history",
    "add_member",
    "add_decorator",
    "add_derive",
    "add_struct_tags",
    "move_file",
    "move_symbol",
    "extract_function",
    "inline_symbol",
    "wrap_try_catch",
    "organize_imports",
    "remove_import",
    // LSP commands
    "lsp_goto_definition",
    "lsp_find_references",
    "lsp_hover",
    "lsp_rename",
    "lsp_prepare_rename",
    "lsp_diagnostics",
];

fn main() {
    // All logging to stderr — stdout is MCP protocol only
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .format(|buf, record| {
            use std::io::Write;
            writeln!(buf, "[aft] {}", record.args())
        })
        .init();

    log::info!("started, pid {}", std::process::id());

    // Load project-level config (aft.toml) and initialize language registry
    let aft_config = AftConfig::load();
    let mut registry = LangRegistry::new();
    if let Some(ref active) = aft_config.languages {
        registry.retain(active);
    }
    init_lang_registry(registry);
    init_lifecycle_config(aft_config.entry_points.lifecycle.clone());

    let ctx = AppContext::new(Box::new(TreeSitterProvider::new()), Config::default());

    let stdin = io::stdin();
    let reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                log::error!("stdin read error: {}", e);
                break;
            }
        };

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&trimmed) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                write_response(&mut writer, &err);
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Notifications (no id) — no response needed
        if method == "notifications/initialized" {
            continue;
        }

        let response = handle_method(method, &request, &ctx);

        let id = id.unwrap_or(Value::Null);
        let mcp_response = match response {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(e) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": e.code, "message": e.message }
            }),
        };

        write_response(&mut writer, &mcp_response);
    }

    ctx.lsp().shutdown_all();
    log::info!("stdin closed, shutting down");
}

// ---------------------------------------------------------------------------
// MCP method handlers
// ---------------------------------------------------------------------------

struct McpError {
    code: i32,
    message: String,
}

fn handle_method(method: &str, request: &Value, ctx: &AppContext) -> Result<Value, McpError> {
    match method {
        "initialize" => handle_initialize(),
        "tools/list" => handle_tools_list(),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            handle_tools_call(&params, ctx)
        }
        _ => Err(McpError {
            code: -32601,
            message: format!("Method not found: {method}"),
        }),
    }
}

fn handle_initialize() -> Result<Value, McpError> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "aft-mcp",
            "version": "0.1.0"
        }
    }))
}

fn handle_tools_list() -> Result<Value, McpError> {
    Ok(json!({
        "tools": [{
            "name": "aft",
            "description": "Code intelligence and navigation. Commands: outline, zoom, callers, call_tree, impact, trace_to, trace_data, ast_search, read. LSP: lsp_hover, lsp_find_references, lsp_goto_definition, lsp_diagnostics, lsp_rename.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Operation to perform",
                        "enum": ALLOWED_COMMANDS
                    },
                    "file": { "type": "string", "description": "Absolute file path" },
                    "symbol": { "type": "string", "description": "Symbol name" },
                    "symbols": {
                        "type": "array", "items": { "type": "string" },
                        "description": "Multiple symbol names (for zoom)"
                    },
                    "directory": { "type": "string", "description": "Directory path (for outline)" },
                    "files": {
                        "type": "array", "items": { "type": "string" },
                        "description": "Multiple file paths (for outline)"
                    },
                    "depth": { "type": "number", "description": "Traversal depth (default 5)" },
                    "pattern": { "type": "string", "description": "AST pattern for ast_search" },
                    "lang": { "type": "string", "description": "Language for ast_search (apex, c, cpp, csharp, go, java, javascript, php, python, ruby, rust, tsx, typescript)" },
                    "startLine": { "type": "number", "description": "Start line (1-based) for read" },
                    "endLine": { "type": "number", "description": "End line (1-based) for read" },
                    "limit": { "type": "number", "description": "Max lines to return for read (default: 2000)" },
                    "expression": { "type": "string", "description": "Expression to track for trace_data (requires file + symbol)" },
                    "line": { "type": "number", "description": "Cursor line (1-based) for lsp_hover, lsp_find_references, lsp_goto_definition" },
                    "character": { "type": "number", "description": "Cursor column (0-based) for lsp_hover, lsp_find_references, lsp_goto_definition" },
                    "include_declaration": { "type": "boolean", "description": "Include declaration in lsp_find_references results (default: true)" },
                    "paths": {
                        "type": "array", "items": { "type": "string" },
                        "description": "Search paths for ast_search (default: project root)"
                    },
                    "globs": {
                        "type": "array", "items": { "type": "string" },
                        "description": "Include/exclude glob filters for ast_search (prefix ! to exclude)"
                    },
                    "context": { "type": "number", "description": "Lines of context around ast_search matches" },
                    "newName": { "type": "string", "description": "New name for lsp_rename" }
                },
                "required": ["command"]
            }
        }]
    }))
}

fn handle_tools_call(params: &Value, ctx: &AppContext) -> Result<Value, McpError> {
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let command = arguments
        .get("command")
        .and_then(|c| c.as_str())
        .ok_or_else(|| McpError {
            code: -32602,
            message: "Missing 'command' in arguments".into(),
        })?;

    // Note: We don't validate against ALLOWED_COMMANDS here anymore.
    // The dispatch function will handle unknown commands by returning
    // a Response::error with "unknown_command" code. This allows tests
    // and clients to get proper error messages instead of MCP-level rejections.

    // Auto-configure project root on first call (from CWD)
    // Can be disabled with AFT_NO_AUTO_CONFIGURE=1 for tests that expect "not_configured" error
    static CONFIGURED: AtomicBool = AtomicBool::new(false);
    let auto_configure_disabled = std::env::var("AFT_NO_AUTO_CONFIGURE").is_ok();
    if !CONFIGURED.load(Ordering::Relaxed) && !auto_configure_disabled {
        if let Ok(cwd) = std::env::current_dir() {
            let project_root = cwd.to_string_lossy().to_string();
            let configure_req = RawRequest {
                id: "auto-configure".to_string(),
                command: "configure".to_string(),
                lsp_hints: None,
                params: json!({ "project_root": project_root }),
            };
            let res = dispatch(configure_req, ctx);
            if res.success {
                CONFIGURED.store(true, Ordering::Relaxed);
                log::info!("auto-configured project root from cwd: {}", project_root);
            } else {
                log::error!("auto-configure failed: {:?}", res.data);
            }
        }
    }

    // aft uses "file" directly — no renaming needed, just remove "command"
    let mut aft_params = arguments.clone();
    if let Some(obj) = aft_params.as_object_mut() {
        obj.remove("command");
    }

    // Directory outline: if "directory" param given, walk it and convert to "files" array
    if command == "outline" {
        if let Some(dir) = aft_params.get("directory").and_then(|v| v.as_str()).map(String::from) {
            let dir_path = std::path::Path::new(&dir);
            if dir_path.is_dir() {
                let mut files = Vec::new();
                collect_source_files(dir_path, &mut files);
                files.sort();
                if let Some(obj) = aft_params.as_object_mut() {
                    obj.remove("directory");
                    obj.insert("files".into(), json!(files));
                }
            }
        }
    }

    // For navigate commands, set the "op" field
    match command {
        "callers" | "call_tree" | "impact" | "trace_to" | "trace_data" => {
            if let Some(obj) = aft_params.as_object_mut() {
                obj.insert("op".into(), json!(command));
            }
        }
        _ => {}
    }

    // Extract lsp_hints if present and remove from params
    let mut lsp_hints = None;
    if let Some(obj) = aft_params.as_object_mut() {
        lsp_hints = obj.remove("lsp_hints");
    }

    // Build aft RawRequest
    let raw_request = RawRequest {
        id: "mcp".to_string(),
        command: command.to_string(),
        lsp_hints,
        params: aft_params,
    };

    // Drain file watcher events before dispatch
    drain_watcher_events(ctx);

    // Dispatch to aft engine
    let response = dispatch(raw_request, ctx);

    // Wrap aft response in MCP format
    let text = serde_json::to_string_pretty(&response.data)
        .unwrap_or_else(|_| format!("{}", response.data));
    Ok(json!({
        "isError": !response.success,
        "content": [{ "type": "text", "text": text }]
    }))
}

// ---------------------------------------------------------------------------
// aft dispatch (read-only subset from original main.rs)
// ---------------------------------------------------------------------------

fn dispatch(req: RawRequest, ctx: &AppContext) -> Response {
    match req.command.as_str() {
        // System commands
        "ping" => Response::success(&req.id, json!({ "command": "pong" })),
        "version" => Response::success(&req.id, json!({ "version": env!("CARGO_PKG_VERSION") })),
        "echo" => {
            // Echo command: returns the message field as-is
            let message = req.params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Response::success(&req.id, json!({ "message": message }))
        }

        // Read-only commands
        "outline" => aft::commands::outline::handle_outline(&req, ctx),
        "zoom" => aft::commands::zoom::handle_zoom(&req, ctx),
        "read" => aft::commands::read::handle_read(&req, ctx),
        "callers" => aft::commands::callers::handle_callers(&req, ctx),
        "call_tree" => aft::commands::call_tree::handle_call_tree(&req, ctx),
        "impact" => aft::commands::impact::handle_impact(&req, ctx),
        "trace_to" => aft::commands::trace_to::handle_trace_to(&req, ctx),
        "trace_data" => aft::commands::trace_data::handle_trace_data(&req, ctx),
        "ast_search" => aft::commands::ast_search::handle_ast_search(&req, ctx),

        // Configuration
        "configure" => aft::commands::configure::handle_configure(&req, ctx),

        // Write commands - AST-based
        "ast_replace" => aft::commands::ast_replace::handle_ast_replace(&req, ctx),

        // Write commands - Match-based
        "edit_match" => aft::commands::edit_match::handle_edit_match(&req, ctx),
        "edit_symbol" => aft::commands::edit_symbol::handle_edit_symbol(&req, ctx),

        // Write commands - Import management
        "add_import" => aft::commands::add_import::handle_add_import(&req, ctx),
        "remove_import" => aft::commands::remove_import::handle_remove_import(&req, ctx),
        "organize_imports" => aft::commands::organize_imports::handle_organize_imports(&req, ctx),

        // Write commands - File operations
        "write" => aft::commands::write::handle_write(&req, ctx),
        "delete_file" => aft::commands::delete_file::handle_delete_file(&req, ctx),
        "move_file" => aft::commands::move_file::handle_move_file(&req, ctx),

        // Write commands - Batch operations
        "batch" => aft::commands::batch::handle_batch(&req, ctx),
        "transaction" => aft::commands::transaction::handle_transaction(&req, ctx),

        // Write commands - Safety
        "checkpoint" => aft::commands::checkpoint::handle_checkpoint(&req, ctx),
        "snapshot" => aft::commands::snapshot::handle_snapshot(&req, ctx),
        "restore_checkpoint" => aft::commands::restore_checkpoint::handle_restore_checkpoint(&req, ctx),
        "list_checkpoints" => aft::commands::list_checkpoints::handle_list_checkpoints(&req, ctx),
        "undo" => aft::commands::undo::handle_undo(&req, ctx),
        "edit_history" => aft::commands::edit_history::handle_edit_history(&req, ctx),

        // Write commands - Structure
        "add_member" => aft::commands::add_member::handle_add_member(&req, ctx),
        "add_decorator" => aft::commands::add_decorator::handle_add_decorator(&req, ctx),
        "add_derive" => aft::commands::add_derive::handle_add_derive(&req, ctx),
        "add_struct_tags" => aft::commands::add_struct_tags::handle_add_struct_tags(&req, ctx),

        // Write commands - Refactoring
        "move_symbol" => aft::commands::move_symbol::handle_move_symbol(&req, ctx),
        "extract_function" => aft::commands::extract_function::handle_extract_function(&req, ctx),
        "inline_symbol" => aft::commands::inline_symbol::handle_inline_symbol(&req, ctx),
        "wrap_try_catch" => aft::commands::wrap_try_catch::handle_wrap_try_catch(&req, ctx),

        // LSP commands
        "lsp_goto_definition" => aft::commands::lsp_goto_definition::handle_lsp_goto_definition(&req, ctx),
        "lsp_find_references" => aft::commands::lsp_find_references::handle_lsp_find_references(&req, ctx),
        "lsp_hover" => aft::commands::lsp_hover::handle_lsp_hover(&req, ctx),
        "lsp_rename" => aft::commands::lsp_rename::handle_lsp_rename(&req, ctx),
        "lsp_prepare_rename" => aft::commands::lsp_prepare_rename::handle_lsp_prepare_rename(&req, ctx),
        "lsp_diagnostics" => aft::commands::lsp_diagnostics::handle_lsp_diagnostics(&req, ctx),

        _ => Response::error(
            &req.id,
            "unknown_command",
            format!("unknown command: {}", req.command),
        ),
    }
}

// ---------------------------------------------------------------------------
// File watcher (from original main.rs)
// ---------------------------------------------------------------------------

const SOURCE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "py", "rs", "go", "css", "html", "htm",
    "cls", "trigger", "apex", "java", "rb", "c", "h", "cpp", "cc",
    "cxx", "hpp", "cs", "php",
];

fn drain_watcher_events(ctx: &AppContext) {
    let changed: HashSet<std::path::PathBuf> = {
        let rx_ref = ctx.watcher_rx().borrow();
        let rx = match rx_ref.as_ref() {
            Some(rx) => rx,
            None => return,
        };
        let mut paths = HashSet::new();
        while let Ok(event_result) = rx.try_recv() {
            if let Ok(event) = event_result {
                for path in event.paths {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if SOURCE_EXTENSIONS.contains(&ext) {
                            paths.insert(path);
                        }
                    }
                }
            }
        }
        paths
    };

    if changed.is_empty() {
        return;
    }

    let mut graph_ref = ctx.callgraph().borrow_mut();
    if let Some(graph) = graph_ref.as_mut() {
        for path in &changed {
            graph.invalidate_file(path);
        }
    }

    log::info!("invalidated {} files", changed.len());
}

// ---------------------------------------------------------------------------

fn write_response(writer: &mut BufWriter<io::StdoutLock>, response: &Value) {
    if let Err(e) = (|| -> io::Result<()> {
        serde_json::to_writer(&mut *writer, response)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    })() {
        log::error!("stdout write error: {}", e);
    }
}

/// Recursively collect source files from a directory (respects common ignore patterns).
fn collect_source_files(dir: &std::path::Path, out: &mut Vec<String>) {
    const SOURCE_EXTS: &[&str] = &[
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "md", "mdx",
        "css", "html", "htm", "cls", "trigger", "apex",
        "java", "rb", "c", "h", "cpp", "cc", "cxx", "hpp", "cs", "php",
    ];
    const SKIP_DIRS: &[&str] = &[
        "node_modules", ".next", ".git", ".claude", "target", "__pycache__",
        "dist", "build", ".turbo",
    ];

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !SKIP_DIRS.contains(&name) && !name.starts_with('.') {
                collect_source_files(&path, out);
            }
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SOURCE_EXTS.contains(&ext) {
                    out.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
}
