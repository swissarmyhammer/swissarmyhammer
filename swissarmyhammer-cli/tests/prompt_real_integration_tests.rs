use anyhow::Result;
use swissarmyhammer::test_utils::IsolatedTestEnvironment;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process;

/// Real CLI integration tests that replace MockMcpClient tests
/// These tests use actual CLI commands and real prompt handling instead of mocks

#[tokio::test]
async fn test_prompt_list_command() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    let result = run_sah_command_in_process(&["prompt", "list"]).await?;

    assert_eq!(result.exit_code, 0, "prompt list should succeed");
    assert!(!result.stdout.is_empty(), "should have output");

    Ok(())
}

#[tokio::test]
async fn test_prompt_help_command() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    let result = run_sah_command_in_process(&["prompt", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "prompt help should succeed");
    assert!(result.stdout.contains("prompt"));

    Ok(())
}

#[tokio::test]
async fn test_concurrent_prompt_commands() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    // Spawn multiple concurrent prompt list commands
    let mut handles = vec![];

    for _i in 0..3 {
        let handle =
            tokio::spawn(async move { run_sah_command_in_process(&["prompt", "list"]).await });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let result = handle.await??;
        assert_eq!(result.exit_code, 0, "concurrent prompt list should succeed");
    }

    Ok(())
}

#[tokio::test]
async fn test_prompt_command_validation() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new()?;

    // Test invalid subcommand
    let result = run_sah_command_in_process(&["prompt", "invalid-subcommand"]).await?;
    assert_ne!(result.exit_code, 0, "should fail for invalid subcommand");

    Ok(())
}
