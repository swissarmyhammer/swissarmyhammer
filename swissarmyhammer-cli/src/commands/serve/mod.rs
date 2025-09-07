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

/// Handle HTTP serve mode
async fn handle_http_serve(matches: &clap::ArgMatches) -> i32 {
    use crate::signal_handler::wait_for_shutdown;
    use swissarmyhammer_tools::mcp::start_http_server;

    // Parse port and host arguments from CLI
    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);

    let host = matches
        .get_one::<String>("host")
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    let bind_addr = format!("{}:{}", host, port);

    println!("Starting SwissArmyHammer MCP server on {}", bind_addr);

    let server_handle = match start_http_server(&bind_addr).await {
        Ok(handle) => {
            println!("✅ MCP HTTP server running on {}", handle.url());
            println!("💡 Use Ctrl+C to stop the server");
            println!("🔍 Health check: {}/health", handle.url());
            if port == 0 {
                println!("📍 Server bound to random port: {}", handle.port());
            }
            handle
        }
        Err(e) => {
            tracing::error!("Failed to start HTTP MCP server: {}", e);
            eprintln!("Failed to start HTTP MCP server: {}", e);
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

/// Handle stdio serve mode (existing behavior)
async fn handle_stdio_serve() -> i32 {
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;
    use swissarmyhammer::PromptLibrary;
    use swissarmyhammer_tools::McpServer;

    tracing::debug!("Starting MCP server in stdio mode");

    // Create library and server
    let library = PromptLibrary::new();
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
