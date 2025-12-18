//! Agent fixtures for conformance testing
//!
//! This module provides factories for creating agent instances that implement
//! the Agent trait and are connected via streams for in-process testing.

use agent_client_protocol::{Agent, AgentSideConnection, ClientSideConnection};
use std::sync::Arc;

/// Result type for agent creation
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Creates a test claude-agent instance connected via streams
///
/// For conformance testing, this uses playback mode to avoid calling the real Claude API.
/// Set CLAUDE_AGENT_MODE=record to record new fixtures.
#[allow(dead_code)]
pub async fn create_claude_agent() -> Result<impl Agent> {
    let mut config = claude_agent::config::AgentConfig::default();

    // Check for environment variable to override mode
    let mode = std::env::var("CLAUDE_AGENT_MODE").unwrap_or_else(|_| "playback".to_string());

    config.claude.mode = match mode.as_str() {
        "record" => {
            let output_path =
                std::env::current_dir()?.join("tests/fixtures/conformance_recording.json");
            claude_agent::config::ClaudeAgentMode::Record { output_path }
        }
        "playback" => {
            let input_path =
                std::env::current_dir()?.join("tests/fixtures/conformance_minimal.json");
            claude_agent::config::ClaudeAgentMode::Playback { input_path }
        }
        _ => claude_agent::config::ClaudeAgentMode::Normal,
    };

    let (agent, _receiver) = claude_agent::agent::ClaudeAgent::new(config)
        .await
        .map_err(|e| format!("Failed to create claude agent: {}", e))?;

    create_agent_connection(agent)
}

/// Creates a test llama-agent instance connected via streams
/// Always includes session mode support for full conformance testing
pub async fn create_llama_agent() -> Result<impl Agent> {
    let model_config = llama_agent::types::ModelConfig {
        source: llama_agent::types::ModelSource::Local {
            folder: std::env::temp_dir(),
            filename: Some("nonexistent.gguf".to_string()), // Won't load model for tests
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 1,
        n_threads_batch: 1,
        use_hf_params: false,
        retry_config: llama_agent::types::RetryConfig::default(),
        debug: false,
    };

    let agent_config = llama_agent::types::AgentConfig {
        model: model_config,
        queue_config: llama_agent::types::QueueConfig::default(),
        mcp_servers: Vec::new(),
        session_config: llama_agent::types::SessionConfig::default(),
        parallel_execution_config: llama_agent::types::ParallelConfig::default(),
    };

    let model_manager = Arc::new(
        llama_agent::model::ModelManager::new(agent_config.model.clone())
            .expect("Failed to create model manager"),
    );
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

    let agent_server = Arc::new(llama_agent::AgentServer::new(
        model_manager,
        request_queue,
        session_manager,
        mcp_client,
        chat_template,
        dependency_analyzer,
        agent_config,
    ));

    // Configure ACP server with session modes and filesystem access
    let mut acp_config = llama_agent::acp::AcpConfig::default();
    acp_config.available_modes = vec![
        agent_client_protocol::SessionMode::new("general-purpose", "General Purpose")
            .description("General-purpose agent for researching complex questions, searching for code, and executing multi-step tasks"),
        agent_client_protocol::SessionMode::new("statusline-setup", "Statusline Setup")
            .description("Configure the status line setting"),
        agent_client_protocol::SessionMode::new("Explore", "Explore")
            .description("Fast agent specialized for exploring codebases"),
        agent_client_protocol::SessionMode::new("Plan", "Plan")
            .description("Software architect agent for designing implementation plans"),
    ];
    acp_config.default_mode_id = "general-purpose".to_string();

    // Allow filesystem access to /tmp for conformance tests
    acp_config.filesystem.allowed_paths = vec![std::env::temp_dir()];

    let acp_server = llama_agent::acp::AcpServer::new(agent_server, acp_config);

    create_agent_connection(acp_server)
}

/// Helper to create client/agent connection with streams
fn create_agent_connection<A>(acp_server: A) -> Result<impl Agent>
where
    A: agent_client_protocol::Agent + 'static,
{
    // Create bidirectional streams using piper (provides futures AsyncRead/AsyncWrite)
    let (client_to_agent_rx, client_to_agent_tx) = piper::pipe(8192);
    let (agent_to_client_rx, agent_to_client_tx) = piper::pipe(8192);

    // Create dummy client for receiving agent requests
    #[derive(Clone)]
    struct DummyClient;

    #[async_trait::async_trait(?Send)]
    impl agent_client_protocol::Client for DummyClient {
        async fn request_permission(
            &self,
            _request: agent_client_protocol::RequestPermissionRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::RequestPermissionResponse>
        {
            Ok(agent_client_protocol::RequestPermissionResponse::new(
                agent_client_protocol::RequestPermissionOutcome::Cancelled,
            ))
        }

        async fn session_notification(
            &self,
            _notification: agent_client_protocol::SessionNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }

        async fn ext_method(
            &self,
            _request: agent_client_protocol::ExtRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::ExtResponse> {
            Err(agent_client_protocol::Error::method_not_found())
        }

        async fn ext_notification(
            &self,
            _notification: agent_client_protocol::ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    let spawn = |fut: futures::future::LocalBoxFuture<'static, ()>| {
        tokio::task::spawn_local(fut);
    };

    // Create client connection to the agent
    let (client_conn, client_io_task) =
        ClientSideConnection::new(DummyClient, client_to_agent_tx, agent_to_client_rx, spawn);

    // Create agent connection (wrapping our ACP server)
    let (_agent_conn, agent_io_task) =
        AgentSideConnection::new(acp_server, agent_to_client_tx, client_to_agent_rx, spawn);

    // Spawn IO tasks
    tokio::task::spawn_local(client_io_task);
    tokio::task::spawn_local(agent_io_task);

    Ok(client_conn)
}
