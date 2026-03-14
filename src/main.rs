use std::io::{self, BufRead, BufWriter, Write};

use aft::config::Config;
use aft::context::AppContext;
use aft::parser::TreeSitterProvider;
use aft::protocol::{EchoParams, RawRequest, Response};

fn main() {
    eprintln!("[aft] started, pid {}", std::process::id());

    let ctx = AppContext::new(Box::new(TreeSitterProvider::new()), Config::default());

    let stdin = io::stdin();
    let reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[aft] stdin read error: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<RawRequest>(trimmed) {
            Ok(req) => dispatch(req, &ctx),
            Err(e) => {
                eprintln!("[aft] parse error: {} — input: {}", e, trimmed);
                Response::error(
                    "_parse_error",
                    "parse_error",
                    format!("failed to parse request: {}", e),
                )
            }
        };

        if let Err(e) = write_response(&mut writer, &response) {
            eprintln!("[aft] stdout write error: {}", e);
            break;
        }
    }

    eprintln!("[aft] stdin closed, shutting down");
}

fn dispatch(req: RawRequest, ctx: &AppContext) -> Response {
    match req.command.as_str() {
        "ping" => Response::success(&req.id, serde_json::json!({ "command": "pong" })),
        "version" => Response::success(&req.id, serde_json::json!({ "version": "0.1.0" })),
        "echo" => handle_echo(&req),
        "outline" => aft::commands::outline::handle_outline(&req, ctx),
        "zoom" => aft::commands::zoom::handle_zoom(&req, ctx),
        "undo" => aft::commands::undo::handle_undo(&req, ctx),
        "edit_history" => aft::commands::edit_history::handle_edit_history(&req, ctx),
        "checkpoint" => aft::commands::checkpoint::handle_checkpoint(&req, ctx),
        "restore_checkpoint" => aft::commands::restore_checkpoint::handle_restore_checkpoint(&req, ctx),
        "list_checkpoints" => aft::commands::list_checkpoints::handle_list_checkpoints(&req, ctx),
        // Test-only: populate the backup store through the protocol (no write/edit_symbol yet)
        "snapshot" => handle_snapshot(&req, ctx),
        _ => {
            eprintln!("[aft] unknown command: {}", req.command);
            Response::error(
                &req.id,
                "unknown_command",
                format!("unknown command: {}", req.command),
            )
        }
    }
}

fn handle_echo(req: &RawRequest) -> Response {
    match serde_json::from_value::<EchoParams>(req.params.clone()) {
        Ok(params) => {
            Response::success(&req.id, serde_json::json!({ "message": params.message }))
        }
        Err(e) => Response::error(
            &req.id,
            "invalid_request",
            format!("echo: invalid params: {}", e),
        ),
    }
}

/// Test-only command: snapshot a file into the backup store.
///
/// Params: `file` (string, required) — path to snapshot.
/// Returns: `{ backup_id }`.
fn handle_snapshot(req: &RawRequest, ctx: &AppContext) -> Response {
    let file = match req.params.get("file").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => {
            return Response::error(
                &req.id,
                "invalid_request",
                "snapshot: missing required param 'file'",
            );
        }
    };

    let path = std::path::Path::new(file);
    let mut backup = ctx.backup().borrow_mut();

    match backup.snapshot(path, "manual snapshot") {
        Ok(id) => Response::success(
            &req.id,
            serde_json::json!({ "backup_id": id }),
        ),
        Err(e) => Response::error(&req.id, e.code(), e.to_string()),
    }
}

fn write_response(writer: &mut BufWriter<io::StdoutLock>, response: &Response) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, response)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
