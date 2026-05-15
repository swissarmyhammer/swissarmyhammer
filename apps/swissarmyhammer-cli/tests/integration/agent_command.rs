//! Integration tests for agent command structure and execution

use anyhow::Result;

use crate::in_process_test_utils::run_sah_command_in_process;

/// Test that agent command exists and shows help
#[tokio::test]
async fn test_agent_command_help() -> Result<()> {
    let result = run_sah_command_in_process(&["agent", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "agent --help should succeed");
    assert!(
        result.stdout.contains("agent") || result.stdout.contains("Agent"),
        "help output should mention agent"
    );
    assert!(
        result.stdout.contains("acp"),
        "help output should mention acp subcommand"
    );

    Ok(())
}

/// Test that agent acp subcommand exists and shows help
#[tokio::test]
async fn test_agent_acp_help() -> Result<()> {
    let result = run_sah_command_in_process(&["agent", "acp", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "agent acp --help should succeed");
    assert!(
        result.stdout.contains("acp") || result.stdout.contains("ACP"),
        "help output should mention ACP"
    );
    assert!(
        result.stdout.contains("config"),
        "help output should mention config option"
    );

    Ok(())
}

/// Test that agent acp command can be parsed (without actually starting server)
#[tokio::test]
async fn test_agent_acp_command_parsing() -> Result<()> {
    // Test basic command structure - we expect it to fail because we're not in stdio mode
    // but it should parse successfully and attempt to run
    let result = run_sah_command_in_process(&["agent", "acp"]).await;

    // The command should parse successfully (may fail at runtime due to missing dependencies)
    assert!(
        result.is_ok(),
        "agent acp command should parse successfully"
    );

    Ok(())
}

/// Test that agent acp with config flag can be parsed
#[tokio::test]
async fn test_agent_acp_with_config_parsing() -> Result<()> {
    use tempfile::TempDir;

    let temp_dir = TempDir::new()?;
    let config_file = temp_dir.path().join("acp.yaml");

    // Create a minimal valid ACP config
    std::fs::write(
        &config_file,
        r#"---
# ACP configuration for testing
"#,
    )?;

    // Test command parsing with config file
    let result =
        run_sah_command_in_process(&["agent", "acp", "--config", config_file.to_str().unwrap()])
            .await;

    // The command should parse successfully
    assert!(
        result.is_ok(),
        "agent acp --config should parse successfully"
    );

    Ok(())
}

/// Test agent command with no subcommand shows error
#[tokio::test]
async fn test_agent_no_subcommand() -> Result<()> {
    let result = run_sah_command_in_process(&["agent"]).await?;

    assert_ne!(
        result.exit_code, 0,
        "agent with no subcommand should return error"
    );
    assert!(
        result.stderr.contains("No subcommand") || result.stderr.contains("subcommand"),
        "should show error about missing subcommand"
    );

    Ok(())
}

/// Test agent with invalid subcommand
#[tokio::test]
async fn test_agent_invalid_subcommand() -> Result<()> {
    let result = run_sah_command_in_process(&["agent", "invalid"]).await?;

    assert_ne!(
        result.exit_code, 0,
        "agent with invalid subcommand should fail"
    );

    Ok(())
}

/// Test that main help includes agent command
#[tokio::test]
async fn test_main_help_includes_agent() -> Result<()> {
    let result = run_sah_command_in_process(&["--help"]).await?;

    assert_eq!(result.exit_code, 0, "main help should succeed");
    assert!(
        result.stdout.contains("agent"),
        "main help should mention agent command"
    );

    Ok(())
}
