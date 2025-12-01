//! Serve command implementation
//!
//! Starts the SwissArmyHammer MCP (Model Context Protocol) server for AI tool integration.
//!
//! This module provides the serve command which starts an MCP server that exposes
//! SwissArmyHammer tools and capabilities through the Model Context Protocol.
//! This enables integration with AI applications like Claude Code.
//!
//! # Features
//!
//! - Tool integration through MCP protocol
//! - Stdio transport for client communication
//! - Graceful shutdown handling
//! - Comprehensive logging and error handling
//! - Integration with SwissArmyHammer tool ecosystem

use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};

pub mod display;

/// Help text for the serve command
#[cfg(test)]
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the serve command
///
/// Starts the MCP server with stdio or HTTP transport based on subcommands.
/// The server runs in blocking mode until the client disconnects or an error occurs.
///
/// # Arguments
///
/// * `matches` - Command line arguments for serve command and subcommands
/// * `cli_context` - CLI context with configuration and global arguments
///
/// # Returns
///
/// Returns an exit code:
/// - 0: Server started and stopped successfully
/// - 1: Server encountered warnings or stopped unexpectedly
/// - 2: Server failed to start or encountered critical errors
pub async fn handle_command(matches: &clap::ArgMatches, cli_context: &CliContext) -> i32 {
    // Extract global --agent flag from root matches
    let agent_override = cli_context
        .matches
        .get_one::<String>("agent")
        .map(|s| s.to_string());

    // Check for HTTP subcommand
    match matches.subcommand() {
        Some(("http", http_matches)) => {
            handle_http_serve(http_matches, cli_context, agent_override).await
        }
        None => {
            // Default to stdio mode (existing behavior)
            handle_stdio_serve(cli_context, agent_override).await
        }
        Some((unknown, _)) => {
            eprintln!("Unknown serve subcommand: {}", unknown);
            EXIT_ERROR
        }
    }
}

/// Handle HTTP serve mode
async fn handle_http_serve(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
    agent_override: Option<String>,
) -> i32 {
    use crate::signal_handler::wait_for_shutdown;
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

    // Parse port and host arguments from CLI
    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);
    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    let bind_addr = format!("{}:{}", host, port);
    let mode = McpServerMode::Http { port: Some(port) };

    // Note: unified server currently only supports 127.0.0.1, host parameter ignored for now
    if host != "127.0.0.1" {
        eprintln!(
            "Warning: Custom host '{}' not yet supported by unified server, using 127.0.0.1",
            host
        );
    }

    // Display starting status
    display_server_status(
        cli_context,
        "HTTP",
        "Starting",
        &bind_addr,
        Some(port),
        0,
        &format!("Starting SwissArmyHammer MCP server on {}", bind_addr),
    );

    println!(
        "Starting SwissArmyHammer MCP server on 127.0.0.1:{}",
        if port == 0 {
            "random port".to_string()
        } else {
            port.to_string()
        }
    );

    let mut server_handle = match start_mcp_server(mode, None, agent_override).await {
        Ok(handle) => {
            let actual_port = handle.port().unwrap_or(port);
            let running_message = if port == 0 {
                format!("✓ MCP HTTP server running on {} (bound to random port: {}). 💡 Use Ctrl+C to stop.", handle.url(), actual_port)
            } else {
                format!(
                    "✓ MCP HTTP server running on {}. 💡 Use Ctrl+C to stop.",
                    handle.url()
                )
            };

            display_server_status(
                cli_context,
                "HTTP",
                "Running",
                handle.url(),
                Some(actual_port),
                0, // Will be updated when we use CliContext for prompt library
                &running_message,
            );
            handle
        }
        Err(e) => {
            tracing::error!("Failed to start HTTP MCP server: {}", e);
            display_server_status(
                cli_context,
                "HTTP",
                "Error",
                &bind_addr,
                Some(port),
                0,
                &format!("Failed to start HTTP MCP server: {}", e),
            );
            return EXIT_ERROR;
        }
    };

    // Wait for shutdown signal
    wait_for_shutdown().await;

    display_server_status(
        cli_context,
        "HTTP",
        "Stopping",
        server_handle.url(),
        server_handle.port(),
        0,
        "🛑 Shutting down server...",
    );

    if let Err(e) = server_handle.shutdown().await {
        tracing::error!("Failed to shutdown server gracefully: {}", e);
        display_server_status(
            cli_context,
            "HTTP",
            "Error",
            server_handle.url(),
            server_handle.port(),
            0,
            &format!("Warning: Server shutdown error: {}", e),
        );
        return EXIT_WARNING;
    }

    // Wait for server task to complete to prevent orphaned processes
    if let Err(e) = server_handle.wait_for_completion().await {
        tracing::error!("Error waiting for server task completion: {}", e);
        display_server_status(
            cli_context,
            "HTTP",
            "Error",
            "-",
            None,
            0,
            &format!("Warning: Server task completion error: {}", e),
        );
        return EXIT_WARNING;
    }

    display_server_status(
        cli_context,
        "HTTP",
        "Stopped",
        "-",
        None,
        0,
        "✓ Server stopped",
    );

    EXIT_SUCCESS
}

