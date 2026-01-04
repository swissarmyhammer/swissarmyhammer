//! Integration tests for CLI serve HTTP subcommand parsing and execution

use anyhow::Result;

use crate::in_process_test_utils::run_sah_command_in_process;

/// Test that the serve http command with default arguments works
#[tokio::test]
async fn test_serve_http_command_default() -> Result<()> {
    let result = run_sah_command_in_process(&["serve", "http", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "serve http --help should succeed");

    // Check that the help text contains expected elements
    assert!(
        result.stdout.contains("Start HTTP MCP server"),
        "Help should describe HTTP server"
    );
    assert!(
        result.stdout.contains("--port"),
        "Help should show --port option"
    );
    assert!(
        result.stdout.contains("--host"),
        "Help should show --host option"
    );

    Ok(())
}

/// Test that serve http command parses port argument correctly
#[tokio::test]
async fn test_serve_http_command_with_port() -> Result<()> {
    // Test that the command accepts the port argument
    // Note: We can't actually start the server in tests due to port conflicts,
    // but we can test that argument parsing works by checking for proper error messages
    let result = run_sah_command_in_process(&["serve", "http", "--port", "9999", "--help"]).await?;

    // Help should still work with additional arguments
    assert_eq!(
        result.exit_code, 0,
        "serve http --port 9999 --help should succeed"
    );

    Ok(())
}

/// Test that serve http command parses host argument correctly  
#[tokio::test]
async fn test_serve_http_command_with_host() -> Result<()> {
    let result =
        run_sah_command_in_process(&["serve", "http", "--host", "0.0.0.0", "--help"]).await?;

    assert_eq!(
        result.exit_code, 0,
        "serve http --host 0.0.0.0 --help should succeed"
    );

    Ok(())
}

/// Test that serve http command parses both port and host arguments
#[tokio::test]
async fn test_serve_http_command_with_port_and_host() -> Result<()> {
    let result = run_sah_command_in_process(&[
        "serve",
        "http",
        "--port",
        "8080",
        "--host",
        "127.0.0.1",
        "--help",
    ])
    .await?;

    assert_eq!(
        result.exit_code, 0,
        "serve http with both port and host should succeed"
    );

    Ok(())
}

/// Test that serve http command rejects invalid port values
#[tokio::test]
async fn test_serve_http_command_invalid_port() -> Result<()> {
    let result = run_sah_command_in_process(&["serve", "http", "--port", "99999"]).await?;

    // Should fail with invalid port (> 65535)
    assert_ne!(
        result.exit_code, 0,
        "serve http with invalid port should fail"
    );

    Ok(())
}

/// Test that serve http command accepts random port (port 0)
#[tokio::test]
async fn test_serve_http_command_random_port() -> Result<()> {
    let result = run_sah_command_in_process(&["serve", "http", "--port", "0", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "serve http --port 0 should succeed");

    Ok(())
}

/// Test the basic serve command (stdio mode) still works
#[tokio::test]
async fn test_serve_command_stdio_mode() -> Result<()> {
    let result = run_sah_command_in_process(&["serve", "--help"]).await?;

    assert_eq!(result.exit_code, 0, "serve --help should succeed");

    assert!(
        result.stdout.contains("Run as MCP server"),
        "Help should describe MCP server"
    );
    assert!(
        result.stdout.contains("http"),
        "Help should mention HTTP subcommand"
    );

    Ok(())
}

/// Test that short argument forms work for HTTP serve
#[tokio::test]
async fn test_serve_http_command_short_args() -> Result<()> {
    let result =
        run_sah_command_in_process(&["serve", "http", "-p", "8080", "-H", "localhost", "--help"])
            .await?;

    assert_eq!(
        result.exit_code, 0,
        "serve http with short args should succeed"
    );

    Ok(())
}

/// Test argument validation - ensure port must be numeric
#[tokio::test]
async fn test_serve_http_command_non_numeric_port() -> Result<()> {
    let result = run_sah_command_in_process(&["serve", "http", "--port", "not-a-number"]).await?;

    // Should fail with non-numeric port
    assert_ne!(
        result.exit_code, 0,
        "serve http with non-numeric port should fail"
    );

    // Check that error message mentions the invalid port
    assert!(
        result.stderr.to_lowercase().contains("invalid")
            || result.stderr.to_lowercase().contains("error"),
        "Error message should indicate invalid port"
    );

    Ok(())
}
