use crate::context::AppContext;
use crate::protocol::{RawRequest, Response};

/// Handle the `restore_checkpoint` command: restore files from a named checkpoint.
///
/// Params: `name` (string, required) — checkpoint name to restore.
/// Returns: `{ name, file_count, created_at }` on success, or `checkpoint_not_found` error.
pub fn handle_restore_checkpoint(req: &RawRequest, ctx: &AppContext) -> Response {
    let name = match req.params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            return Response::error(
                &req.id,
                "invalid_request",
                "restore_checkpoint: missing required param 'name'",
            );
        }
    };

    let checkpoint_store = ctx.checkpoint().borrow();

    match checkpoint_store.restore(name) {
        Ok(info) => Response::success(
            &req.id,
            serde_json::json!({
                "name": info.name,
                "file_count": info.file_count,
                "created_at": info.created_at,
            }),
        ),
        Err(e) => Response::error(&req.id, e.code(), e.to_string()),
    }
}
