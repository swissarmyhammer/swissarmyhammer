//! Integration tests for llama-agent ACP initialization conformance
//!
//! These tests verify that llama-agent correctly implements the ACP initialization
//! protocol per https://agentclientprotocol.com/protocol/initialization

use acp_conformance::initialization::*;
use agent_client_protocol::{Agent, AgentSideConnection, ClientSideConnection};
use std::sync::Arc;

/// Helper to create a test llama-agent instance connected via streams
async fn create_llama_test_agent() -> acp_conformance::Result<impl Agent> {
    // Create bidirectional streams using piper (provides futures AsyncRead/AsyncWrite)
    let (client_to_agent_rx, client_to_agent_tx) = piper::pipe(8192);
    let (agent_to_client_rx, agent_to_client_tx) = piper::pipe(8192);

    // Create minimal agent configuration
    let model_config = llama_agent::types::ModelConfig {
        source: llama_agent::types::ModelSource::Local {
            folder: std::env::temp_dir(),
            filename: Some("nonexistent.gguf".to_string()), // Won't load model for init tests
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

    // Create agent components
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

    // Create ACP server
    let acp_config = llama_agent::acp::AcpConfig::default();
    let acp_server = llama_agent::acp::AcpServer::new(agent_server, acp_config);

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

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_minimal_initialization() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_llama_test_agent()
                .await
                .expect("Failed to create llama test agent");

            test_minimal_initialization(&agent)
                .await
                .expect("Minimal initialization should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_full_capabilities() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_llama_test_agent()
                .await
                .expect("Failed to create llama test agent");

            test_full_capabilities_initialization(&agent)
                .await
                .expect("Full capabilities initialization should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_protocol_version() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_llama_test_agent()
                .await
                .expect("Failed to create llama test agent");

            test_protocol_version_negotiation(&agent)
                .await
                .expect("Protocol version negotiation should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_minimal_client_caps() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_llama_test_agent()
                .await
                .expect("Failed to create llama test agent");

            test_minimal_client_capabilities(&agent)
                .await
                .expect("Minimal client capabilities should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_initialize_idempotent() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_llama_test_agent()
                .await
                .expect("Failed to create llama test agent");

            acp_conformance::initialization::test_initialize_idempotent(&agent)
                .await
                .expect("Initialize idempotency test should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_with_client_info() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_llama_test_agent()
                .await
                .expect("Failed to create llama test agent");

            acp_conformance::initialization::test_with_client_info(&agent)
                .await
                .expect("Client info test should succeed");
        })
        .await;
}
