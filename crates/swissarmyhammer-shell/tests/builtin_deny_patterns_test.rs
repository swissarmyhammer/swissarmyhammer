//! Acceptance tests pinning the builtin shell deny patterns.
//!
//! The shell tool runs AI-generated commands, so substring deny patterns can
//! never be a real security boundary (trivially evadable). Their only job is to
//! be low-false-positive guards against catastrophic *mistakes*. These tests
//! pin two things:
//!
//! 1. Legit dev commands that used to false-positive must now pass validation.
//! 2. Catastrophic-mistake guards must still block.
//!
//! Everything is exercised through the live builtin config (the same path the
//! production shell tool uses), not a hand-built policy, so drift is impossible.

use swissarmyhammer_shell::{
    parse_shell_config, CompiledShellConfig, BUILTIN_CONFIG_YAML,
};

/// Compile the builtin config exactly as the live tool loads it.
fn builtin_compiled() -> CompiledShellConfig {
    let config = parse_shell_config(BUILTIN_CONFIG_YAML).expect("builtin config must parse");
    CompiledShellConfig::compile(&config).expect("builtin config must compile")
}

/// Assert a command passes builtin validation (is NOT blocked).
fn assert_allowed(command: &str) {
    let compiled = builtin_compiled();
    let result = swissarmyhammer_shell::evaluate_command(command, &compiled);
    assert!(
        result.is_ok(),
        "command should be ALLOWED by builtin config but was blocked: {command:?} -> {:?}",
        result.err()
    );
}

/// Assert a command is blocked by builtin validation.
fn assert_blocked(command: &str) {
    let compiled = builtin_compiled();
    let result = swissarmyhammer_shell::evaluate_command(command, &compiled);
    assert!(
        result.is_err(),
        "command should be BLOCKED by builtin config but was allowed: {command:?}"
    );
}

#[test]
fn legit_dev_commands_are_not_false_positives() {
    // These all previously tripped substring deny patterns (eval, sed, format,
    // /etc/passwd, ssh) and must now pass.
    assert_allowed("cargo test eval 2>&1 | tail -30");
    assert_allowed("cargo nextest run eval");
    assert_allowed("grep -r foo .");
    assert_allowed("cd a && cargo build | tail");
    assert_allowed("sed -n '1,5p' f | head");
    assert_allowed("grep -n /etc/passwd docs/security.md");
    assert_allowed("ssh user@host echo hi");
    assert_allowed("echo formatting the output | cat");
    assert_allowed("/bin/echo hello");
}

#[test]
fn catastrophic_mistake_guards_still_block() {
    assert_blocked("rm -rf /");
    assert_blocked("rm -rf *");
    assert_blocked("dd if=/dev/zero of=/dev/disk0");
    assert_blocked("mkfs ext4 /dev/sda1");
    assert_blocked("fdisk /dev/sda");
    assert_blocked("parted /dev/sda");
}

#[test]
fn eliminated_patterns_are_gone_from_builtin() {
    let config = parse_shell_config(BUILTIN_CONFIG_YAML).expect("builtin config must parse");
    // These false-positive magnets must no longer appear as deny patterns.
    let eliminated = [
        r"eval\s+",
        r"format\s+",
        r"exec\s+/bin/",
        r"ssh\s+.*@",
        "/etc/passwd",
        "/etc/shadow",
    ];
    for pat in eliminated {
        assert!(
            !config.deny.iter().any(|r| r.pattern == pat),
            "deny pattern {pat:?} should have been eliminated from builtin config"
        );
    }
}
