//! ACP server with stdio transport
//!
//! This example demonstrates how to run an ACP server using stdio transport,
//! which is the standard way to integrate with code editors like Zed and JetBrains IDEs.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example acp_stdio --features acp
//! ```
//!
//! The server will:
//! - Read JSON-RPC requests from stdin
//! - Write JSON-RPC responses and notifications to stdout
//! - Log debug information to stderr
//!
//! # Editor Integration
//!
//! To integrate with an editor, configure it to spawn this binary and communicate
//! via stdin/stdout using the Agent Client Protocol.

use anyhow::Result;

use llama_agent::acp::{AcpConfig, AcpServer};

use llama_agent::agent::AgentServer;

use llama_agent::types::{
    AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
};

use std::sync::Arc;

use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr (stdout is reserved for JSON-RPC)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting ACP server with stdio transport");

    // Load ACP configuration
    let acp_config = AcpConfig::default();

    // Create a minimal agent configuration
    // In production, you would load this from a config file or environment
    let model_config = ModelConfig {
        source: ModelSource::HuggingFace {
            repo: "unsloth/Qwen3-1.7B-GGUF".to_string(),
            filename: Some("Qwen3-1.7B-IQ4_NL.gguf".to_string()),
            folder: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: num_cpus::get() as i32,
        n_threads_batch: num_cpus::get() as i32,
        use_hf_params: true,
        retry_config: RetryConfig::default(),
        debug: false,
    };

    let agent_config = AgentConfig {
        model: model_config.clone(),
        queue_config: QueueConfig::default(),
        mcp_servers: Vec::new(),
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    tracing::info!("Initializing agent server components...");

    // Create all the components needed for AgentServer
    let model_manager = Arc::new(llama_agent::model::ModelManager::new(model_config)?);
    let request_queue = Arc::new(llama_agent::queue::RequestQueue::new(
        model_manager.clone(),
        agent_config.queue_config.clone(),
        agent_config.session_config.clone(),
    ));
    let session_manager = Arc::new(llama_agent::session::SessionManager::new(
        agent_config.session_config.clone(),
    ));
    let mcp_client: Arc<dyn llama_agent::mcp::MCPClient> =
        Arc::new(llama_agent::mcp::NoOpMCPClient::new());
    let chat_template = Arc::new(llama_agent::chat_template::ChatTemplateEngine::new());
    let dependency_analyzer = Arc::new(llama_agent::dependency_analysis::DependencyAnalyzer::new(
        agent_config.parallel_execution_config.clone(),
    ));

    tracing::info!("Creating agent server...");

    // Create the agent server
    let agent_server = Arc::new(AgentServer::new(
        model_manager,
        request_queue,
        session_manager,
        mcp_client,
        chat_template,
        dependency_analyzer,
        agent_config,
    ));

    tracing::info!("Agent server initialized successfully");

    // Create the ACP server
    let acp_server = Arc::new(AcpServer::new(agent_server, acp_config).0);

    tracing::info!("Starting ACP protocol server on stdio...");

    // Start the server with stdio transport
    // This will block until the client closes stdin
    acp_server.start_stdio().await?;

    tracing::info!("ACP server shutdown complete");

    Ok(())
}
