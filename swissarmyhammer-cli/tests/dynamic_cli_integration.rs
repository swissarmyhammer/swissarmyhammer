use assert_cmd::Command;
use predicates::prelude::*;
use swissarmyhammer::test_utils::create_test_home_guard;

/// Test dynamic CLI integration - verifies that dynamic commands are properly generated
/// from MCP tools and work alongside static commands
#[test]
fn test_dynamic_cli_help_includes_mcp_commands() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        // Static commands should still be present
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("prompt"))
        .stdout(predicate::str::contains("flow"))
        // Dynamic MCP commands should now be present
        .stdout(predicate::str::contains("file"))
        .stdout(predicate::str::contains("issue"))
        .stdout(predicate::str::contains("memo"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("shell"))
        .stdout(predicate::str::contains("web-search"));
}

#[test]
fn test_dynamic_issue_command_help() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("issue")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Issue management commands"))
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("work"))
        .stdout(predicate::str::contains("merge"));
}

#[test]
fn test_dynamic_memo_command_help() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("memo")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("delete"))
        .stdout(predicate::str::contains("search"));
}

#[test]
fn test_dynamic_file_command_help() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("file")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("read"))
        .stdout(predicate::str::contains("write"))
        .stdout(predicate::str::contains("edit"))
        .stdout(predicate::str::contains("glob"))
        .stdout(predicate::str::contains("grep"));
}

#[test]
fn test_dynamic_shell_command_help() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("shell")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("exec"));
}

#[test]
fn test_static_commands_still_work() {
    let _guard = create_test_home_guard();
    
    // Test that prompt command (static) still works
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("prompt")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("test"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("validate"));
}

#[test]
fn test_flow_command_still_works() {
    let _guard = create_test_home_guard();
    
    // Test that flow command (static) still works
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("flow")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("status"));
}

/// Test that dynamic commands can be executed with proper subcommands
#[test]
fn test_issue_list_command_executes() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("issue")
        .arg("list")
        .assert()
        .success(); // Should not fail, even if no issues exist
}

/// Test that dynamic commands handle invalid subcommands properly
#[test]
fn test_dynamic_command_invalid_subcommand() {
    let _guard = create_test_home_guard();
    
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("issue")
        .arg("invalid-subcommand")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"))
        .stderr(predicate::str::contains("invalid-subcommand"));
}

/// Test that the CLI gracefully handles tool loading errors
#[test]
fn test_cli_handles_tool_loading_gracefully() {
    let _guard = create_test_home_guard();
    
    // Even if MCP tools fail to load for some reason, the CLI should still show help
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_completion_generation_works() {
    let _guard = create_test_home_guard();
    
    // Test that shell completion generation works (dynamic commands are runtime-only)
    let mut cmd = Command::cargo_bin("sah").unwrap();
    cmd.arg("completion")
        .arg("bash")
        .assert()
        .success()
        // Should generate valid bash completion script with static commands
        .stdout(predicate::str::contains("_swissarmyhammer"))
        .stdout(predicate::str::contains("complete"));
}