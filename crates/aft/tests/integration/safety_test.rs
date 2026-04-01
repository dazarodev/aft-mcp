//! Integration tests for the safety & recovery system (undo, checkpoint, edit_history).
//!
//! Tests exercise the full round-trip through the binary's JSON protocol:
//! snapshot → checkpoint → modify → restore → verify file contents.

use super::helpers::AftProcess;
use std::fs;

/// Helper: create a temp directory with a unique name for this test.
fn temp_dir(test_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join("aft_safety_tests")
        .join(test_name)
        .join(format!("{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn test_checkpoint_create_restore_cycle() {
    let dir = temp_dir("checkpoint_cycle");
    let file_a = dir.join("a.txt");
    let file_b = dir.join("b.txt");

    fs::write(&file_a, "original-a").unwrap();
    fs::write(&file_b, "original-b").unwrap();

    let mut aft = AftProcess::spawn();

    // Snapshot both files (populates backup store + tracked files)
    let resp = aft.send(&format!(
        r#"{{"id":"snap-a","command":"snapshot","file":"{}"}}"#,
        file_a.display()
    ));
    assert_eq!(resp["success"], true, "snapshot a: {:?}", resp);

    let resp = aft.send(&format!(
        r#"{{"id":"snap-b","command":"snapshot","file":"{}"}}"#,
        file_b.display()
    ));
    assert_eq!(resp["success"], true, "snapshot b: {:?}", resp);

    // Create checkpoint (no explicit files → uses tracked files from backup store)
    let resp = aft.send(r#"{"id":"cp-create","command":"checkpoint","name":"safe-point"}"#);
    assert_eq!(resp["success"], true, "checkpoint create: {:?}", resp);
    assert_eq!(resp["name"], "safe-point");
    assert!(resp["file_count"].as_u64().unwrap() >= 2);

    // Modify files externally
    fs::write(&file_a, "modified-a").unwrap();
    fs::write(&file_b, "modified-b").unwrap();
    assert_eq!(fs::read_to_string(&file_a).unwrap(), "modified-a");
    assert_eq!(fs::read_to_string(&file_b).unwrap(), "modified-b");

    // Restore checkpoint
    let resp =
        aft.send(r#"{"id":"cp-restore","command":"restore_checkpoint","name":"safe-point"}"#);
    assert_eq!(resp["success"], true, "restore: {:?}", resp);
    assert_eq!(resp["name"], "safe-point");

    // Verify files match original content
    assert_eq!(
        fs::read_to_string(&file_a).unwrap(),
        "original-a",
        "file a should be restored"
    );
    assert_eq!(
        fs::read_to_string(&file_b).unwrap(),
        "original-b",
        "file b should be restored"
    );

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_undo_restores_previous_version() {
    let dir = temp_dir("undo_restore");
    let file = dir.join("target.txt");

    fs::write(&file, "version-1").unwrap();

    let mut aft = AftProcess::spawn();

    // Snapshot the original
    let resp = aft.send(&format!(
        r#"{{"id":"snap-1","command":"snapshot","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(resp["success"], true);

    // Overwrite externally
    fs::write(&file, "version-2").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "version-2");

    // Undo → should restore version-1
    let resp = aft.send(&format!(
        r#"{{"id":"undo-1","command":"undo","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(resp["success"], true, "undo: {:?}", resp);
    assert!(resp["backup_id"].is_string());
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "version-1",
        "file should be restored to version-1"
    );

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_undo_restores_file_after_edit_command() {
    let dir = temp_dir("undo_after_edit_command");
    let file = dir.join("target.txt");

    fs::write(&file, "hello world\n").unwrap();

    let mut aft = AftProcess::spawn();

    let edit = serde_json::json!({
        "id": "edit-before-undo",
        "command": "edit_match",
        "file": file.display().to_string(),
        "match": "world",
        "replacement": "rust"
    });
    let edit_resp = aft.send(&serde_json::to_string(&edit).unwrap());
    assert_eq!(
        edit_resp["success"], true,
        "edit should succeed: {edit_resp:?}"
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello rust\n");

    let undo = aft.send(&format!(
        r#"{{"id":"undo-after-edit","command":"undo","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(undo["success"], true, "undo should succeed: {undo:?}");
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello world\n");

    let history = aft.send(&format!(
        r#"{{"id":"history-after-undo","command":"edit_history","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(history["success"], true);
    assert!(history["entries"].as_array().unwrap().is_empty());

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_edit_history_returns_stack() {
    let dir = temp_dir("edit_history");
    let file = dir.join("tracked.txt");

    fs::write(&file, "v1").unwrap();

    let mut aft = AftProcess::spawn();

    // Snapshot v1
    aft.send(&format!(
        r#"{{"id":"s1","command":"snapshot","file":"{}"}}"#,
        file.display()
    ));

    // Modify and snapshot v2
    fs::write(&file, "v2").unwrap();
    aft.send(&format!(
        r#"{{"id":"s2","command":"snapshot","file":"{}"}}"#,
        file.display()
    ));

    // Modify and snapshot v3
    fs::write(&file, "v3").unwrap();
    aft.send(&format!(
        r#"{{"id":"s3","command":"snapshot","file":"{}"}}"#,
        file.display()
    ));

    // Query edit history
    let resp = aft.send(&format!(
        r#"{{"id":"hist","command":"edit_history","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(resp["success"], true, "edit_history: {:?}", resp);

    let entries = resp["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 3, "should have 3 history entries");

    // Most recent first (reversed from stack order)
    for entry in entries {
        assert!(entry["backup_id"].is_string());
        assert!(entry["timestamp"].is_u64());
        assert!(entry["description"].is_string());
    }

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_list_checkpoints() {
    let dir = temp_dir("list_checkpoints");
    let file_a = dir.join("a.txt");
    let file_b = dir.join("b.txt");

    fs::write(&file_a, "data-a").unwrap();
    fs::write(&file_b, "data-b").unwrap();

    let mut aft = AftProcess::spawn();

    // Create checkpoint with 1 file
    let resp = aft.send(&format!(
        r#"{{"id":"cp1","command":"checkpoint","name":"first","files":["{}"]}}"#,
        file_a.display()
    ));
    assert_eq!(resp["success"], true);

    // Create checkpoint with 2 files
    let resp = aft.send(&format!(
        r#"{{"id":"cp2","command":"checkpoint","name":"second","files":["{}","{}"]}}"#,
        file_a.display(),
        file_b.display()
    ));
    assert_eq!(resp["success"], true);

    // List checkpoints
    let resp = aft.send(r#"{"id":"list","command":"list_checkpoints"}"#);
    assert_eq!(resp["success"], true, "list_checkpoints: {:?}", resp);

    let checkpoints = resp["checkpoints"].as_array().expect("checkpoints array");
    assert_eq!(checkpoints.len(), 2);

    let names: Vec<&str> = checkpoints
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"first"));
    assert!(names.contains(&"second"));

    // Verify file counts
    let first = checkpoints.iter().find(|c| c["name"] == "first").unwrap();
    let second = checkpoints.iter().find(|c| c["name"] == "second").unwrap();
    assert_eq!(first["file_count"], 1);
    assert_eq!(second["file_count"], 2);

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_undo_no_history_error() {
    let dir = temp_dir("undo_no_history");
    let file = dir.join("never_snapshotted.txt");
    fs::write(&file, "content").unwrap();

    let mut aft = AftProcess::spawn();

    // Undo with no prior snapshots → error
    let resp = aft.send(&format!(
        r#"{{"id":"undo-err","command":"undo","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(resp["success"], false, "undo should fail: {:?}", resp);
    assert_eq!(resp["code"], "no_undo_history");
    assert!(resp["message"]
        .as_str()
        .unwrap()
        .contains(&file.display().to_string())
        .then_some(true)
        .or_else(|| Some(
            resp["message"]
                .as_str()
                .unwrap()
                .contains("no undo history")
        ))
        .unwrap());

    // Process should still be alive
    let resp = aft.send(r#"{"id":"alive-1","command":"ping"}"#);
    assert_eq!(resp["success"], true, "process should survive error");

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_restore_nonexistent_checkpoint() {
    let mut aft = AftProcess::spawn();

    // Restore a checkpoint that doesn't exist → error
    let resp = aft.send(r#"{"id":"rc-err","command":"restore_checkpoint","name":"ghost"}"#);
    assert_eq!(resp["success"], false, "restore should fail: {:?}", resp);
    assert_eq!(resp["code"], "checkpoint_not_found");
    assert!(resp["message"].as_str().unwrap().contains("ghost"));

    // Process should still be alive
    let resp = aft.send(r#"{"id":"alive-2","command":"ping"}"#);
    assert_eq!(resp["success"], true, "process should survive error");

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_checkpoint_overwrite() {
    let dir = temp_dir("checkpoint_overwrite");
    let file_a = dir.join("a.txt");
    let file_b = dir.join("b.txt");

    fs::write(&file_a, "a-v1").unwrap();
    fs::write(&file_b, "b-v1").unwrap();

    let mut aft = AftProcess::spawn();

    // Create checkpoint "reusable" with file_a
    let resp = aft.send(&format!(
        r#"{{"id":"ow1","command":"checkpoint","name":"reusable","files":["{}"]}}"#,
        file_a.display()
    ));
    assert_eq!(resp["success"], true);
    assert_eq!(resp["file_count"], 1);

    // Modify files
    fs::write(&file_a, "a-v2").unwrap();
    fs::write(&file_b, "b-v2").unwrap();

    // Overwrite checkpoint "reusable" with both files (different content now)
    let resp = aft.send(&format!(
        r#"{{"id":"ow2","command":"checkpoint","name":"reusable","files":["{}","{}"]}}"#,
        file_a.display(),
        file_b.display()
    ));
    assert_eq!(resp["success"], true);
    assert_eq!(resp["file_count"], 2);

    // Modify files again
    fs::write(&file_a, "a-v3").unwrap();
    fs::write(&file_b, "b-v3").unwrap();

    // Restore → should get v2 content (the second checkpoint), not v1
    let resp = aft.send(r#"{"id":"ow-restore","command":"restore_checkpoint","name":"reusable"}"#);
    assert_eq!(resp["success"], true, "restore: {:?}", resp);

    assert_eq!(fs::read_to_string(&file_a).unwrap(), "a-v2");
    assert_eq!(fs::read_to_string(&file_b).unwrap(), "b-v2");

    // Process should still be alive after all this
    let resp = aft.send(r#"{"id":"alive-3","command":"ping"}"#);
    assert_eq!(resp["success"], true);

    let status = aft.shutdown();
    assert!(status.success());
}

#[test]
fn test_edit_history_caps_at_twenty_entries_per_file() {
    let dir = temp_dir("history_cap");
    let file = dir.join("history_cap.txt");
    fs::write(&file, "v0").unwrap();

    let mut aft = AftProcess::spawn();

    for i in 1..=21 {
        let req = serde_json::json!({
            "id": format!("edit-{i}"),
            "command": "edit_match",
            "file": file.display().to_string(),
            "match": format!("v{}", i - 1),
            "replacement": format!("v{i}")
        });
        let resp = aft.send(&serde_json::to_string(&req).unwrap());
        assert_eq!(resp["success"], true, "edit {i} failed: {resp:?}");
    }

    assert_eq!(fs::read_to_string(&file).unwrap(), "v21");

    let history = aft.send(&format!(
        r#"{{"id":"hist-cap","command":"edit_history","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(history["success"], true, "history failed: {:?}", history);

    let entries = history["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 20, "history should be capped: {:?}", entries);
    assert_eq!(entries[0]["description"], "edit_match: v20");
    assert_eq!(entries[19]["description"], "edit_match: v1");
    assert!(!entries
        .iter()
        .any(|entry| entry["description"] == "edit_match: v0"));

    for expected in (1..=20).rev() {
        let undo = aft.send(&format!(
            r#"{{"id":"undo-{expected}","command":"undo","file":"{}"}}"#,
            file.display()
        ));
        assert_eq!(undo["success"], true, "undo {expected} failed: {undo:?}");
        assert_eq!(fs::read_to_string(&file).unwrap(), format!("v{expected}"));
    }

    let no_more_history = aft.send(&format!(
        r#"{{"id":"undo-empty","command":"undo","file":"{}"}}"#,
        file.display()
    ));
    assert_eq!(no_more_history["success"], false);
    assert_eq!(no_more_history["code"], "no_undo_history");
    assert_eq!(fs::read_to_string(&file).unwrap(), "v1");

    let status = aft.shutdown();
    assert!(status.success());
}
