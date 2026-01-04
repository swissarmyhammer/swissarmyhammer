//! Echo server using stdio transport
//!
//! This example shows how to run the EchoService with stdio transport,
//! following the pattern from rmcp's counter_stdio.rs example.

use anyhow::Result;
use llama_agent::echo::EchoService;
use rmcp::{transport::stdio, ServiceExt};
use swissarmyhammer_common::Pretty;
use tracing_subscriber::{self, EnvFilter};

/// Example usage:
/// cargo run --example echo_stdio
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Echo MCP server with stdio transport");

    // Create an instance of our echo service and serve with stdio transport
    let service = EchoService::new().serve(stdio()).await.map_err(|e| {
        tracing::error!("serving error: {}", Pretty(&e));
        e
    })?;

    // Wait for the service to complete
    service.waiting().await?;

    Ok(())
}
