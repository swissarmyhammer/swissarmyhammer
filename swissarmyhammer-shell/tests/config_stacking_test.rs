//! End-to-end integration tests for shell config stacking.
//!
//! Exercises the full builtin → user → project precedence chain using
//! real YAML files on disk via tempdir.

use std::collections::HashMap;
use swissarmyhammer_shell::{
    evaluate_command, load_shell_config_from_paths, CompiledShellConfig, ShellSecurityValidator,
};
use tempfile::TempDir;

/// Helper: write a YAML config file into a temp overlay directory.
fn write_overlay(dir: &std::path::Path, yaml: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("config.yaml"), yaml).unwrap();
}

/// Helper: load config from overlay dirs, compile, and evaluate a command.
fn eval(overlays: &[std::path::PathBuf], command: &str) -> Result<(), String> {
    let config = load_shell_config_from_paths(overlays);
    let compiled =
        CompiledShellConfig::compile(&config).map_err(|e| format!("compile error: {e}"))?;
    evaluate_command(command, &compiled).map_err(|e| format!("{e}"))
}

// ---------------------------------------------------------------------------
// Scenario 1: Project permits what builtin denies
// ---------------------------------------------------------------------------

#[test]
fn project_permit_overrides_builtin_deny() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");

    // Builtin denies `sudo\s+`. Project permits `sudo\s+apt`.
    write_overlay(
        &project,
        r#"
permit:
  - pattern: 'sudo\s+apt'
    reason: "Project allows apt via sudo"
"#,
    );

    // `sudo apt install foo` matches permit → allowed
    assert!(eval(&[project.clone()], "sudo apt install foo").is_ok());

    // `sudo reboot` does NOT match permit → still denied by builtin
    assert!(eval(&[project], "sudo reboot").is_err());
}

// ---------------------------------------------------------------------------
// Scenario 2: User adds custom deny
// ---------------------------------------------------------------------------

#[test]
fn user_adds_custom_deny() {
    let tmp = TempDir::new().unwrap();
    let user = tmp.path().join("user");

    write_overlay(
        &user,
        r#"
deny:
  - pattern: 'docker\s+rm'
    reason: "No removing docker containers"
"#,
    );

    // `docker rm container` matches the user deny → blocked
    assert!(eval(&[user.clone()], "docker rm my-container").is_err());

    // `docker ps` is not denied by anything → allowed
    assert!(eval(&[user], "docker ps").is_ok());
}

// ---------------------------------------------------------------------------
// Scenario 3: Project overrides settings
// ---------------------------------------------------------------------------

#[test]
fn project_overrides_max_command_length() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");

    write_overlay(
        &project,
        r#"
settings:
  max_command_length: 8192
"#,
    );

    let config = load_shell_config_from_paths(&[project]);

    // Settings from project layer should override builtin's 4096
    assert_eq!(config.settings.max_command_length, 8192);

    // A validator built from this config should allow a 5000-char command
    let validator = ShellSecurityValidator::from_config(&config).unwrap();
    let long_cmd = format!("echo {}", "x".repeat(4990));
    assert!(
        validator.validate_command(&long_cmd).is_ok(),
        "5000-char command should pass with 8192 limit"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: Hot reload — no caching
// ---------------------------------------------------------------------------

#[test]
fn hot_reload_picks_up_config_changes() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");

    // Start with no overlays — builtin denies `eval\s+`
    assert!(eval(&[], "eval dangerous_thing").is_err());

    // Now write a project overlay that permits `eval`
    write_overlay(
        &project,
        r#"
permit:
  - pattern: 'eval\s+'
    reason: "Project allows eval"
"#,
    );

    // Re-load with the overlay — same command is now permitted
    assert!(
        eval(&[project.clone()], "eval dangerous_thing").is_ok(),
        "eval should be allowed after overlay permits it"
    );

    // Overwrite the overlay to remove the permit
    write_overlay(
        &project,
        r#"
permit: []
"#,
    );

    // Re-load again — eval is denied again
    assert!(
        eval(&[project], "eval dangerous_thing").is_err(),
        "eval should be denied again after permit removed"
    );
}

// ---------------------------------------------------------------------------
// Scenario 5: Full three-layer stack (builtin + user + project)
// ---------------------------------------------------------------------------

#[test]
fn three_layer_stack_merges_correctly() {
    let tmp = TempDir::new().unwrap();
    let user = tmp.path().join("user");
    let project = tmp.path().join("project");

    // User layer: add a custom deny
    write_overlay(
        &user,
        r#"
deny:
  - pattern: 'terraform\s+destroy'
    reason: "User blocks terraform destroy"
"#,
    );

    // Project layer: permit a specific sudo pattern + override settings
    write_overlay(
        &project,
        r#"
permit:
  - pattern: 'sudo\s+make\s+install'
    reason: "Project needs sudo make install"
settings:
  max_command_length: 16384
"#,
    );

    let config = load_shell_config_from_paths(&[user, project]);

    // Builtin denies present
    assert!(config.deny.iter().any(|r| r.pattern.contains("rm\\s+-rf")));
    // User deny merged
    assert!(config
        .deny
        .iter()
        .any(|r| r.pattern == "terraform\\s+destroy"));
    // Project permit merged
    assert!(config
        .permit
        .iter()
        .any(|r| r.pattern == "sudo\\s+make\\s+install"));
    // Project settings win (last layer)
    assert_eq!(config.settings.max_command_length, 16384);

    // Functional check
    let compiled = CompiledShellConfig::compile(&config).unwrap();

    // Builtin deny still works
    assert!(evaluate_command("rm -rf /", &compiled).is_err());
    // User deny works
    assert!(evaluate_command("terraform destroy -auto-approve", &compiled).is_err());
    // Project permit overrides builtin deny for sudo
    assert!(evaluate_command("sudo make install", &compiled).is_ok());
    // Regular sudo still denied
    assert!(evaluate_command("sudo rm -rf /", &compiled).is_err());
}

// ---------------------------------------------------------------------------
// Scenario 6: Environment variable validation uses config settings
// ---------------------------------------------------------------------------

#[test]
fn env_var_validation_respects_config_settings() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("project");

    // Project raises max env value length
    write_overlay(
        &project,
        r#"
settings:
  max_env_value_length: 4096
"#,
    );

    let config = load_shell_config_from_paths(&[project]);

    // A 2000-char env value should pass with 4096 limit
    let mut env = HashMap::new();
    env.insert("BIG_VAR".to_string(), "x".repeat(2000));
    assert!(
        ShellSecurityValidator::validate_environment_variables_with_settings(
            &env,
            config.settings.max_env_value_length
        )
        .is_ok()
    );

    // A 5000-char env value should fail even with 4096 limit
    env.insert("TOO_BIG".to_string(), "x".repeat(5000));
    assert!(
        ShellSecurityValidator::validate_environment_variables_with_settings(
            &env,
            config.settings.max_env_value_length
        )
        .is_err()
    );
}
