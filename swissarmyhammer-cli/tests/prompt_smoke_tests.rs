use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn test_prompt_command_shows_help() {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("prompt");
    
    let output = cmd.assert().success();
    
    // Should show help/usage information
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("prompt"), "Output should mention prompt functionality");
    assert!(!stdout.is_empty(), "Should produce helpful output");
}

#[test] 
fn test_prompt_list_command_runs() {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("prompt").arg("list");
    
    let output = cmd.assert().success();
    
    // Should list available prompts
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(!stdout.is_empty(), "Should list prompts or show 'no prompts found'");
}

#[test]
fn test_prompt_test_help_shows_usage() {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("prompt").arg("test").arg("--help");
    
    let output = cmd.assert().success();
    
    // Should show test command help
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("test"), "Should show test command help");
    assert!(stdout.contains("prompt"), "Should mention prompt in help");
}

#[test]
fn test_prompt_list_with_global_verbose() {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("--verbose").arg("prompt").arg("list");
    
    // Should run without error (even if verbose doesn't work yet)
    cmd.assert().success();
}

#[test]
fn test_prompt_list_with_global_format_json() {
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("--format").arg("json").arg("prompt").arg("list");
    
    let output = cmd.assert().success();
    
    // Should return valid JSON output
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("["), "Should output JSON array");
    assert!(stdout.contains("]"), "Should output valid JSON array");
}