use assert_cmd::Command;
use predicates::prelude::*;

use swissarmyhammer::test_utils::IsolatedTestEnvironment;

#[test]
fn test_help_command() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "MCP (Model Context Protocol) server",
        ))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("completion"));
}

#[test]
fn test_version_command() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("swissarmyhammer"));
}

#[test]
fn test_doctor_command() {
    let _guard = IsolatedTestEnvironment::new();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("doctor")
        .assert()
        .stdout(predicate::str::contains("SwissArmyHammer Doctor"))
        .stdout(predicate::str::contains("Running diagnostics"))
        .stdout(predicate::str::contains("Summary:"));
}

#[test]
fn test_serve_command_help() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("serve")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Runs swissarmyhammer as an MCP server",
        ));
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_quiet_flag() {
    let _guard = IsolatedTestEnvironment::new();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("--quiet")
        .arg("doctor")
        .assert()
        .code(1) // Exit code 1 for warnings
        .stdout(predicate::str::contains("SwissArmyHammer Doctor"));
}

#[test]
fn test_verbose_flag() {
    let _guard = IsolatedTestEnvironment::new();

    let mut cmd = Command::cargo_bin("swissarmyhammer").unwrap();
    cmd.arg("--verbose")
        .arg("doctor")
        .assert()
        .code(1) // Exit code 1 for warnings
        .stdout(predicate::str::contains("SwissArmyHammer Doctor"));
}
