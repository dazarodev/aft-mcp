use std::path::Path;

use crate::context::AppContext;
use crate::protocol::{RawRequest, Response};

/// Handle a `callers` request.
///
/// Expects:
/// - `file` (string, required) — path to the source file containing the target symbol
/// - `symbol` (string, required) — name of the symbol to find callers for
/// - `depth` (number, optional, default 1) — recursive depth (1 = direct callers only)
///
/// Returns callers grouped by file with fields: `symbol`, `file`,
/// `callers` (array of `{ file, callers: [{ symbol, line }] }`),
/// `total_callers`, `scanned_files`.
///
/// Returns error if:
/// - required params missing
/// - call graph not initialized (configure not called)
/// - symbol not found in the file
pub fn handle_callers(req: &RawRequest, ctx: &AppContext) -> Response {
    let file = match req.params.get("file").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => {
            return Response::error(
                &req.id,
                "invalid_request",
                "callers: missing required param 'file'",
            );
        }
    };

    let symbol = match req.params.get("symbol").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return Response::error(
                &req.id,
                "invalid_request",
                "callers: missing required param 'symbol'",
            );
        }
    };

    let depth = req
        .params
        .get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .min(100) as usize;

    let mut cg_ref = ctx.callgraph().borrow_mut();
    let graph = match cg_ref.as_mut() {
        Some(g) => g,
        None => {
            return Response::error(
                &req.id,
                "not_configured",
                "callers: project not configured — send 'configure' first",
            );
        }
    };

    let file_path = Path::new(file);

    // Build file data first to check if the symbol exists
    match graph.build_file(file_path) {
        Ok(data) => {
            let has_symbol = data.calls_by_symbol.contains_key(symbol)
                || data.exported_symbols.contains(&symbol.to_string());
            if !has_symbol {
                return Response::error(
                    &req.id,
                    "symbol_not_found",
                    format!("callers: symbol '{}' not found in {}", symbol, file),
                );
            }
        }
        Err(e) => {
            return Response::error(&req.id, e.code(), e.to_string());
        }
    }

    match graph.callers_of(file_path, symbol, depth) {
        Ok(result) => {
            let result_json = serde_json::to_value(&result).unwrap_or_default();
            Response::success(&req.id, result_json)
        }
        Err(e) => Response::error(&req.id, e.code(), e.to_string()),
    }
}
