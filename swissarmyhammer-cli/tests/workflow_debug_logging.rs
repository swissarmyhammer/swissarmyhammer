//! Integration tests for workflow debug logging functionality
//! 
//! Tests that debug logging works correctly when --debug flag is used with flow commands

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_debug_flag_with_flow_creates_ndjson_log() {
    // Test that --debug with flow command creates debug.ndjson file
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
    for line in content.lines() {
        if !line.trim().is_empty() {
            let parsed: serde_json::Value = serde_json::from_str(line)
                .expect("Each line should be valid JSON");
            
            // Verify required fields
            assert!(parsed["timestamp"].is_string());
            assert!(parsed["level"].is_string());
            assert!(parsed["target"].is_string());
            assert!(parsed["message"].is_string());
            assert!(parsed.get("fields").is_some());
        }
    }
}

#[test]
fn test_no_debug_flag_creates_no_ndjson_file() {
    // Test that without --debug flag, no debug.ndjson is created
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

#[test]
fn test_debug_with_non_flow_command_creates_no_ndjson() {
    // Test that --debug with non-flow commands doesn't create debug.ndjson
    let temp_dir = TempDir::new().unwrap();
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    // Run non-flow command with debug flag
    let _output = std::process::Command::new("cargo")
        .args(&["run", "--", "--debug", "doctor"])
        .output()
        .expect("Failed to run command");

    // Restore directory
    std::env::set_current_dir(old_dir).unwrap();

    // Check that debug.ndjson was NOT created
    let debug_file = temp_dir.path().join(".swissarmyhammer/debug.ndjson");
    assert!(!debug_file.exists(), "Debug NDJSON file should NOT be created for non-flow commands");
}