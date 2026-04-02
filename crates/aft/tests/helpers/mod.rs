//! Shared test helpers for integration tests.
//!
//! Provides `AftProcess` — a handle to a running aft binary with piped I/O —
//! and `fixture_path` for resolving test fixture files.
//!
//! Protocol adaptation layer: tests send old-format JSON, helpers convert to MCP JSON-RPC 2.0,
//! and responses are unwrapped back to old format for backward compatibility.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// A handle to a running aft process with piped I/O.
///
/// Uses a persistent `BufReader` over stdout so sequential reads
/// don't lose buffered data between calls.
pub struct AftProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
}

impl AftProcess {
    /// Spawn the aft binary with piped stdin/stdout/stderr.
    pub fn spawn() -> Self {
        Self::spawn_with_env(&[])
    }

    /// Spawn the aft binary with additional environment variables.
    /// Stderr is suppressed by default. Use `spawn_with_stderr()` for tests
    /// that need to inspect stderr output.
    pub fn spawn_with_env(envs: &[(&str, &std::ffi::OsStr)]) -> Self {
        Self::spawn_inner(envs, false)
    }

    /// Spawn with stderr piped so tests can read it via `stderr_output()`.
    pub fn spawn_with_stderr() -> Self {
        Self::spawn_inner(&[], true)
    }

    fn spawn_inner(envs: &[(&str, &std::ffi::OsStr)], pipe_stderr: bool) -> Self {
        let binary = env!("CARGO_BIN_EXE_aft-mcp");
        let mut command = Command::new(binary);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if pipe_stderr {
                Stdio::piped()
            } else {
                Stdio::null()
            });

        for (key, value) in envs {
            command.env(key, value);
        }

        let mut child = command.spawn().expect("failed to spawn aft binary");

        let stdout = child.stdout.take().expect("stdout handle");
        let reader = BufReader::new(stdout);

