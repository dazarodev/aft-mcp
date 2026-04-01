use std::path::PathBuf;
use std::sync::mpsc;

use notify::{RecursiveMode, Watcher};

use crate::callgraph::CallGraph;
use crate::context::AppContext;
use crate::protocol::{RawRequest, Response};

/// Handle a `configure` request.
///
/// Expects `project_root` (string, required) — absolute path to the project root.
/// Sets the project root on `Config`, initializes the `CallGraph` with that root,
/// spawns a file watcher for live invalidation, and returns success with the
/// configured path.
///
/// Stderr log: `[aft] project root set: <path>`
/// Stderr log: `[aft] watcher started: <path>`
pub fn handle_configure(req: &RawRequest, ctx: &AppContext) -> Response {
    let root = match req.params.get("project_root").and_then(|v| v.as_str()) {
        Some(r) => r,
        None => {
            return Response::error(
                &req.id,
                "invalid_request",
                "configure: missing required param 'project_root'",
            );
        }
    };

    let root_path = PathBuf::from(root);
    if !root_path.is_dir() {
        return Response::error(
            &req.id,
            "invalid_request",
            format!("configure: project_root is not a directory: {}", root),
        );
    }

    // Set project root on config
    ctx.config_mut().project_root = Some(root_path.clone());

    // Optional feature flags from plugin config
    // Optional feature flags from plugin config
    if let Some(v) = req.params.get("format_on_edit").and_then(|v| v.as_bool()) {
        ctx.config_mut().format_on_edit = v;
    }
    if let Some(v) = req.params.get("validate_on_edit").and_then(|v| v.as_str()) {
        ctx.config_mut().validate_on_edit = Some(v.to_string());
    }
    // Per-language formatter overrides: { "typescript": "biome", "python": "ruff" }
    if let Some(v) = req.params.get("formatter").and_then(|v| v.as_object()) {
        for (lang, tool) in v {
            if let Some(tool_str) = tool.as_str() {
                ctx.config_mut()
                    .formatter
                    .insert(lang.clone(), tool_str.to_string());
            }
        }
    }
    // Restrict file operations to project root (default: false)
    if let Some(v) = req
        .params
        .get("restrict_to_project_root")
        .and_then(|v| v.as_bool())
    {
        ctx.config_mut().restrict_to_project_root = v;
    }
    // Per-language checker overrides: { "typescript": "tsc", "python": "pyright" }
    if let Some(v) = req.params.get("checker").and_then(|v| v.as_object()) {
        for (lang, tool) in v {
            if let Some(tool_str) = tool.as_str() {
                ctx.config_mut()
                    .checker
                    .insert(lang.clone(), tool_str.to_string());
            }
        }
    }
    // Initialize call graph with the project root
    let graph = CallGraph::new(root_path.clone());
    *ctx.callgraph().borrow_mut() = Some(graph);

    // Drop old watcher/receiver before creating new ones (re-configure)
    *ctx.watcher().borrow_mut() = None;
    *ctx.watcher_rx().borrow_mut() = None;

    // Spawn file watcher for live invalidation
    let (tx, rx) = mpsc::channel();
    match notify::recommended_watcher(tx) {
        Ok(mut w) => {
            if let Err(e) = w.watch(&root_path, RecursiveMode::Recursive) {
                log::debug!(
                    "[aft] watcher watch error: {} — callers will work with stale data",
                    e
                );
            } else {
                log::info!("watcher started: {}", root_path.display());
            }
            *ctx.watcher().borrow_mut() = Some(w);
            *ctx.watcher_rx().borrow_mut() = Some(rx);
        }
        Err(e) => {
            log::debug!(
                "[aft] watcher init failed: {} — callers will work with stale data",
                e
            );
        }
    }

    log::info!("project root set: {}", root_path.display());

    Response::success(
        &req.id,
        serde_json::json!({ "project_root": root_path.display().to_string() }),
    )
}
