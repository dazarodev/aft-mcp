//! Integration tests for the auto-format pipeline through the binary protocol.
//!
//! Verifies that mutation commands run the formatter when available and
//! gracefully degrade when the formatter is missing or the language is unsupported.

use std::fs;
use std::process::Command;

use super::helpers::AftProcess;

// ============================================================================
// Helpers
// ============================================================================

/// Check if a binary is available on PATH by attempting to run `--version`.
fn is_on_path(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Create a temp directory scoped to format tests.
/// Create a unique temp directory for each test invocation.
fn format_test_dir(test_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join("aft_format_tests")
        .join(test_name);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ============================================================================
// format_integration tests
// ============================================================================

#[test]

fn format_integration_applied_rustfmt() {
    if !is_on_path("rustfmt") {
        eprintln!("SKIP: rustfmt not on PATH");
        return;
    }

    let dir = format_test_dir("applied_rustfmt");
    // Cargo.toml needed so config-file detection triggers for Rust
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    let target = dir.join("format_applied.rs");
    let _ = fs::remove_file(&target);

    let ugly_code = "fn  main( ){  let   x=1;  }";

    let mut aft = AftProcess::spawn();
    aft.configure(&dir);
    let resp = aft.send(&format!(
        r#"{{"id":"fmt-1","command":"write","file":"{}","content":"{}"}}"#,
        target.display(),
        ugly_code
    ));

    assert_eq!(resp["success"], true, "write should succeed: {:?}", resp);
    assert_eq!(
        resp["formatted"], true,
        "rustfmt should have formatted the file"
    );
    assert!(
        resp.get("format_skipped_reason").is_none() || resp["format_skipped_reason"].is_null(),
        "no skip reason when formatted"
    );

    // Verify on-disk content is actually formatted
    let on_disk = fs::read_to_string(&target).unwrap();
    assert!(
        !on_disk.contains("fn  main"),
        "file should be reformatted, got: {}",
        on_disk
    );
    assert!(
        on_disk.contains("fn main()"),
        "file should contain properly formatted fn main(), got: {}",
        on_disk
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// Write a .txt file → formatter is unsupported for this language.
#[test]
fn format_integration_unsupported_language() {
    let dir = format_test_dir("unsupported_lang");
    let target = dir.join("format_unsupported.txt");
    let _ = fs::remove_file(&target);

    let mut aft = AftProcess::spawn();
    let resp = aft.send(&format!(
        r#"{{"id":"fmt-2","command":"write","file":"{}","content":"hello world"}}"#,
        target.display()
    ));

    assert_eq!(resp["success"], true, "write should succeed: {:?}", resp);
    assert_eq!(
        resp["formatted"], false,
        "txt files should not be formatted"
    );
    assert_eq!(
        resp["format_skipped_reason"], "unsupported_language",
        "skip reason should be unsupported_language"
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// Write a .py file when no Python formatter is available → not_found.
#[test]
fn format_integration_not_found() {
    // This test only makes sense if neither ruff nor black is on PATH
    if is_on_path("ruff") || is_on_path("black") {
        eprintln!("SKIP: ruff or black is on PATH, cannot test not_found path");
        return;
    }

    let dir = format_test_dir("not_found");
    let target = dir.join("format_not_found.py");
    let _ = fs::remove_file(&target);

    let mut aft = AftProcess::spawn();
    let resp = aft.send(&format!(
        r#"{{"id":"fmt-3","command":"write","file":"{}","content":"x = 1"}}"#,
        target.display()
    ));

    assert_eq!(resp["success"], true, "write should succeed: {:?}", resp);
    assert_eq!(
        resp["formatted"], false,
        "should not be formatted without formatter"
    );
    assert_eq!(
        resp["format_skipped_reason"], "not_found",
        "skip reason should be not_found"
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// add_import on a .rs file → verify response has formatted field.

#[test]
fn format_integration_add_import_with_format() {
    let dir = format_test_dir("add_import");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    let target = dir.join("format_add_import.rs");
    // Write a valid Rust file with a function
    fs::write(&target, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let mut aft = AftProcess::spawn();
    aft.configure(&dir);
    let resp = aft.send(&format!(
        r#"{{"id":"fmt-4","command":"add_import","file":"{}","module":"std::collections::HashMap"}}"#,
        target.display()
    ));

    assert_eq!(
        resp["success"], true,
        "add_import should succeed: {:?}",
        resp
    );
    assert_eq!(resp["added"], true);
    // The formatted field must always be present
    assert!(
        resp.get("formatted").is_some() && !resp["formatted"].is_null(),
        "formatted field must be present in add_import response: {:?}",
        resp
    );

    // Verify the import was actually added to the file
    let on_disk = fs::read_to_string(&target).unwrap();
    assert!(
        on_disk.contains("use std::collections::HashMap"),
        "import should be in file, got: {}",
        on_disk
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// edit_symbol on a .rs file → verify formatted field in response.

#[test]
fn format_integration_edit_symbol_with_format() {
    let dir = format_test_dir("edit_symbol");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    let target = dir.join("format_edit_symbol.rs");
    // Write a Rust file with a function to edit
    fs::write(&target, "fn greet() {\n    println!(\"hi\");\n}\n").unwrap();

    let mut aft = AftProcess::spawn();
    aft.configure(&dir);

    // Use edit_symbol to replace the function
    let new_body = r#"fn greet() {\n    println!(\"hello world\");\n}"#;
    let resp = aft.send(&format!(
        r#"{{"id":"fmt-5","command":"edit_symbol","file":"{}","symbol":"greet","operation":"replace","content":"{}"}}"#,
        target.display(),
        new_body
    ));

    assert_eq!(
        resp["success"], true,
        "edit_symbol should succeed: {:?}",
        resp
    );
    // The formatted field must always be present
    assert!(
        resp.get("formatted").is_some() && !resp["formatted"].is_null(),
        "formatted field must be present in edit_symbol response: {:?}",
        resp
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// Verify that the `formatted` field is always present in mutation responses,
/// even for unsupported languages. Tests across write, add_import (which

/// even for unsupported languages. Tests across write, add_import (which
/// would fail on .txt), and edit_symbol.
#[test]
fn format_integration_fields_always_present() {
    let dir = format_test_dir("fields_present");
    // Cargo.toml needed for .rs test
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    // Test 1: write to a .md file (unsupported language for formatting)
    let md_target = dir.join("format_fields_check.md");
    let _ = fs::remove_file(&md_target);

    let mut aft = AftProcess::spawn();
    aft.configure(&dir);
    let resp = aft.send(&format!(
        r#"{{"id":"fmt-6a","command":"write","file":"{}","content":"Hello markdown"}}"#,
        md_target.display()
    ));

    assert_eq!(
        resp["success"], true,
        "write to .md should succeed: {:?}",
        resp
    );
    // `formatted` must be present (not missing from JSON)
    assert!(
        resp.get("formatted").is_some(),
        "formatted field must be present even for unsupported languages: {:?}",
        resp
    );
    assert_eq!(resp["formatted"], false);
    assert_eq!(resp["format_skipped_reason"], "not_found");

    // Test 2: write to a .rs file — formatted field present with value true (if rustfmt available)
    let rs_target = dir.join("format_fields_check.rs");
    let _ = fs::remove_file(&rs_target);

    let resp2 = aft.send(&format!(
        r#"{{"id":"fmt-6b","command":"write","file":"{}","content":"fn main() {{}}"}}"#,
        rs_target.display()
    ));

    assert_eq!(
        resp2["success"], true,
        "write to .rs should succeed: {:?}",
        resp2
    );
    assert!(
        resp2.get("formatted").is_some(),
        "formatted field must be present for .rs files: {:?}",
        resp2
    );

    let _ = fs::remove_file(&md_target);
    let _ = fs::remove_file(&rs_target);
    let status = aft.shutdown();
    assert!(status.success());
}

// ============================================================================
// validate_full integration tests
// ============================================================================

/// Send mutation without validate param → no validation_errors in response.
#[test]
fn validate_full_default_no_errors() {
    let dir = format_test_dir("validate_default");
    let target = dir.join("validate_default.rs");
    let _ = fs::remove_file(&target);

    let mut aft = AftProcess::spawn();
    let resp = aft.send(&format!(
        r#"{{"id":"val-1","command":"write","file":"{}","content":"fn main() {{}}"}}"#,
        target.display()
    ));

    assert_eq!(resp["success"], true, "write should succeed: {:?}", resp);
    // Without validate:"full", validation_errors should not be present (or empty)
    let has_errors = resp.get("validation_errors").is_some()
        && !resp["validation_errors"].is_null()
        && resp["validation_errors"]
            .as_array()
            .map_or(false, |a| !a.is_empty());
    assert!(
        !has_errors,
        "validation_errors should be absent or empty without validate:full, got: {:?}",
        resp
    );
    // validate_skipped_reason should not be present
    assert!(
        resp.get("validate_skipped_reason").is_none() || resp["validate_skipped_reason"].is_null(),
        "validate_skipped_reason should not be present without validate:full: {:?}",
        resp
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// Send write with validate:"full" on a .rs file with valid code → if cargo available,
/// response includes validation_errors: [] (empty).
#[test]
fn validate_full_with_checker() {
    if !is_on_path("cargo") {
        eprintln!("SKIP: cargo not on PATH");
        return;
    }

    let dir = format_test_dir("validate_valid");
    let target = dir.join("validate_valid.rs");
    // Write valid Rust code
    let _ = fs::remove_file(&target);

    let mut aft = AftProcess::spawn();
    let resp = aft.send(&format!(
        r#"{{"id":"val-2","command":"write","file":"{}","content":"fn main() {{}}","validate":"full"}}"#,
        target.display()
    ));

    assert_eq!(resp["success"], true, "write should succeed: {:?}", resp);
    // With validate:"full" and cargo available, we should get validation fields
    // Note: cargo check on an isolated .rs file may skip or error (no Cargo.toml),
    // so we check that the validate path was invoked (either errors or skip reason present)
    let has_validation =
        resp.get("validation_errors").is_some() || resp.get("validate_skipped_reason").is_some();
    assert!(
        has_validation,
        "validate:full should produce validation_errors or validate_skipped_reason: {:?}",
        resp
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// Send write with validate:"full" on a .txt file → validate_skipped_reason: "unsupported_language"
#[test]
fn validate_full_unsupported_language() {
    let dir = format_test_dir("validate_unsupported");
    let target = dir.join("validate_unsupported.txt");
    let _ = fs::remove_file(&target);

    let mut aft = AftProcess::spawn();
    let resp = aft.send(&format!(
        r#"{{"id":"val-3","command":"write","file":"{}","content":"hello","validate":"full"}}"#,
        target.display()
    ));

    assert_eq!(resp["success"], true, "write should succeed: {:?}", resp);
    assert_eq!(
        resp["validate_skipped_reason"], "unsupported_language",
        "should skip validation for unsupported language: {:?}",
        resp
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}

/// Send write with validate:"full" via add_import to verify validate param flows through
/// all mutation commands (not just write).
#[test]
fn validate_full_flows_through_add_import() {
    let dir = format_test_dir("validate_import");
    let target = dir.join("validate_import.rs");
    // Create a valid Rust file first
    fs::write(&target, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let mut aft = AftProcess::spawn();
    let resp = aft.send(&format!(
        r#"{{"id":"val-4","command":"add_import","file":"{}","module":"std::collections::HashMap","validate":"full"}}"#,
        target.display()
    ));

    assert_eq!(
        resp["success"], true,
        "add_import should succeed: {:?}",
        resp
    );
    // Validate param should flow through — either errors or skip reason
    let has_validation =
        resp.get("validation_errors").is_some() || resp.get("validate_skipped_reason").is_some();
    assert!(
        has_validation,
        "validate:full should produce validation_errors or validate_skipped_reason via add_import: {:?}",
        resp
    );

    let _ = fs::remove_file(&target);
    let status = aft.shutdown();
    assert!(status.success());
}
