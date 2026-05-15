//! Comprehensive integration tests for prompt commands
//!
//! This module provides complete test coverage for all prompt command combinations,
//! including basic commands, global arguments, error scenarios, and edge cases.
//! These tests ensure the prompt command works correctly end-to-end from CLI
//! parsing through execution and output formatting.

#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

/// Test basic prompt command shows help/list when no subcommand provided
#[test]
fn test_prompt_command_shows_help() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .assert()
        .success()
        .stdout(predicate::str::contains("prompt").or(predicate::str::contains("Name")));
}

/// Test prompt list basic functionality
#[test]
fn test_prompt_list_basic() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test prompt test help shows usage information
#[test]
fn test_prompt_test_help() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("test").and(predicate::str::contains("Usage")));
}

/// Test prompt test with existing prompt (say-hello)
#[test]
fn test_prompt_test_say_hello() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test prompt test with variable substitution
#[test]
fn test_prompt_test_with_variable() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--var")
        .arg("name=World")
        .assert()
        .success()
        .stdout(predicate::str::contains("World"));
}

/// Test prompt test with multiple variables
#[test]
fn test_prompt_test_with_multiple_variables() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--var")
        .arg("name=Alice")
        .arg("--var")
        .arg("language=Spanish")
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"));
}

/// Test prompt test with nonexistent prompt returns error
#[test]
fn test_prompt_test_nonexistent_prompt() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("definitely-does-not-exist-12345")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("failed")));
}

/// Test global verbose flag with prompt list
#[test]
fn test_global_verbose_with_prompt_list() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--verbose")
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Description").or(predicate::str::is_empty().not()));
}

/// Test global format JSON with prompt list
#[test]
fn test_global_format_json_with_prompt_list() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format")
        .arg("json")
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("[").or(predicate::str::starts_with("{")));
}

/// Test global format YAML with prompt list
#[test]
fn test_global_format_yaml_with_prompt_list() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format")
        .arg("yaml")
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test that prompt list shows builtin prompts like say-hello
#[test]
fn test_prompt_list_shows_builtin_prompts() {
    let assert_result = Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success();
    let output = assert_result.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show some builtin prompts like say-hello
    assert!(
        stdout.contains("say-hello") || stdout.contains("Available prompts:"),
        "Should list prompts or show appropriate message. Got: {}",
        stdout
    );
}

/// Test invalid prompt subcommand
#[test]
fn test_invalid_prompt_subcommand() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("invalid-subcommand")
        .assert()
        .failure();
}

/// Test prompt test with missing prompt name
#[test]
fn test_prompt_test_missing_prompt_name() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required").or(predicate::str::contains("Usage")));
}

/// Test invalid global format option
#[test]
fn test_invalid_global_format() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format")
        .arg("invalid-format-12345")
        .arg("prompt")
        .arg("list")
        .assert()
        .failure();
}

/// Test global debug flag with prompt commands
#[test]
fn test_global_debug_with_prompt_test() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--debug")
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test global quiet flag with prompt commands
#[test]
fn test_global_quiet_with_prompt_test() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--quiet")
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .assert()
        .success();
}

/// Test combined global flags
#[test]
fn test_combined_global_flags() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--verbose")
        .arg("--format")
        .arg("json")
        .arg("prompt")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("[").or(predicate::str::starts_with("{")));
}

/// Test prompt test raw flag
#[test]
fn test_prompt_test_raw_flag() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--raw")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test prompt test debug flag
#[test]
fn test_prompt_test_debug_flag() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--debug")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test prompt test copy flag (should not fail even if clipboard unavailable)
#[test]
fn test_prompt_test_copy_flag() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--copy")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

/// Test that all basic prompt subcommands exist and work
#[test]
fn test_all_basic_subcommands_exist() {
    // Test that list works
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success();

    // Test that test works
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .assert()
        .success();
}

/// Test error handling with malformed variable arguments
#[test]
fn test_malformed_var_arguments() {
    // Test variable without equals sign
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--var")
        .arg("invalid-var-format")
        .assert()
        .success(); // Should succeed but might warn about invalid format
}

/// Test prompt command exit codes
#[test]
fn test_prompt_command_exit_codes() {
    // Success case should return 0
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .code(0);

    // Failure case should return non-zero
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("nonexistent-prompt-xyz")
        .assert()
        .code(predicate::ne(0));
}

/// Test that prompt commands handle stdin/stdout correctly
#[test]
fn test_prompt_commands_handle_io_correctly() {
    let output = Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .output()
        .expect("failed to execute process");

    // Should produce output on stdout
    assert!(
        !output.stdout.is_empty(),
        "Command should produce stdout output"
    );

    // Should not crash or hang
    assert!(
        output.status.success() || output.status.code().is_some(),
        "Command should complete with a status code"
    );
}

/// Test prompt command performance (should complete quickly)
#[test]
fn test_prompt_command_performance() {
    use std::time::Instant;

    let start = Instant::now();

    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success();

    let duration = start.elapsed();

    // Commands should complete within reasonable time (10 seconds is very generous)
    assert!(
        duration.as_secs() < 10,
        "Prompt list command took too long: {:?}",
        duration
    );
}

/// Test that help commands work for all subcommands
#[test]
fn test_help_commands_work() {
    // Test main prompt help
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("prompt"));

    // Test list help
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"));

    // Test test help
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("test"));
}

/// Integration test for the complete workflow from issue requirements (fast version)
#[test]
fn test_complete_workflow_from_issue() {
    // This test has been optimized to run faster by testing only the most critical functionality
    // Full comprehensive testing is covered by other individual tests in the module

    // Essential workflow: list prompts and test one with variables
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("list")
        .assert()
        .success();

    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("say-hello")
        .arg("--var")
        .arg("name=World")
        .assert()
        .success();

    // Representative format test (covers global args + formatting)
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--format=json")
        .arg("prompt")
        .arg("list")
        .assert()
        .success();

    // Error handling validation
    Command::cargo_bin("sah")
        .unwrap()
        .arg("prompt")
        .arg("test")
        .arg("nonexistent")
        .assert()
        .failure();
}
