use tempfile::TempDir;

mod test_utils;
use test_utils::{create_temp_dir, setup_git_repo};

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process_with_dir;

use swissarmyhammer::test_utils::IsolatedTestEnvironment;

/// Test that the implement command starts workflow execution (demonstrates CliContext integration)
#[tokio::test]
async fn test_implement_command_starts_workflow() {
    let _guard = IsolatedTestEnvironment::new();
    let (_temp_dir, temp_path) = setup_implement_test_environment().unwrap();

    // Test basic implement command execution
    let result = run_sah_command_in_process_with_dir(&["implement"], &temp_path)
        .await
        .expect("Failed to run implement command");

    // The implement command should start workflow execution (demonstrates CliContext integration)
    // It may fail in workflow execution, but CLI parsing should work
    assert!(
        !result.stderr.contains("unexpected argument")
            && !result.stderr.contains("unrecognized subcommand"),
        "Implement command should not have CLI parsing errors. stderr: '{}'",
        result.stderr
    );

    // Should show deprecation warning
    assert!(
        result.stderr.contains("Warning: 'sah implement' wrapper command is deprecated"),
        "Should show deprecation warning. stderr: '{}'",
        result.stderr
    );

    // Should show that it started the workflow via CliContext delegation
    // (deprecation warning is shown first)
    assert!(
        result.stderr.contains("Starting workflow: implement") || result.exit_code == 0,
        "Should show workflow started via CliContext delegation or succeed. stderr: '{}', exit_code: {}",
        result.stderr,
        result.exit_code
    );
}

/// Test implement workflow delegation via flow test command (CliContext integration test)
#[tokio::test]
async fn test_implement_workflow_delegation_via_flow_test() {
    let _guard = IsolatedTestEnvironment::new();
    let (_temp_dir, temp_path) = setup_implement_test_environment().unwrap();

    // Test implement workflow via flow command with dry-run (tests CliContext delegation)
    let result =
        run_sah_command_in_process_with_dir(&["flow", "implement", "--dry-run"], &temp_path)
            .await
            .expect("Failed to run implement workflow test");

    assert!(
        result.exit_code == 0,
        "Implement workflow via flow test should succeed. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Running workflow: implement")
            || stdout.contains("Testing workflow: implement"),
        "Should execute implement workflow via CliContext delegation: {stdout}"
    );
}

/// Test implement workflow with custom variables (CliContext parameter passing)
#[tokio::test]
async fn test_implement_workflow_with_custom_variables() {
    let _guard = IsolatedTestEnvironment::new();
    let (_temp_dir, temp_path) = setup_implement_test_environment().unwrap();

    // Test implement workflow with custom variables via flow with dry-run (tests CliContext parameter passing)
    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "implement",
            "--dry-run",
            "--var",
            "test_key=test_value",
            "--var",
            "custom_param=custom_value",
        ],
        &temp_path,
    )
    .await
    .expect("Failed to run implement command with variables");

    assert!(
        result.exit_code == 0,
        "Implement workflow with variables should succeed (CliContext parameter passing). stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Running workflow: implement")
            || stdout.contains("Testing workflow: implement"),
        "Should execute workflow with custom variables via CliContext: {stdout}"
    );
}

/// Test that implement command delegates to flow correctly (CliContext delegation)
#[tokio::test]
async fn test_implement_command_delegates_to_flow() {
    let _guard = IsolatedTestEnvironment::new();
    let (_temp_dir, temp_path) = setup_implement_test_environment().unwrap();

    // Test that implement command behaves like 'sah flow run implement'
    // This tests the CliContext delegation pattern described in the code review

    // Test direct implement command
    let implement_result = run_sah_command_in_process_with_dir(&["implement"], &temp_path)
        .await
        .expect("Failed to run implement command");

    // Test equivalent flow run command (but use dry-run to avoid actual AI calls)
    let flow_result =
        run_sah_command_in_process_with_dir(&["flow", "implement", "--dry-run"], &temp_path)
            .await
            .expect("Failed to run flow implement with dry-run");

    // Implement command should start workflow (CliContext delegation working)
    assert!(
        !implement_result.stderr.contains("unexpected argument")
            && !implement_result.stderr.contains("unrecognized subcommand"),
        "Implement command should not have CLI parsing errors (CliContext working). stderr: '{}'",
        implement_result.stderr
    );

    // Should show deprecation warning
    assert!(
        implement_result.stderr.contains("Warning: 'sah implement' wrapper command is deprecated"),
        "Should show deprecation warning. stderr: '{}'",
        implement_result.stderr
    );

    assert!(
        implement_result.stderr.contains("Starting workflow: implement") || implement_result.exit_code == 0,
        "Implement command should delegate to flow via CliContext. stderr: '{}', exit_code: {}",
        implement_result.stderr,
        implement_result.exit_code
    );

    // Flow test should succeed and show the same workflow
    assert_eq!(
        flow_result.exit_code, 0,
        "Flow test implement should succeed. stderr: {}",
        flow_result.stderr
    );

    assert!(
        flow_result.stdout.contains("Running workflow: implement")
            || flow_result.stdout.contains("Testing workflow: implement"),
        "Flow should confirm it's running the same implement workflow that CliContext delegates to"
    );
}

/// Setup a complete test environment for implement command testing
fn setup_implement_test_environment() -> anyhow::Result<(TempDir, std::path::PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create necessary directories
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    std::fs::create_dir_all(&swissarmyhammer_dir)?;

    let tmp_dir = swissarmyhammer_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;

    // Initialize git repository for realistic testing
    setup_git_repo(&temp_path)?;

    Ok((temp_dir, temp_path))
}
