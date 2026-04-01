//! Integration tests for `extract_function` through the binary protocol.
//!
//! Uses temp-dir isolation (copy fixtures, mutate copies, verify results)
//! to test the full extract pipeline: free variable detection, return value
//! inference, function generation, dry-run mode, and error paths.

use crate::helpers::{fixture_path, AftProcess};

/// Copy the `tests/fixtures/extract_function/` directory into a temp dir.
/// Returns `(TempDir, root_path)`.
fn setup_extract_fixture() -> (tempfile::TempDir, String) {
    let fixtures = fixture_path("extract_function");
    let tmp = tempfile::tempdir().expect("create temp dir");

    for entry in std::fs::read_dir(&fixtures).expect("read fixtures dir") {
        let entry = entry.expect("read entry");
        let src = entry.path();
        if src.is_file() {
            let dst = tmp.path().join(entry.file_name());
            std::fs::copy(&src, &dst).expect("copy fixture file");
        }
    }

    let root = tmp.path().display().to_string();
    (tmp, root)
}

/// Helper: configure aft with the given project root and assert success.
fn configure(aft: &mut AftProcess, root: &str) {
    let resp = aft.send(&format!(
        r#"{{"id":"cfg","command":"configure","project_root":"{}"}}"#,
        root
    ));
    assert_eq!(
        resp["success"], true,
        "configure should succeed: {:?}",
        resp
    );
}

// ---------------------------------------------------------------------------
// Success path tests
// ---------------------------------------------------------------------------

/// Basic extract: TS function with 3 free variables → new function has 3 params,
/// original range replaced with call.
#[test]
fn extract_function_basic_ts() {
    let (_tmp, root) = setup_extract_fixture();
    let mut aft = AftProcess::spawn();
    configure(&mut aft, &root);

    let file = format!("{}/sample.ts", root);

    // Extract lines 5-9 of processData (the body lines with filtered, mapped, result, console.log)
    // These use `items`, `prefix` from the enclosing function → free variables
    let resp = aft.send(&format!(
        r#"{{"id":"1","command":"extract_function","file":"{}","name":"doProcess","start_line":6,"end_line":10}}"#,
        file
    ));

    assert_eq!(resp["success"], true, "extract should succeed: {:?}", resp);
    assert_eq!(resp["name"], "doProcess");

    // Should detect free variables from the enclosing function scope
    let params = resp["parameters"].as_array().expect("parameters array");
    assert!(
        params.len() >= 2,
        "should have at least 2 parameters (items, prefix), got {:?}",
        params
    );

    // Verify file was modified
    let content = std::fs::read_to_string(&file).expect("read file");
    assert!(
        content.contains("function doProcess"),
        "should contain the extracted function:\n{}",
        content
    );
    assert!(
        content.contains("doProcess("),
        "should contain the call site:\n{}",
        content
    );

    aft.shutdown();
}

/// Extract with return value: variable assigned in range is used after range.
#[test]
fn extract_function_with_return_value() {
    let (_tmp, root) = setup_extract_fixture();
    let mut aft = AftProcess::spawn();
    configure(&mut aft, &root);

    let file = format!("{}/sample.ts", root);

    // Extract lines 13-14 of simpleHelper (doubled and added)
    // `added` is used after the range (return added) → return value
    let resp = aft.send(&format!(
        r#"{{"id":"1","command":"extract_function","file":"{}","name":"computeValues","start_line":14,"end_line":16}}"#,
        file
    ));

    assert_eq!(resp["success"], true, "extract should succeed: {:?}", resp);

    // Should detect x as a parameter
    let params = resp["parameters"].as_array().expect("parameters array");
    assert!(
        !params.is_empty(),
        "should have at least one parameter (x), got {:?}",
        params
    );

    // Verify the return type
    let return_type = resp["return_type"].as_str().unwrap();
    assert!(
        return_type == "variable" || return_type == "expression",
        "should detect return value, got: {}",
        return_type
    );

    // Verify the file was modified with the new function
    let content = std::fs::read_to_string(&file).expect("read file");
    assert!(
        content.contains("function computeValues"),
        "should contain extracted function:\n{}",
        content
    );

    aft.shutdown();
}