/// Handle stdio serve mode (existing behavior)
async fn handle_stdio_serve(cli_context: &CliContext, agent_override: Option<String>) -> i32 {
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

    tracing::debug!("Starting unified MCP server in stdio mode");

    // Get prompt library from CliContext
    let library = match cli_context.get_prompt_library() {
        Ok(lib) => lib,
        Err(e) => {
            tracing::error!("Failed to load prompts: {}", e);
            display_server_status(
                cli_context,
                "Stdio",
                "Error",
                "stdio",
                None,
                0,
                &format!("Failed to load prompts: {}", e),
            );
            return EXIT_ERROR;
        }
    };

    let prompt_count = library.list().map(|p| p.len()).unwrap_or(0);
    tracing::debug!("Loaded {} prompts for MCP server", prompt_count);

    // Display starting status for stdio mode (only in verbose mode)
    if cli_context.verbose {
        display_server_status(
            cli_context,
            "Stdio",
            "Starting",
            "stdio",
            None,
            prompt_count,
            "Starting MCP server in stdio mode",
        );
    }

    let mode = McpServerMode::Stdio;
    let mut server_handle = match start_mcp_server(mode, Some(library), agent_override).await {
        Ok(handle) => handle,
        Err(e) => {
            tracing::error!("Failed to start unified stdio MCP server: {}", e);
            eprintln!("Failed to start unified stdio MCP server: {}", e);
            return EXIT_ERROR;
        }
    };

    // Wait for either shutdown signal or natural server completion (EOF on stdin)
    use crate::signal_handler::wait_for_shutdown;

    // The server task will complete when:
    // 1. EOF is received on stdin (natural exit) - signals via completion_rx
    // 2. Shutdown signal is sent (via shutdown_tx) - triggered by SIGTERM/CTRL+C
    //
    // The tokio::select! macro will complete as soon as ONE of these branches resolves:
    // - Signal branch: Triggered by SIGTERM/CTRL+C, sends shutdown to server task
    // - Completion branch: Triggered by EOF on stdin, indicates server already exited
    //
    // This ensures the process exits cleanly in both scenarios without requiring
    // both events to occur (avoiding the hang when EOF occurs but no signal arrives).
    let mut completion_rx = server_handle.take_completion_rx();

    tokio::select! {
        _ = wait_for_shutdown() => {
            tracing::info!("Received shutdown signal (SIGTERM/CTRL+C)");
            // Signal the server to shut down
            if let Err(e) = server_handle.shutdown().await {
                tracing::warn!("Error sending shutdown signal: {}", e);
            }
        }
        _ = async {
            if let Some(rx) = completion_rx.as_mut() {
                let _ = rx.await;
            } else {
                // If no completion receiver, wait forever (shouldn't happen for stdio mode)
                std::future::pending::<()>().await
            }
        } => {
            tracing::info!("Server completed naturally (EOF on stdin)");
            // No need to call shutdown() here - server already exited
        }
    }

    // Wait for server task to complete to prevent orphaned processes
    if let Err(e) = server_handle.wait_for_completion().await {
        tracing::error!("Error waiting for server task completion: {}", e);
        return EXIT_ERROR;
    }

    tracing::info!("MCP stdio server completed successfully");
    EXIT_SUCCESS
}

/// Helper function to display server status based on verbose flag
fn display_server_status(
    cli_context: &CliContext,
    server_type: &str,
    status: &str,
    address: &str,
    port: Option<u16>,
    prompt_count: usize,
    message: &str,
) {
    if cli_context.verbose {
        let health_url = port.map(|p| {
            format!(
                "http://{}:{}/health",
                address.split(':').next().unwrap_or("127.0.0.1"),
                p
            )
        });

        let verbose_status = vec![display::VerboseServerStatus::new(
            server_type.to_string(),
            status.to_string(),
            address.to_string(),
            port,
            health_url,
            prompt_count,
            message.to_string(),
        )];

        if let Err(e) = cli_context.display(verbose_status) {
            eprintln!("Failed to display status: {}", e);
        }
    } else {
        let basic_status = vec![display::ServerStatus::new(
            server_type.to_string(),
            status.to_string(),
            address.to_string(),
            message.to_string(),
        )];

        if let Err(e) = cli_context.display(basic_status) {
            eprintln!("Failed to display status: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    #[test]
    fn test_description_content() {
        assert!(DESCRIPTION.contains("MCP server"));
        assert!(DESCRIPTION.contains("Bridge AI"));
        assert!(
            DESCRIPTION.len() > 100,
            "Description should be comprehensive"
        );
    }

    #[tokio::test]
    async fn test_handle_command_signature() {
        use crate::cli::OutputFormat;
        use crate::context::CliContext;

        // This test just verifies that the function signature matches expected pattern
        let app = Command::new("test").arg(Arg::new("test").long("test"));
        let matches = app.try_get_matches_from(vec!["test"]).unwrap();

        // Create a test CliContext
        let template_context = swissarmyhammer_config::TemplateContext::new();
        let cli_context = CliContext::new(
            template_context,
            OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches.clone(),
        )
        .await
        .expect("Failed to create CliContext");

        // We can verify the signature compiles and matches expected pattern
        let _result: std::pin::Pin<Box<dyn std::future::Future<Output = i32>>> =
            Box::pin(handle_command(&matches, &cli_context));
    }
}
