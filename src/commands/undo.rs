use std::path::Path;

use crate::context::AppContext;
use crate::protocol::{RawRequest, Response};

/// Handle the `undo` command: restore the most recent backup for a file.
///
/// Params: `file` (string, required) — path to the file to undo.
/// Returns: `{ path, backup_id }` on success, or `no_undo_history` error.
pub fn handle_undo(req: &RawRequest, ctx: &AppContext) -> Response {
    let file = match req.params.get("file").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => {
            return Response::error(
                &req.id,
                "invalid_request",
                "undo: missing required param 'file'",
            );
        }
    };

    let path = Path::new(file);
    let mut backup = ctx.backup().borrow_mut();

    match backup.restore_latest(path) {
        Ok(entry) => Response::success(
            &req.id,
            serde_json::json!({
                "path": file,
                "backup_id": entry.backup_id,
            }),
        ),
        Err(e) => Response::error(&req.id, e.code(), e.to_string()),
    }
}
