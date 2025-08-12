use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_swissarmyhammer_binary_exists() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("swissarmyhammer"));
}

#[test]
fn test_swissarmyhammer_has_expected_commands() {
    Command::cargo_bin("sah")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("prompt"))
        .stdout(predicate::str::contains("flow"))
        .stdout(predicate::str::contains("completion"))
        .stdout(predicate::str::contains("validate"));
}
