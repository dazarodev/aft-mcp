use crate::context::AppContext;
use crate::protocol::{RawRequest, Response};
use std::path::PathBuf;

/// Handle a `snapshot` request.
///
/// Snapshots a file for backup/safety purposes. This pre-populates the backup
/// store and marks the file as "tracked" for use in subsequent checkpoint/restore operations.
///
/// Expects `file` (string, required) — path to file to snapshot.
/// Returns the snapshot ID.
pub fn handle_snapshot(req: &RawRequest, ctx: &AppContext) -> Response {
    let file_param = match req.params.get("file").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => {
            return Response::error(&req.id, "invalid_request", "snapshot: missing required param 'file'");
        }
    };

    let file_path = PathBuf::from(file_param);

    // Snapshot the file using the backup store
    let snapshot_id = match ctx.backup().borrow_mut().snapshot(&file_path, "snapshot") {
        Ok(id) => id,
        Err(e) => {
            return Response::error(&req.id, "snapshot_failed", format!("failed to snapshot file: {}", e));
        }
    };

    Response::success(&req.id, serde_json::json!({ "snapshot_id": snapshot_id }))
}