        AftProcess { child, reader }
    }

    /// Send a raw line and read back the JSON response.
    /// This is the low-level primitive; other methods wrap it with MCP protocol translation.
    fn send_raw(&mut self, request: &str) -> serde_json::Value {
        let stdin = self.child.stdin.as_mut().expect("stdin handle");
        writeln!(stdin, "{}", request).expect("write to stdin");
        stdin.flush().expect("flush stdin");

        let mut line = String::new();
        self.reader.read_line(&mut line).expect("read from stdout");
        assert!(
            !line.is_empty(),
            "expected a response line but got EOF from aft"
        );
        serde_json::from_str(line.trim()).expect("parse response JSON")
    }

    /// Send old-format request in new MCP JSON-RPC 2.0 format and unwrap response.
    /// Parses old-format JSON, wraps in MCP, sends, and unwraps response back to old format.
    pub fn send(&mut self, request: &str) -> serde_json::Value {
        // Parse the old-format request
        let old_format: serde_json::Value = match serde_json::from_str(request) {
            Ok(v) => v,
            Err(e) => {
                // For malformed JSON, just send the raw line to the server
                // The server's JSON-RPC handler will return a parse error
                self.send_raw(request);

                // Return parse error response with sentinel id
                return serde_json::json!({
                    "id": "_parse_error",
                    "success": false,
                    "code": "parse_error",
                    "message": format!("failed to parse request: {}", e)
                });
            }
        };

        // Extract command and other fields (now we know it's valid JSON)
        let command = match old_format
            .get("command")
            .and_then(|c| c.as_str())
        {
            Some(c) => c,
            None => {
                // If command field is missing, return a proper error
                return serde_json::json!({
                    "id": "_parse_error",
                    "success": false,
                    "code": "parse_error",
                    "message": "old-format request must have 'command' field".to_string()
                });
            }
        };

        // Build arguments object from all fields except "id" and "command"
        let mut arguments = serde_json::json!({});
        if let Some(obj) = arguments.as_object_mut() {
            if let Some(old_obj) = old_format.as_object() {
                for (key, value) in old_obj {
                    if key != "id" && key != "command" {
                        obj.insert(key.clone(), value.clone());
                    }
                }
            }
            obj.insert("command".to_string(), serde_json::json!(command));
        }

        // Build MCP JSON-RPC 2.0 request
        static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let mcp_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "aft",
                "arguments": arguments
            }
        });

        // Send MCP request and get MCP response
        let mcp_resp = self.send_raw(&mcp_req.to_string());

        // Unwrap MCP response to old format
        Self::unwrap_mcp_response(&old_format, mcp_resp)
    }

    /// Parse MCP JSON-RPC response and extract the aft payload in old format.
    /// Adds "success" field and preserves "id" from original request.
    fn unwrap_mcp_response(
        original_request: &serde_json::Value,
        mcp_response: serde_json::Value,
    ) -> serde_json::Value {
        // Extract original request id if present
        let original_id = original_request.get("id").cloned();

        // Check for JSON-RPC error
        if let Some(_err) = mcp_response.get("error") {
            let mut result = serde_json::json!({
                "success": false,
                "error": "aft error"
            });
            if let Some(id) = original_id {
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("id".to_string(), id);
                }
            }
            return result;
        }

        // Extract from result.content[0].text
        if let Some(result) = mcp_response.get("result") {
            let is_error = result
                .get("isError")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                if let Some(first) = content.first() {
                    if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                        // Parse text as JSON (should always work since main.rs uses to_string_pretty)
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
                            let mut obj = parsed;
                            if let Some(map) = obj.as_object_mut() {
                                // Add success field based on isError
                                map.insert(
                                    "success".to_string(),
                                    serde_json::json!(!is_error),
                                );
                                // Preserve original id if present
                                if let Some(id) = original_id {
                                    map.insert("id".to_string(), id);
                                }
                            }
                            return obj;
                        }

                        // Fallback: if text is not JSON, return as plain text
                        let mut response = serde_json::json!({
                            "success": !is_error,
                            "text": text
                        });
                        if let Some(id) = original_id {
                            if let Some(obj) = response.as_object_mut() {
                                obj.insert("id".to_string(), id);
                            }
                        }
                        return response;
                    }
                }
            }
        }

        // Fallback: return original request with success: false
        let mut fallback = serde_json::json!({ "success": false });
        if let Some(id) = original_id {
            if let Some(obj) = fallback.as_object_mut() {
                obj.insert("id".to_string(), id);
            }
        }
        fallback
    }

    /// Send a configure command with project_root.
    pub fn configure(&mut self, project_root: &std::path::Path) -> serde_json::Value {
        self.send(&format!(
            r#"{{"id":"cfg","command":"configure","project_root":"{}"}}"#,
            project_root.display()
        ))
    }

    /// Send a raw line that should produce no response (e.g. empty line).
    /// Verifies the process is still alive by sending a follow-up ping.
    pub fn send_silent(&mut self, request: &str) {
        let stdin = self.child.stdin.as_mut().expect("stdin handle");
        writeln!(stdin, "{}", request).expect("write to stdin");
        stdin.flush().expect("flush stdin");
    }

    /// Close stdin and wait for the process to exit. Returns the exit status.
    pub fn shutdown(mut self) -> std::process::ExitStatus {
        drop(self.child.stdin.take());
        self.child.wait().expect("wait for process exit")
    }

    /// Read stderr contents after process exits.
    pub fn stderr_output(mut self) -> (std::process::ExitStatus, String) {
        drop(self.child.stdin.take());
        let status = self.child.wait().expect("wait for process exit");
        let mut stderr_content = String::new();
        if let Some(mut stderr) = self.child.stderr.take() {
            use std::io::Read;
            stderr.read_to_string(&mut stderr_content).ok();
        }
        (status, stderr_content)
    }
}

/// Resolve a fixture file path relative to the project root.
pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}
