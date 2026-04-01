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

use aft::config::Config;
use aft::context::AppContext;
use aft::parser::TreeSitterProvider;
use aft::protocol::{RawRequest, Response};

use serde_json::{json, Value};

/// Commands exposed via MCP (read-only subset of aft).
const ALLOWED_COMMANDS: &[&str] = &[
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
    "ping",
    "version",
];

fn main() {
    // All logging to stderr — stdout is MCP protocol only
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .format(|buf, record| {
            use std::io::Write;
            writeln!(buf, "[aft-mcp] {}", record.args())
        })
        .init();

    log::info!("started, pid {}", std::process::id());

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
            "description": "Code intelligence: outline (file structure), zoom (symbol + call annotations), callers (who calls this), call_tree (what this calls), impact (what breaks), trace_to (execution path from entry points), trace_data (data flow), ast_search (structural pattern search), read (file content).",
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
                    "lang": { "type": "string", "description": "Language for ast_search (typescript, javascript, python, rust, go)" },
                    "startLine": { "type": "number", "description": "Start line (1-based) for read" },
                    "endLine": { "type": "number", "description": "End line (1-based) for read" },
                    "expression": { "type": "string", "description": "Expression for trace_data" }
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

    // Security: only allow read-only commands
    if !ALLOWED_COMMANDS.contains(&command) {
        return Err(McpError {
            code: -32602,
            message: format!("Command '{command}' not allowed. Allowed: {}", ALLOWED_COMMANDS.join(", ")),
        });
    }

    // Auto-configure project root on first call (from CWD)
    static CONFIGURED: AtomicBool = AtomicBool::new(false);
    if !CONFIGURED.load(Ordering::Relaxed) {
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

    // Build aft RawRequest
    let raw_request = RawRequest {
        id: "mcp".to_string(),
        command: command.to_string(),
        lsp_hints: None,
        params: aft_params,
    };

    // Drain file watcher events before dispatch
    drain_watcher_events(ctx);

    // Dispatch to aft engine
    let response = dispatch(raw_request, ctx);

    // Wrap aft response in MCP format
    if response.success {
        let text = serde_json::to_string_pretty(&response.data)
            .unwrap_or_else(|_| format!("{}", response.data));
        Ok(json!({
            "content": [{ "type": "text", "text": text }]
        }))
    } else {
        let msg = response.data.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        Ok(json!({
            "isError": true,
            "content": [{ "type": "text", "text": msg }]
        }))
    }
}

// ---------------------------------------------------------------------------
// aft dispatch (read-only subset from original main.rs)
// ---------------------------------------------------------------------------

fn dispatch(req: RawRequest, ctx: &AppContext) -> Response {
    match req.command.as_str() {
        "ping" => Response::success(&req.id, json!({ "command": "pong" })),
        "version" => Response::success(&req.id, json!({ "version": env!("CARGO_PKG_VERSION") })),
        "outline" => aft::commands::outline::handle_outline(&req, ctx),
        "zoom" => aft::commands::zoom::handle_zoom(&req, ctx),
        "read" => aft::commands::read::handle_read(&req, ctx),
        "callers" => aft::commands::callers::handle_callers(&req, ctx),
        "call_tree" => aft::commands::call_tree::handle_call_tree(&req, ctx),
        "impact" => aft::commands::impact::handle_impact(&req, ctx),
        "trace_to" => aft::commands::trace_to::handle_trace_to(&req, ctx),
        "trace_data" => aft::commands::trace_data::handle_trace_data(&req, ctx),
        "ast_search" => aft::commands::ast_search::handle_ast_search(&req, ctx),
        "configure" => aft::commands::configure::handle_configure(&req, ctx),
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

const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "py", "rs", "go", "css", "html", "htm", "cls", "trigger", "apex"];

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
