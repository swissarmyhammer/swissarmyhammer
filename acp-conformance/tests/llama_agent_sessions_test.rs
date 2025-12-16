//! Integration tests for llama-agent ACP session setup conformance
//!
//! These tests verify that llama-agent correctly implements the ACP session setup
//! protocol per https://agentclientprotocol.com/protocol/session-setup

use acp_conformance::sessions::*;
use agent_client_protocol::{Agent, AgentSideConnection, ClientSideConnection};
use std::sync::Arc;

/// Helper to create a test llama-agent instance
async fn create_test_agent() -> acp_conformance::Result<impl Agent> {
    let (client_to_agent_rx, client_to_agent_tx) = piper::pipe(8192);
    let (agent_to_client_rx, agent_to_client_tx) = piper::pipe(8192);

    let model_config = llama_agent::types::ModelConfig {
        source: llama_agent::types::ModelSource::Local {
            folder: std::env::temp_dir(),
            filename: Some("nonexistent.gguf".to_string()),
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

    let acp_config = llama_agent::acp::AcpConfig::default();
    let acp_server = llama_agent::acp::AcpServer::new(agent_server, acp_config);

    #[derive(Clone)]
    struct DummyClient;

    #[async_trait::async_trait(?Send)]
    impl agent_client_protocol::Client for DummyClient {
        async fn request_permission(
            &self,
            _request: agent_client_protocol::RequestPermissionRequest,
        ) -> agent_client_protocol::Result<agent_client_protocol::RequestPermissionResponse> {
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

    let (client_conn, client_io_task) = ClientSideConnection::new(
        DummyClient,
        client_to_agent_tx,
        agent_to_client_rx,
        spawn,
    );

    let (_agent_conn, agent_io_task) =
        AgentSideConnection::new(acp_server, agent_to_client_tx, client_to_agent_rx, spawn);

    tokio::task::spawn_local(client_io_task);
    tokio::task::spawn_local(agent_io_task);

    Ok(client_conn)
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_new_session_minimal() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_test_agent()
                .await
                .expect("Failed to create test agent");

            test_new_session_minimal(&agent)
                .await
                .expect("New session minimal should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_new_session_with_mcp() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_test_agent()
                .await
                .expect("Failed to create test agent");

            test_new_session_with_mcp(&agent)
                .await
                .expect("New session with MCP should succeed");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_session_ids_unique() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_test_agent()
                .await
                .expect("Failed to create test agent");

            test_session_ids_unique(&agent)
                .await
                .expect("Session IDs should be unique");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_load_nonexistent_session() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_test_agent()
                .await
                .expect("Failed to create test agent");

            test_load_nonexistent_session(&agent)
                .await
                .expect("Load nonexistent should fail correctly");
        })
        .await;
}

#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_llama_set_session_mode() {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = create_test_agent()
                .await
                .expect("Failed to create test agent");

            test_set_session_mode(&agent)
                .await
                .expect("Set session mode should succeed");
        })
        .await;
}
