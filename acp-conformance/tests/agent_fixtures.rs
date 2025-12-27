//! Agent fixtures for conformance testing
//!
//! One factory per agent type. All return Box<dyn AgentWithFixture>.
//! Tests call agent.use_fixture("test_name") to auto-configure record/playback.

use agent_client_protocol_extras::AgentWithFixture;

/// Result type for agent creation
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Llama agent factory for rstest
pub(crate) fn llama_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_llama_agent()
            .await
            .expect("Failed to create llama agent")
    })
}

/// Claude agent factory for rstest
pub(crate) fn claude_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_claude_agent()
            .await
            .expect("Failed to create claude agent")
    })
}

/// Create claude-agent for testing
async fn create_claude_agent() -> Result<Box<dyn AgentWithFixture>> {
    use agent_client_protocol_extras::{
        get_fixture_path_for, get_test_name_from_thread, start_test_mcp_server, RecordingAgent,
    };
    use tokio_stream::wrappers::BroadcastStream;

    // Start TestMcpServer
    let mcp_url = start_test_mcp_server().await?;
    tracing::info!("TestMcpServer started at: {}", mcp_url);

    // Add TestMcpServer to claude config
    let mut config = claude_agent::config::AgentConfig::default();
    config
        .mcp_servers
        .push(claude_agent::config::McpServerConfig::Http(
            claude_agent::config::HttpTransport {
                transport_type: "http".to_string(),
                name: "test-mcp-server".to_string(),
                url: mcp_url,
                headers: vec![],
            },
        ));

    let (agent, receiver) = claude_agent::agent::ClaudeAgent::new(config).await?;

    let test_name = get_test_name_from_thread();
    let path = get_fixture_path_for(agent.agent_type(), &test_name);

    // Wrap with notification capture (pass receiver directly)
    let recording_agent = RecordingAgent::with_notifications(agent, path, receiver);

    Ok(Box::new(recording_agent))
}

/// Create llama-agent for testing
async fn create_llama_agent() -> Result<Box<dyn AgentWithFixture>> {
    use agent_client_protocol_extras::{
        get_fixture_path_for, get_test_name_from_thread, start_test_mcp_server, RecordingAgent,
    };
    use tokio_stream::wrappers::BroadcastStream;

    // Start TestMcpServer
    let mcp_url = start_test_mcp_server().await?;
    tracing::info!("TestMcpServer started at: {}", mcp_url);

    // Use test model config
    use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};
    let mut config = llama_agent::types::AgentConfig::default();
    config.model.source = llama_agent::types::ModelSource::HuggingFace {
        repo: TEST_MODEL_REPO.to_string(),
        filename: Some(TEST_MODEL_FILE.to_string()),
        folder: None,
    };

    // Create ACP config with TestMcpServer as default and permissive policy
    let mut acp_config = llama_agent::acp::AcpConfig::default();
    // Use rule-based policy that allows all MCP tool calls
    acp_config.permission_policy =
        llama_agent::acp::PermissionPolicy::RuleBased(vec![llama_agent::acp::PermissionRule {
            pattern: llama_agent::acp::ToolPattern::All, // Match all tools
            action: llama_agent::acp::PermissionAction::Allow,
        }]);
    acp_config
        .default_mcp_servers
        .push(agent_client_protocol::McpServer::Http(
            agent_client_protocol::McpServerHttp::new("test-mcp-server", &mcp_url),
        ));

    let (agent, notification_rx) =
        llama_agent::acp::test_utils::create_acp_server_with_config(config, acp_config).await?;

    let test_name = get_test_name_from_thread();
    let path = get_fixture_path_for(agent.agent_type(), &test_name);

    // Wrap with notification capture (pass receiver directly)
    let recording_agent = RecordingAgent::with_notifications(agent, path, notification_rx);
    Ok(Box::new(recording_agent))
}

/// Create generic agent (uses llama)
async fn create_agent() -> Result<Box<dyn AgentWithFixture>> {
    create_llama_agent().await
}
