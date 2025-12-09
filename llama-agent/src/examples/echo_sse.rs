//! Echo server using stdio transport (SSE not working)

use llama_agent::echo::EchoService;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting Echo MCP server with stdio transport");

    let service = EchoService::new().serve(rmcp::transport::stdio()).await?;

    service.waiting().await?;

    Ok(())
}
