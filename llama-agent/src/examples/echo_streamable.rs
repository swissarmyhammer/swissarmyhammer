//! Echo server using stdio transport (streamable HTTP not yet working)

use llama_agent::echo::EchoService;
use rmcp::ServiceExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Example usage:
/// cargo run --example echo_streamable
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Echo server with stdio transport");

    // Use stdio transport for now
    let service = EchoService::new().serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    Ok(())
}
