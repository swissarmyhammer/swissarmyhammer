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
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::unified_server::McpServerHandle;

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
    let model_override = cli_context
        .matches
        .get_one::<String>("agent")
        .map(|s| s.to_string());

    // Check for HTTP subcommand
    match matches.subcommand() {
        Some(("http", http_matches)) => {
            handle_http_serve(http_matches, cli_context, model_override).await
        }
        None => {
            // Default to stdio mode (existing behavior)
            handle_stdio_serve(cli_context, model_override).await
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
    model_override: Option<String>,
) -> i32 {
    let server_handle = match initialize_http_server(matches, cli_context, model_override).await {
        Ok(handle) => handle,
        Err(exit_code) => return exit_code,
    };

    manage_http_server_lifecycle(cli_context, server_handle).await
}

/// Initialize HTTP server and return handle or error exit code
async fn initialize_http_server(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
    model_override: Option<String>,
) -> Result<McpServerHandle, i32> {
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);
    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    let bind_addr = format!("{}:{}", host, port);

    if host != "127.0.0.1" {
        eprintln!(
            "Warning: Custom host '{}' not yet supported by unified server, using 127.0.0.1",
            host
        );
    }

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

    let mode = McpServerMode::Http { port: Some(port) };
    let server_handle = start_mcp_server(mode, None, model_override, None)
        .await
        .map_err(|e| {
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
            EXIT_ERROR
        })?;

    display_http_server_running_status(cli_context, &server_handle, port);
    Ok(server_handle)
}

/// Display running status for HTTP server
fn display_http_server_running_status(
    cli_context: &CliContext,
    handle: &McpServerHandle,
    requested_port: u16,
) {
    let actual_port = handle.port().unwrap_or(requested_port);
    let running_message =
        format_http_server_running_message(handle.url(), requested_port, actual_port);

    display_server_status(
        cli_context,
        "HTTP",
        "Running",
        handle.url(),
        Some(actual_port),
        0,
        &running_message,
    );
}

/// Format the running message for HTTP server
fn format_http_server_running_message(url: &str, requested_port: u16, actual_port: u16) -> String {
    if requested_port == 0 {
        format!(
            "✓ MCP HTTP server running on {} (bound to random port: {}). 💡 Use Ctrl+C to stop.",
            url, actual_port
        )
    } else {
        format!(
            "✓ MCP HTTP server running on {}. 💡 Use Ctrl+C to stop.",
            url
        )
    }
}

/// Manage HTTP server lifecycle including shutdown
async fn manage_http_server_lifecycle(
    cli_context: &CliContext,
    mut server_handle: McpServerHandle,
) -> i32 {
    use crate::signal_handler::wait_for_shutdown;

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
async fn handle_stdio_serve(cli_context: &CliContext, model_override: Option<String>) -> i32 {
    let (library, prompt_count) = match initialize_prompt_library(cli_context) {
        Ok(result) => result,
        Err(exit_code) => return exit_code,
    };

    let server_handle =
        match start_stdio_server(cli_context, library, prompt_count, model_override).await {
            Ok(handle) => handle,
            Err(exit_code) => return exit_code,
        };

    handle_stdio_server_shutdown(server_handle).await
}

/// Initialize prompt library for stdio mode
fn initialize_prompt_library(cli_context: &CliContext) -> Result<(PromptLibrary, usize), i32> {
    tracing::debug!("Starting unified MCP server in stdio mode");

    let library = cli_context.get_prompt_library().map_err(|e| {
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
        EXIT_ERROR
    })?;

    let prompt_count = library.list().map(|p| p.len()).unwrap_or(0);
    tracing::debug!("Loaded {} prompts for MCP server", prompt_count);

    Ok((library, prompt_count))
}

/// Start stdio server and return handle or error exit code
async fn start_stdio_server(
    cli_context: &CliContext,
    library: PromptLibrary,
    prompt_count: usize,
    model_override: Option<String>,
) -> Result<McpServerHandle, i32> {
    use swissarmyhammer_tools::mcp::{start_mcp_server, McpServerMode};

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
    start_mcp_server(mode, Some(library), model_override, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to start unified stdio MCP server: {}", e);
            eprintln!("Failed to start unified stdio MCP server: {}", e);
            EXIT_ERROR
        })
}

/// Handle stdio server shutdown and completion
async fn handle_stdio_server_shutdown(mut server_handle: McpServerHandle) -> i32 {
    wait_for_stdio_server_termination(&mut server_handle).await;
    finalize_stdio_server_shutdown(server_handle).await
}

/// Wait for server termination via signal or natural completion
async fn wait_for_stdio_server_termination(server_handle: &mut McpServerHandle) {
    use crate::signal_handler::wait_for_shutdown;

    let mut completion_rx = server_handle.take_completion_rx();

    tokio::select! {
        _ = wait_for_shutdown() => {
            handle_shutdown_signal(server_handle).await;
        }
        _ = wait_for_natural_completion(completion_rx.as_mut()) => {
            tracing::info!("Server completed naturally (EOF on stdin)");
        }
    }
}

/// Handle shutdown signal for stdio server
async fn handle_shutdown_signal(server_handle: &mut McpServerHandle) {
    tracing::info!("Received shutdown signal (SIGTERM/CTRL+C)");
    if let Err(e) = server_handle.shutdown().await {
        tracing::warn!("Error sending shutdown signal: {}", e);
    }
}

/// Wait for natural completion of server
async fn wait_for_natural_completion(
    completion_rx: Option<&mut tokio::sync::oneshot::Receiver<()>>,
) {
    if let Some(rx) = completion_rx {
        let _ = rx.await;
    } else {
        std::future::pending::<()>().await
    }
}

/// Finalize server shutdown and return exit code
async fn finalize_stdio_server_shutdown(mut server_handle: McpServerHandle) -> i32 {
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
    if !cli_context.verbose {
        display_basic_server_status(cli_context, server_type, status, address, message);
        return;
    }

    display_verbose_server_status(
        cli_context,
        server_type,
        status,
        address,
        port,
        prompt_count,
        message,
    );
}

/// Display basic server status
fn display_basic_server_status(
    cli_context: &CliContext,
    server_type: &str,
    status: &str,
    address: &str,
    message: &str,
) {
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

/// Display verbose server status with additional details
fn display_verbose_server_status(
    cli_context: &CliContext,
    server_type: &str,
    status: &str,
    address: &str,
    port: Option<u16>,
    prompt_count: usize,
    message: &str,
) {
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
