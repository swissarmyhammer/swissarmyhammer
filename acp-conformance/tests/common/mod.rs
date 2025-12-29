//! Agent fixtures for conformance testing
//!
//! One factory per agent type. All return Box<dyn AgentWithFixture>.
//! Tests call agent.use_fixture("test_name") to auto-configure record/playback.
//!
//! IMPORTANT: If a fixture exists, PlaybackAgent is returned directly WITHOUT
//! starting the actual LLM agent. This saves significant memory and CPU.

use agent_client_protocol_extras::{
    get_fixture_path_for, get_test_name_from_thread, AgentWithFixture, PlaybackAgent,
};

/// Result type for agent creation
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Llama agent factory for rstest
///
/// Returns PlaybackAgent if fixture exists (fast, no LLM loaded).
/// Returns RecordingAgent wrapping real agent if fixture missing (records for next run).
pub(crate) fn llama_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_llama_agent()
            .await
            .expect("Failed to create llama agent")
    })
}

/// Claude agent factory for rstest
///
/// Returns PlaybackAgent if fixture exists (fast, no API calls).
/// Returns RecordingAgent wrapping real agent if fixture missing (records for next run).
pub(crate) fn claude_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_claude_agent()
            .await
            .expect("Failed to create claude agent")
    })
}

/// Helper to convert errors to Send+Sync
fn to_send_sync_error(
    e: impl std::error::Error + 'static,
) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        e.to_string(),
    ))
}

/// Agent type identifier for claude
const CLAUDE_AGENT_TYPE: &str = "claude";

/// Agent type identifier for llama
const LLAMA_AGENT_TYPE: &str = "llama";

/// Create claude-agent for testing
///
/// Checks if fixture exists FIRST - if so, returns PlaybackAgent without
/// creating the real ClaudeAgent (avoids API initialization overhead).
async fn create_claude_agent() -> Result<Box<dyn AgentWithFixture>> {
    let test_name = get_test_name_from_thread();
    let fixture_path = get_fixture_path_for(CLAUDE_AGENT_TYPE, &test_name);

    // Check if fixture exists - if so, use playback (no real agent needed!)
    if fixture_path.exists() {
        tracing::info!(
            "Fixture exists at {:?}, using PlaybackAgent (skipping real agent creation)",
            fixture_path
        );
        return Ok(Box::new(PlaybackAgent::new(
            fixture_path,
            CLAUDE_AGENT_TYPE,
        )));
    }

    // No fixture - need to create real agent and record
    tracing::info!(
        "No fixture at {:?}, creating real ClaudeAgent for recording",
        fixture_path
    );

    use agent_client_protocol_extras::{
        start_test_mcp_server_with_capture, McpNotificationSource, RecordingAgent,
    };

    // Start TestMcpServer with proxy for notification capture
    let mcp_server = start_test_mcp_server_with_capture().await?;
    let mcp_url = mcp_server.url().to_string();
    tracing::info!("TestMcpServer with proxy started at: {}", mcp_url);

    // Add TestMcpServer (via proxy) to claude config
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

    let (agent, receiver) = claude_agent::agent::ClaudeAgent::new(config)
        .await
        .map_err(|e| to_send_sync_error(e))?;

    // Wrap with notification capture
    let recording_agent = RecordingAgent::with_notifications(agent, fixture_path, receiver);

    // Add MCP notification source from the proxy
    recording_agent.add_mcp_source(mcp_server.subscribe());

    Ok(Box::new(recording_agent))
}

/// Create llama-agent for testing
///
/// Checks if fixture exists FIRST - if so, returns PlaybackAgent without
/// loading the LLM model (avoids massive memory and CPU overhead).
async fn create_llama_agent() -> Result<Box<dyn AgentWithFixture>> {
    let test_name = get_test_name_from_thread();
    let fixture_path = get_fixture_path_for(LLAMA_AGENT_TYPE, &test_name);

    // Check if fixture exists - if so, use playback (no LLM loading needed!)
    if fixture_path.exists() {
        tracing::info!(
            "Fixture exists at {:?}, using PlaybackAgent (skipping LLM model loading)",
            fixture_path
        );
        return Ok(Box::new(PlaybackAgent::new(fixture_path, LLAMA_AGENT_TYPE)));
    }

    // No fixture - need to create real agent and record
    tracing::info!(
        "No fixture at {:?}, creating real LlamaAgent for recording (this will load the LLM model)",
        fixture_path
    );

    use agent_client_protocol_extras::{
        start_test_mcp_server_with_capture, McpNotificationSource, RecordingAgent,
    };

    // Start TestMcpServer with proxy for notification capture
    let mcp_server = start_test_mcp_server_with_capture().await?;
    let mcp_url = mcp_server.url().to_string();
    tracing::info!("TestMcpServer with proxy started at: {}", mcp_url);

    // Use test model config
    use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};
    let mut config = llama_agent::types::AgentConfig::default();
    config.model.source = llama_agent::types::ModelSource::HuggingFace {
        repo: TEST_MODEL_REPO.to_string(),
        filename: Some(TEST_MODEL_FILE.to_string()),
        folder: None,
    };

    // Create ACP config with TestMcpServer (via proxy) as default and permissive policy
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
        llama_agent::acp::test_utils::create_acp_server_with_config(config, acp_config)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

    // Wrap with notification capture
    let recording_agent = RecordingAgent::with_notifications(agent, fixture_path, notification_rx);

    // Add MCP notification source from the proxy
    recording_agent.add_mcp_source(mcp_server.subscribe());

    Ok(Box::new(recording_agent))
}

/// Create generic agent (uses llama)
#[allow(dead_code)]
async fn create_agent() -> Result<Box<dyn AgentWithFixture>> {
    create_llama_agent().await
}
