//! Integration tests for workflow debug logging functionality

use std::fs;
use tempfile::TempDir;

#[test]
fn test_debug_flag_creates_ndjson_logs() {
    // Test that --debug with flow command creates NDJSON debug logs
    let temp_dir = TempDir::new().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    // Run flow command with debug flag
    let output = std::process::Command::new("cargo")
        .args(&["run", "--", "--debug", "flow", "list"])
        .output()
        .expect("Failed to run command");

    // Restore directory
    std::env::set_current_dir(old_dir).unwrap();

    // Check that debug.ndjson was created
    let debug_file = temp_dir.path().join(".swissarmyhammer/debug.ndjson");
    assert!(debug_file.exists(), "Debug NDJSON file should be created with --debug flag");

    // Check file has content
    let content = fs::read_to_string(&debug_file).unwrap();
    assert!(!content.is_empty(), "Debug file should have content");
    
    // Verify NDJSON format
    let first_line = content.lines().next().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(first_line)
        .expect("First line should be valid JSON");
    
    // Verify required fields
    assert!(parsed["timestamp"].is_string());
    assert!(parsed["level"].is_string());
    assert!(parsed["target"].is_string());
    assert!(parsed["message"].is_string());
    assert!(parsed.get("fields").is_some());
}

#[test]
fn test_no_debug_flag_creates_no_logs() {
    // Test that without --debug flag, no debug logs are created
    let temp_dir = TempDir::new().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    // Run flow command without debug flag
    let _output = std::process::Command::new("cargo")
        .args(&["run", "--", "flow", "list"])
        .output()
        .expect("Failed to run command");

    // Restore directory
    std::env::set_current_dir(old_dir).unwrap();

    // Check that debug.ndjson was NOT created
    let debug_file = temp_dir.path().join(".swissarmyhammer/debug.ndjson");
    assert!(!debug_file.exists(), "Debug NDJSON file should NOT be created without --debug flag");
}