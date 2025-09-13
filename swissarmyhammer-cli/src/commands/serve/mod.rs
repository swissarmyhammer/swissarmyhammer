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
use crate::context::CliContext;

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
pub async fn handle_command(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
) -> i32 {
    // Check for HTTP subcommand
    match matches.subcommand() {
        Some(("http", http_matches)) => handle_http_serve(http_matches, cli_context).await,
        None => {
            // Default to stdio mode (existing behavior)
            handle_stdio_serve(cli_context).await
        }
        Some((unknown, _)) => {
            eprintln!("Unknown serve subcommand: {}", unknown);
            EXIT_ERROR
        }
    }
}

/// Handle HTTP serve mode
async fn handle_http_serve(matches: &clap::ArgMatches, cli_context: &CliContext) -> i32 {
    use crate::signal_handler::wait_for_shutdown;
    use swissarmyhammer_tools::mcp::start_http_server;

    // Parse port and host arguments from CLI
    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);

    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    let bind_addr = format!("{}:{}", host, port);

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

    let server_handle = match start_http_server(&bind_addr).await {
        Ok(handle) => {
            let actual_port = handle.port();
            let running_message = if port == 0 {
                format!("✅ MCP HTTP server running on {} (bound to random port: {}). 💡 Use Ctrl+C to stop.", handle.url(), actual_port)
            } else {
                format!("✅ MCP HTTP server running on {}. 💡 Use Ctrl+C to stop.", handle.url())
            };
            
            display_server_status(
                cli_context,
                "HTTP",
                "Running",
                &handle.url(),
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
        &server_handle.url(),
        Some(server_handle.port()),
        0,
        "🛑 Shutting down server...",
    );
    
    if let Err(e) = server_handle.shutdown().await {
        tracing::error!("Failed to shutdown server gracefully: {}", e);
        display_server_status(
            cli_context,
            "HTTP",
            "Error",
            &server_handle.url(),
            Some(server_handle.port()),
            0,
            &format!("Warning: Server shutdown error: {}", e),
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
        "✅ Server stopped",
    );

    EXIT_SUCCESS
}

/// Handle stdio serve mode (existing behavior)
async fn handle_stdio_serve(cli_context: &CliContext) -> i32 {
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;
    use swissarmyhammer_tools::McpServer;

    tracing::debug!("Starting MCP server in stdio mode");

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

    let server = match McpServer::new(library).await {
        Ok(server) => server,
        Err(e) => {
            tracing::error!("Failed to create MCP server: {}", e);
            eprintln!("Failed to create MCP server: {}", e);
            return EXIT_ERROR;
        }
    };

    // Initialize the server before starting
    if let Err(e) = server.initialize().await {
        tracing::error!("Failed to initialize MCP server: {}", e);
        eprintln!("Failed to initialize MCP server: {}", e);
        return EXIT_ERROR;
    }

    tracing::info!("MCP server initialized successfully");

    // Start the rmcp SDK server with stdio transport -- THINK before messing with this!
    let running_service = match serve_server(server, stdio()).await {
        Ok(service) => {
            tracing::info!("MCP server started successfully");
            service
        }
        Err(e) => {
            tracing::error!("MCP server error: {}", e);
            eprintln!("MCP server error: {}", e);
            return EXIT_WARNING;
        }
    };

    // Wait for the service to complete - this will return when:
    // - The client disconnects (transport closed)
    // - The server is cancelled
    // - A serious error occurs
    // THINK before messing with this, otherwise you can easily end up with a server that does not start.
    match running_service.waiting().await {
        Ok(quit_reason) => {
            // The QuitReason enum is not exported by rmcp, so we'll just log it
            tracing::info!("MCP server stopped: {:?}", quit_reason);
        }
        Err(e) => {
            tracing::error!("MCP server task error: {}", e);
            return EXIT_WARNING;
        }
    }

    tracing::info!("MCP server shutting down gracefully");
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
        let health_url = if let Some(p) = port {
            Some(format!("http://{}:{}/health", address.split(':').next().unwrap_or("127.0.0.1"), p))
        } else {
            None
        };
        
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
        assert!(DESCRIPTION.contains("Serve Command"));
        assert!(DESCRIPTION.contains("MCP server"));
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
        ).await.expect("Failed to create CliContext");

        // We can verify the signature compiles and matches expected pattern
        let _result: std::pin::Pin<Box<dyn std::future::Future<Output = i32>>> =
            Box::pin(handle_command(&matches, &cli_context));
    }
}