/// Python extract: verify correct `def` syntax.
#[test]
fn extract_function_python() {
    let (_tmp, root) = setup_extract_fixture();
    let mut aft = AftProcess::spawn();
    configure(&mut aft, &root);

    let file = format!("{}/sample.py", root);

    // Extract lines 5-8 of process_data body
    let resp = aft.send(&format!(
        r#"{{"id":"1","command":"extract_function","file":"{}","name":"do_process","start_line":6,"end_line":10}}"#,
        file
    ));

    assert_eq!(
        resp["success"], true,
        "python extract should succeed: {:?}",
        resp
    );
    assert_eq!(resp["name"], "do_process");

    let params = resp["parameters"].as_array().expect("parameters array");
    assert!(
        params.len() >= 2,
        "should detect parameters from enclosing scope, got {:?}",
        params
    );

    // Verify Python syntax: def keyword
    let content = std::fs::read_to_string(&file).expect("read file");
    assert!(
        content.contains("def do_process"),
        "should use Python def syntax:\n{}",
        content
    );

    aft.shutdown();
}

/// Dry-run: file unchanged, diff returned.
#[test]
fn extract_function_dry_run() {
    let (_tmp, root) = setup_extract_fixture();
    let mut aft = AftProcess::spawn();
    configure(&mut aft, &root);

    let file = format!("{}/sample.ts", root);

    // Snapshot before
    let before = std::fs::read_to_string(&file).unwrap();

    let resp = aft.send(&format!(
        r#"{{"id":"1","command":"extract_function","file":"{}","name":"preview","start_line":6,"end_line":10,"dry_run":true}}"#,
        file
    ));

    assert_eq!(resp["success"], true, "dry_run should succeed: {:?}", resp);
    assert_eq!(resp["dry_run"], true, "should flag dry_run");
    assert!(resp["diff"].as_str().is_some(), "should have diff");
    assert!(resp["parameters"].is_array(), "should have parameters");
    assert!(
        resp["return_type"].as_str().is_some(),
        "should have return_type"
    );

    // Verify file NOT modified
    let after = std::fs::read_to_string(&file).unwrap();
    assert_eq!(before, after, "file should be unchanged after dry_run");

    aft.shutdown();
}

/// Unsupported language error: `.rs` file returns `unsupported_language`.
#[test]
fn extract_function_unsupported_language() {
    let (_tmp, root) = setup_extract_fixture();
    let mut aft = AftProcess::spawn();
    configure(&mut aft, &root);

    // Create a .rs file in the temp dir
    let file = format!("{}/test.rs", root);
    std::fs::write(&file, "fn main() {\n    let x = 1;\n}\n").unwrap();

    let resp = aft.send(&format!(
        r#"{{"id":"1","command":"extract_function","file":"{}","name":"foo","start_line":2,"end_line":3}}"#,
        file
    ));

    assert_eq!(resp["success"], false, "should fail: {:?}", resp);
    assert_eq!(resp["code"], "unsupported_language");

    aft.shutdown();
}

/// This-reference error: range containing `this` returns `this_reference_in_range`.
#[test]
fn extract_function_this_reference() {
    let (_tmp, root) = setup_extract_fixture();
    let mut aft = AftProcess::spawn();
    configure(&mut aft, &root);

    let file = format!("{}/sample_this.ts", root);

    // Lines 4-7 of UserService.getUser contain `this.users`
    let resp = aft.send(&format!(
        r#"{{"id":"1","command":"extract_function","file":"{}","name":"extracted","start_line":5,"end_line":8}}"#,
        file
    ));

    assert_eq!(resp["success"], false, "should fail: {:?}", resp);
    assert_eq!(resp["code"], "this_reference_in_range");
    let msg = resp["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("this"),
        "error message should mention 'this': {}",
        msg
    );

    aft.shutdown();
}
