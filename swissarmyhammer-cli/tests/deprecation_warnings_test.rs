//! Tests for deprecation warnings on implement and plan wrapper commands
//!
//! These tests verify that the deprecated wrapper commands show appropriate
//! warnings to users while still functioning correctly. The tests focus on:
//! - Warning message content and format
//! - Stderr vs stdout separation
//! - Quiet flag suppression
//! - Delegation to flow command
//!
//! All tests use in-process execution for fast feedback.

use anyhow::Result;
use tempfile::TempDir;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process_with_dir;

mod test_utils;
use test_utils::{create_temp_dir, setup_git_repo};

use swissarmyhammer::test_utils::IsolatedTestEnvironment;

/// Setup a basic test environment for deprecation warning tests
fn setup_test_environment() -> Result<(TempDir, std::path::PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create necessary directories
    let issues_dir = temp_path.join("issues");
    std::fs::create_dir_all(&issues_dir)?;

    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    std::fs::create_dir_all(&swissarmyhammer_dir)?;

    let tmp_dir = swissarmyhammer_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;

    // Initialize git repository
    setup_git_repo(&temp_path)?;

    Ok((temp_dir, temp_path))
}

/// Test that implement command shows deprecation warning
#[tokio::test]
async fn test_implement_shows_deprecation_warning() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    let result = run_sah_command_in_process_with_dir(&["implement"], &temp_path).await?;

    let stderr = &result.stderr;

    // Verify deprecation warning is present
    assert!(
        stderr.contains("Warning: 'sah implement' wrapper command is deprecated"),
        "Should show deprecation warning header. stderr: {stderr}"
    );

    assert!(
        stderr.contains("Use 'sah flow implement'"),
        "Should suggest 'sah flow implement'. stderr: {stderr}"
    );

    assert!(
        stderr.contains("(via dynamic shortcut)"),
        "Should mention dynamic shortcut. stderr: {stderr}"
    );

    assert!(
        stderr.contains("This wrapper will be removed in a future version"),
        "Should warn about future removal. stderr: {stderr}"
    );

    // Verify command still works (starts workflow execution)
    assert!(
        stderr.contains("Starting workflow: implement") || result.exit_code == 0,
        "Command should still function correctly. stderr: {stderr}"
    );

    Ok(())
}

/// Test that plan command shows deprecation warning
#[tokio::test]
async fn test_plan_shows_deprecation_warning() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    // Create a test plan file
    let plan_file = temp_path.join("test-plan.md");
    std::fs::write(
        &plan_file,
        "# Test Plan\n\nSimple test specification for deprecation warning test.",
    )?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", plan_file.to_str().unwrap()], &temp_path)
            .await?;

    let stderr = &result.stderr;

    // Verify deprecation warning is present
    assert!(
        stderr.contains("Warning: 'sah plan <file>' wrapper command is deprecated"),
        "Should show deprecation warning header. stderr: {stderr}"
    );

    assert!(
        stderr.contains("Use 'sah flow plan <file>'"),
        "Should suggest 'sah flow plan <file>'. stderr: {stderr}"
    );

    assert!(
        stderr.contains("(via dynamic shortcut)"),
        "Should mention dynamic shortcut. stderr: {stderr}"
    );

    assert!(
        stderr.contains("This wrapper will be removed in a future version"),
        "Should warn about future removal. stderr: {stderr}"
    );

    // Verify command still works (starts workflow execution)
    assert!(
        stderr.contains("Starting workflow: plan") || result.exit_code == 0,
        "Command should still function correctly. stderr: {stderr}"
    );

    Ok(())
}

/// Test that quiet flag suppresses deprecation warning for implement command
#[tokio::test]
async fn test_implement_quiet_suppresses_warning() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    let result = run_sah_command_in_process_with_dir(&["--quiet", "implement"], &temp_path).await?;

    let stderr = &result.stderr;

    // Verify deprecation warning is NOT present
    assert!(
        !stderr.contains("Warning: 'sah implement' wrapper command is deprecated"),
        "Should NOT show deprecation warning with --quiet. stderr: {stderr}"
    );

    assert!(
        !stderr.contains("This wrapper will be removed in a future version"),
        "Should NOT show removal warning with --quiet. stderr: {stderr}"
    );

    Ok(())
}

