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

use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};

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
/// * `_template_context` - Template context (currently unused)
///
/// # Returns
///
/// Returns an exit code:
/// - 0: Server started and stopped successfully
/// - 1: Server encountered warnings or stopped unexpectedly
/// - 2: Server failed to start or encountered critical errors
pub async fn handle_command(
    matches: &clap::ArgMatches,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> i32 {
    // Check for HTTP subcommand
    match matches.subcommand() {
        Some(("http", http_matches)) => handle_http_serve(http_matches).await,
        None => {
            // Default to stdio mode (existing behavior)
            handle_stdio_serve().await
        }
        Some((unknown, _)) => {
            eprintln!("Unknown serve subcommand: {}", unknown);
            EXIT_ERROR
        }
    }
}

/// Handle HTTP serve mode using unified MCP server
async fn handle_http_serve(matches: &clap::ArgMatches) -> i32 {
    use crate::signal_handler::wait_for_shutdown;
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

    // Parse port and host arguments from CLI
    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);
    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    // Note: unified server currently only supports 127.0.0.1, host parameter ignored for now
    if host != "127.0.0.1" {
        eprintln!("Warning: Custom host '{}' not yet supported by unified server, using 127.0.0.1", host);
    }

    let mode = McpServerMode::Http {
        port: if port == 0 { None } else { Some(port) }
    };

    println!("Starting SwissArmyHammer MCP server on 127.0.0.1:{}", if port == 0 { "random port".to_string() } else { port.to_string() });

    let mut server_handle = match start_mcp_server(mode, None).await {
        Ok(handle) => {
            println!("✅ MCP HTTP server running on {}", handle.url());
            println!("💡 Use Ctrl+C to stop the server");
            if let Some(actual_port) = handle.port() {
                if port == 0 {
                    println!("📍 Server bound to random port: {}", actual_port);
                }
            }
            handle
        }
        Err(e) => {
            tracing::error!("Failed to start unified HTTP MCP server: {}", e);
            eprintln!("Failed to start unified HTTP MCP server: {}", e);
            return EXIT_ERROR;
        }
    };

    // Wait for shutdown signal
    wait_for_shutdown().await;

    println!("🛑 Shutting down server...");
    if let Err(e) = server_handle.shutdown().await {
        tracing::error!("Failed to shutdown server gracefully: {}", e);
        eprintln!("Warning: Server shutdown error: {}", e);
        return EXIT_WARNING;
    }
    println!("✅ Server stopped");

    EXIT_SUCCESS
}

/// Handle stdio serve mode using unified MCP server
async fn handle_stdio_serve() -> i32 {
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};
    use crate::signal_handler::wait_for_shutdown;

    tracing::debug!("Starting unified MCP server in stdio mode");

    let mode = McpServerMode::Stdio;

    // Start the stdio server (runs in background)
    let mut server_handle = match start_mcp_server(mode, None).await {
        Ok(handle) => {
            tracing::info!("MCP stdio server started successfully");
            handle
        }
        Err(e) => {
            tracing::error!("Failed to start unified stdio MCP server: {}", e);
            eprintln!("Failed to start unified stdio MCP server: {}", e);
            return EXIT_ERROR;
        }
    };

    // Wait for shutdown signal or server completion
    wait_for_shutdown().await;

    // Shutdown server gracefully
    if let Err(e) = server_handle.shutdown().await {
        tracing::error!("Failed to shutdown server gracefully: {}", e);
        return EXIT_WARNING;
    }

    tracing::info!("MCP stdio server completed successfully");
    EXIT_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    #[test]
    fn test_description_content() {
        assert!(DESCRIPTION.contains("Serve Command"));
        assert!(DESCRIPTION.contains("MCP server"));
        assert!(
            DESCRIPTION.len() > 100,
            "Description should be comprehensive"
        );
    }

    #[test]
    fn test_handle_command_signature() {
        // This test just verifies that the function signature matches expected pattern
        let app = Command::new("test").arg(Arg::new("test").long("test"));
        let matches = app.try_get_matches_from(vec!["test"]).unwrap();

        // We can't easily test the actual async function without a full MCP setup,
        // but we can verify the signature compiles and matches expected pattern
        let test_context = swissarmyhammer_config::TemplateContext::new();
        let _result: std::pin::Pin<Box<dyn std::future::Future<Output = i32>>> =
            Box::pin(handle_command(&matches, &test_context));
    }
}