/// Test that quiet flag suppresses deprecation warning for plan command
#[tokio::test]
async fn test_plan_quiet_suppresses_warning() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    // Create a test plan file
    let plan_file = temp_path.join("test-plan.md");
    std::fs::write(
        &plan_file,
        "# Test Plan\n\nSimple test specification for quiet flag test.",
    )?;

    let result = run_sah_command_in_process_with_dir(
        &["--quiet", "plan", plan_file.to_str().unwrap()],
        &temp_path,
    )
    .await?;

    let stderr = &result.stderr;

    // Verify deprecation warning is NOT present
    assert!(
        !stderr.contains("Warning: 'sah plan <file>' wrapper command is deprecated"),
        "Should NOT show deprecation warning with --quiet. stderr: {stderr}"
    );

    assert!(
        !stderr.contains("This wrapper will be removed in a future version"),
        "Should NOT show removal warning with --quiet. stderr: {stderr}"
    );

    Ok(())
}

/// Test that implement command still delegates correctly to flow command
#[tokio::test]
async fn test_implement_delegates_correctly() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    let result = run_sah_command_in_process_with_dir(&["implement"], &temp_path).await?;

    let stdout = &result.stdout;
    let stderr = &result.stderr;

    // Verify delegation to flow command works - should start workflow
    assert!(
        stdout.contains("Starting workflow: implement") || stderr.contains("Starting workflow: implement"),
        "Should delegate to flow command and start workflow. stdout: {stdout}, stderr: {stderr}"
    );

    // Verify no CLI parsing errors
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized subcommand"),
        "Should not have CLI parsing errors. stderr: {stderr}"
    );

    Ok(())
}

/// Test that plan command still delegates correctly to flow command
#[tokio::test]
async fn test_plan_delegates_correctly() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    // Create a test plan file
    let plan_file = temp_path.join("test-plan.md");
    std::fs::write(
        &plan_file,
        "# Test Plan\n\nSimple test specification for delegation test.",
    )?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", plan_file.to_str().unwrap()], &temp_path)
            .await?;

    let stdout = &result.stdout;
    let stderr = &result.stderr;

    // Verify delegation to flow command works - should execute plan workflow
    assert!(
        stdout.contains("Running plan command") || stdout.contains("Making the plan for") || stderr.contains("Starting workflow: plan"),
        "Should delegate to flow command and start workflow. stdout: {stdout}, stderr: {stderr}"
    );

    // Verify no CLI parsing errors
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized subcommand"),
        "Should not have CLI parsing errors. stderr: {stderr}"
    );

    Ok(())
}

/// Test that warning format is consistent across both commands
#[tokio::test]
async fn test_warning_format_consistency() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    // Test implement warning
    let implement_result = run_sah_command_in_process_with_dir(&["implement"], &temp_path).await?;

    // Create plan file and test plan warning
    let plan_file = temp_path.join("test-plan.md");
    std::fs::write(&plan_file, "# Test Plan\n\nTest specification.")?;
    let plan_result =
        run_sah_command_in_process_with_dir(&["plan", plan_file.to_str().unwrap()], &temp_path)
            .await?;

    let implement_stderr = &implement_result.stderr;
    let plan_stderr = &plan_result.stderr;

    // Both should have consistent warning structure
    assert!(
        implement_stderr.contains("Warning:") && plan_stderr.contains("Warning:"),
        "Both commands should use 'Warning:' prefix"
    );

    assert!(
        implement_stderr.contains("deprecated") && plan_stderr.contains("deprecated"),
        "Both commands should mention deprecation"
    );

    assert!(
        implement_stderr.contains("(via dynamic shortcut)")
            && plan_stderr.contains("(via dynamic shortcut)"),
        "Both commands should mention dynamic shortcuts"
    );

    assert!(
        implement_stderr.contains("will be removed in a future version")
            && plan_stderr.contains("will be removed in a future version"),
        "Both commands should warn about future removal"
    );

    Ok(())
}

/// Test that warnings are written to stderr, not stdout
#[tokio::test]
async fn test_warnings_on_stderr() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;
    let (_temp_dir, temp_path) = setup_test_environment()?;

    let result = run_sah_command_in_process_with_dir(&["implement"], &temp_path).await?;

    let stdout = &result.stdout;
    let stderr = &result.stderr;

    // Verify warning is on stderr, not stdout
    assert!(
        stderr.contains("Warning:"),
        "Warning should be on stderr. stderr: {stderr}"
    );

    assert!(
        !stdout.contains("Warning:") && !stdout.contains("deprecated"),
        "Warning should NOT be on stdout. stdout: {stdout}"
    );

    Ok(())
}
