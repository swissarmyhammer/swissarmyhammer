//! Agent fixtures for conformance testing.
//!
//! One factory per agent type. All return `Box<dyn AgentWithFixture>`.
//! Tests obtain a `ConnectionTo<Agent>` from the wrapper and drive ACP
//! requests against it directly.
//!
//! IMPORTANT: If a fixture exists, [`PlaybackAgentWithFixture`] is returned
//! directly without starting the actual LLM agent — this saves significant
//! memory and CPU. When no fixture exists, a real `ClaudeAgent` /
//! `llama_agent::AcpServer` is constructed, wrapped in a
//! [`ConnectTo<Client>`] adapter (mirroring the production wiring in
//! `swissarmyhammer-agent`), and folded into a
//! [`RecordingAgentWithFixture`] that captures the session to disk on drop.
//!
//! ## ACP 0.11 wiring
//!
//! In ACP 0.10 these factories returned `Box<dyn Agent>` and wrapped the
//! backend in `RecordingAgent` directly. ACP 0.11 removed the `Agent`
//! trait — the per-backend wiring now registers typed handlers on
//! `agent_client_protocol::Agent.builder()`, demultiplexes incoming
//! `ClientRequest` / `ClientNotification` enums onto the backend's
//! inherent methods, and forwards the backend's broadcast
//! `SessionNotification` stream onto the connection's typed notification
//! channel via `with_spawned(...)`. The result is a `ConnectTo<Client>`
//! component that can be wrapped in `RecordingAgent` exactly the same way
//! the old `Agent` impl was.

use std::sync::Arc;

use agent_client_protocol::schema::{
    ClientNotification, ClientRequest, McpServer, McpServerHttp, SessionNotification,
};
use agent_client_protocol::{Agent, Client, ConnectTo, Responder};
use agent_client_protocol_extras::{
    get_fixture_path_for, get_test_name_from_thread, AgentWithFixture, PlaybackAgentWithFixture,
};
use tokio::sync::broadcast;

/// Result type for agent creation
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Llama agent factory for rstest.
///
/// Returns a [`PlaybackAgentWithFixture`] if a fixture exists for the
/// current test (fast, no LLM loaded). Otherwise builds a real
/// `llama_agent::AcpServer`, wraps it in [`LlamaAgentAdapter`] for the new
/// `ConnectTo<Client>` shape, and folds it into a recording wrapper for
/// the next run.
pub(crate) fn llama_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_llama_agent()
            .await
            .expect("Failed to create llama agent")
    })
}

/// Claude agent factory for rstest. See [`llama_agent_factory`] for the
/// full wiring story.
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
    Box::new(std::io::Error::other(e.to_string()))
}

/// Agent type identifier for claude
const CLAUDE_AGENT_TYPE: &str = "claude";

/// Agent type identifier for llama
const LLAMA_AGENT_TYPE: &str = "llama";

/// Create the playback wrapper for an existing fixture.
///
/// Returns the boxed wrapper. The caller is responsible for deciding
/// whether a fixture is present; this helper exists so the playback path
/// is consistent across both factories.
async fn open_playback_fixture(
    fixture_path: std::path::PathBuf,
    agent_type: &'static str,
) -> Result<Box<dyn AgentWithFixture>> {
    let wrapper = PlaybackAgentWithFixture::from_fixture(fixture_path, agent_type)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        })?;
    Ok(Box::new(wrapper))
}

/// Create claude-agent for testing.
///
/// Checks if a fixture exists FIRST — if so, returns
/// [`PlaybackAgentWithFixture`] without creating the real `ClaudeAgent`
/// (avoids API initialization overhead).
async fn create_claude_agent() -> Result<Box<dyn AgentWithFixture>> {
    let test_name = get_test_name_from_thread();
    let fixture_path = get_fixture_path_for(CLAUDE_AGENT_TYPE, &test_name);

    if fixture_path.exists() {
        tracing::info!(
            "Fixture exists at {:?}, using PlaybackAgent (skipping real agent creation)",
            fixture_path
        );
        return open_playback_fixture(fixture_path, CLAUDE_AGENT_TYPE).await;
    }

    tracing::info!(
        "No fixture at {:?}, creating real ClaudeAgent for recording",
        fixture_path
    );

    use agent_client_protocol_extras::{start_test_mcp_server_with_capture, RecordingAgent};

    // Start TestMcpServer with proxy for notification capture.
    let mcp_server = start_test_mcp_server_with_capture().await?;
    let mcp_url = mcp_server.url().to_string();
    tracing::info!("TestMcpServer with proxy started at: {}", mcp_url);

    // Add TestMcpServer (via proxy) to claude config.
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
        .map_err(to_send_sync_error)?;

    // Wrap the inherent-method ClaudeAgent in a ConnectTo<Client> adapter,
    // fold into a recording wrapper, and register the MCP proxy as an
    // additional notification source.
    let adapter = ClaudeAgentAdapter::new(Arc::new(agent));
    let recording = RecordingAgent::with_notifications(
        adapter,
        fixture_path,
        CLAUDE_AGENT_TYPE,
        receiver,
    )
    .await
    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::other(e.to_string()))
    })?;
    recording.add_mcp_source(mcp_server.subscribe());

    Ok(Box::new(recording))
}

/// Create llama-agent for testing.
///
/// Checks if a fixture exists FIRST — if so, returns
/// [`PlaybackAgentWithFixture`] without loading the LLM model (avoids
/// massive memory and CPU overhead).
async fn create_llama_agent() -> Result<Box<dyn AgentWithFixture>> {
    let test_name = get_test_name_from_thread();
    let fixture_path = get_fixture_path_for(LLAMA_AGENT_TYPE, &test_name);

    if fixture_path.exists() {
        tracing::info!(
            "Fixture exists at {:?}, using PlaybackAgent (skipping LLM model loading)",
            fixture_path
        );
        return open_playback_fixture(fixture_path, LLAMA_AGENT_TYPE).await;
    }

    tracing::info!(
        "No fixture at {:?}, creating real LlamaAgent for recording (this will load the LLM model)",
        fixture_path
    );

    use agent_client_protocol_extras::{start_test_mcp_server_with_capture, RecordingAgent};

    // Start TestMcpServer with proxy for notification capture.
    let mcp_server = start_test_mcp_server_with_capture().await?;
    let mcp_url = mcp_server.url().to_string();
    tracing::info!("TestMcpServer with proxy started at: {}", mcp_url);

    // Use test model config.
    use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};
    let mut config = llama_agent::types::AgentConfig::default();
    config.model.source = llama_agent::types::ModelSource::HuggingFace {
        repo: TEST_MODEL_REPO.to_string(),
        filename: Some(TEST_MODEL_FILE.to_string()),
        folder: None,
    };

    // Create ACP config with TestMcpServer (via proxy) as default and
    // permissive policy so test prompts can use the proxy's tools.
    let mut acp_config = llama_agent::acp::AcpConfig {
        permission_policy: llama_agent::acp::PermissionPolicy::RuleBased(vec![
            llama_agent::acp::PermissionRule {
                pattern: llama_agent::acp::ToolPattern::All,
                action: llama_agent::acp::PermissionAction::Allow,
            },
        ]),
        ..Default::default()
    };
    acp_config.default_mcp_servers.push(McpServer::Http(
        McpServerHttp::new("test-mcp-server", &mcp_url),
    ));

    let (agent, notification_rx) =
        llama_agent::acp::test_utils::create_acp_server_with_config(config, acp_config)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                Box::new(std::io::Error::other(e.to_string()))
            })?;

    let adapter = LlamaAgentAdapter::new(Arc::new(agent));
    let recording = RecordingAgent::with_notifications(
        adapter,
        fixture_path,
        LLAMA_AGENT_TYPE,
        notification_rx,
    )
    .await
    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::other(e.to_string()))
    })?;
    recording.add_mcp_source(mcp_server.subscribe());

    Ok(Box::new(recording))
}

/// Create generic agent (uses llama)
#[allow(dead_code)]
async fn create_agent() -> Result<Box<dyn AgentWithFixture>> {
    create_llama_agent().await
}

// ---------------------------------------------------------------------------
// Per-backend ConnectTo<Client> adapters
// ---------------------------------------------------------------------------
//
// In ACP 0.11 backends are not implemented as `impl Agent for ...` types —
// callers wire them up by registering typed handlers on
// `Agent.builder()`. The two adapters below mirror the production wiring
// in `swissarmyhammer-agent` (`wrap_claude_into_handle`,
// `wrap_llama_into_handle`) and are kept local to the conformance crate
// so it doesn't gain a hard dependency on `swissarmyhammer-agent` (or its
// LLM stack) just for these tests.

/// `ConnectTo<Client>` adapter that drives `claude_agent::ClaudeAgent`'s
/// inherent methods through an `Agent.builder()` topology.
struct ClaudeAgentAdapter {
    inner: Arc<claude_agent::ClaudeAgent>,
}

impl ClaudeAgentAdapter {
    fn new(inner: Arc<claude_agent::ClaudeAgent>) -> Self {
        Self { inner }
    }
}

impl ConnectTo<Client> for ClaudeAgentAdapter {
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> agent_client_protocol::Result<()> {
        let agent_for_requests = Arc::clone(&self.inner);
        let agent_for_notifications = Arc::clone(&self.inner);
        Agent
            .builder()
            .name("claude-agent-conformance")
            .on_receive_request(
                async move |req: ClientRequest, responder: Responder<serde_json::Value>, _cx| {
                    dispatch_claude_request(&agent_for_requests, req, responder).await
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |notif: ClientNotification, _cx| {
                    dispatch_claude_notification(&agent_for_notifications, notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

/// `ConnectTo<Client>` adapter that drives `llama_agent::AcpServer`'s
/// inherent methods through an `Agent.builder()` topology.
struct LlamaAgentAdapter {
    inner: Arc<llama_agent::AcpServer>,
}

impl LlamaAgentAdapter {
    fn new(inner: Arc<llama_agent::AcpServer>) -> Self {
        Self { inner }
    }
}

impl ConnectTo<Client> for LlamaAgentAdapter {
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> agent_client_protocol::Result<()> {
        let agent_for_requests = Arc::clone(&self.inner);
        let agent_for_notifications = Arc::clone(&self.inner);
        Agent
            .builder()
            .name("llama-agent-conformance")
            .on_receive_request(
                async move |req: ClientRequest, responder: Responder<serde_json::Value>, _cx| {
                    dispatch_llama_request(&agent_for_requests, req, responder).await
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |notif: ClientNotification, _cx| {
                    dispatch_llama_notification(&agent_for_notifications, notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

// ---------------------------------------------------------------------------
// Per-backend dispatch helpers
// ---------------------------------------------------------------------------

/// Demultiplex an incoming `ClientRequest` onto `ClaudeAgent`'s inherent
/// methods. Mirrors `swissarmyhammer-agent::dispatch_claude_request`.
async fn dispatch_claude_request(
    agent: &Arc<claude_agent::ClaudeAgent>,
    request: ClientRequest,
    responder: Responder<serde_json::Value>,
) -> std::result::Result<(), agent_client_protocol::Error> {
    match request {
        ClientRequest::InitializeRequest(req) => responder
            .cast()
            .respond_with_result(agent.initialize(req).await),
        ClientRequest::AuthenticateRequest(req) => responder
            .cast()
            .respond_with_result(agent.authenticate(req).await),
        ClientRequest::NewSessionRequest(req) => responder
            .cast()
            .respond_with_result(agent.new_session(req).await),
        ClientRequest::LoadSessionRequest(req) => responder
            .cast()
            .respond_with_result(agent.load_session(req).await),
        ClientRequest::SetSessionModeRequest(req) => responder
            .cast()
            .respond_with_result(agent.set_session_mode(req).await),
        ClientRequest::PromptRequest(req) => responder
            .cast()
            .respond_with_result(agent.prompt(req).await),
        ClientRequest::ExtMethodRequest(req) => {
            let result = agent.ext_method(req).await.and_then(|ext_response| {
                serde_json::from_str::<serde_json::Value>(ext_response.0.get())
                    .map_err(|_| agent_client_protocol::Error::internal_error())
            });
            responder.respond_with_result(result)
        }
        other => {
            tracing::warn!(
                "Unsupported ClientRequest variant for claude-agent: {}",
                other.method()
            );
            responder
                .cast::<serde_json::Value>()
                .respond_with_error(agent_client_protocol::Error::method_not_found())
        }
    }
}

/// Demultiplex an incoming `ClientNotification` onto `ClaudeAgent`. Errors
/// are logged inside the per-variant handler and never propagated.
async fn dispatch_claude_notification(
    agent: &Arc<claude_agent::ClaudeAgent>,
    notification: ClientNotification,
) {
    match notification {
        ClientNotification::CancelNotification(n) => {
            if let Err(e) = agent.cancel(n).await {
                tracing::error!("cancel notification handler failed: {}", e);
            }
        }
        ClientNotification::ExtNotification(n) => {
            if let Err(e) = agent.ext_notification(n).await {
                tracing::error!("ext notification handler failed: {}", e);
            }
        }
        other => {
            tracing::debug!(
                "Ignoring unsupported ClientNotification variant: {}",
                other.method()
            );
        }
    }
}

/// Demultiplex an incoming `ClientRequest` onto `AcpServer`'s inherent
/// methods. Mirrors `swissarmyhammer-agent::dispatch_llama_request`.
async fn dispatch_llama_request(
    agent: &Arc<llama_agent::AcpServer>,
    request: ClientRequest,
    responder: Responder<serde_json::Value>,
) -> std::result::Result<(), agent_client_protocol::Error> {
    match request {
        ClientRequest::InitializeRequest(req) => responder
            .cast()
            .respond_with_result(agent.initialize(req).await),
        ClientRequest::AuthenticateRequest(req) => responder
            .cast()
            .respond_with_result(agent.authenticate(req).await),
        ClientRequest::NewSessionRequest(req) => responder
            .cast()
            .respond_with_result(agent.new_session(req).await),
        ClientRequest::LoadSessionRequest(req) => responder
            .cast()
            .respond_with_result(agent.load_session(req).await),
        ClientRequest::SetSessionModeRequest(req) => responder
            .cast()
            .respond_with_result(agent.set_session_mode(req).await),
        ClientRequest::PromptRequest(req) => responder
            .cast()
            .respond_with_result(agent.prompt(req).await),
        ClientRequest::ExtMethodRequest(req) => {
            let result = agent.ext_method(req).await.and_then(|ext_response| {
                serde_json::from_str::<serde_json::Value>(ext_response.0.get())
                    .map_err(|_| agent_client_protocol::Error::internal_error())
            });
            responder.respond_with_result(result)
        }
        other => {
            tracing::warn!(
                "Unsupported ClientRequest variant for llama-agent: {}",
                other.method()
            );
            responder
                .cast::<serde_json::Value>()
                .respond_with_error(agent_client_protocol::Error::method_not_found())
        }
    }
}

/// Demultiplex an incoming `ClientNotification` onto `AcpServer`'s inherent
/// methods.
async fn dispatch_llama_notification(
    agent: &Arc<llama_agent::AcpServer>,
    notification: ClientNotification,
) {
    match notification {
        ClientNotification::CancelNotification(n) => {
            if let Err(e) = agent.cancel(n).await {
                tracing::error!("cancel notification handler failed: {}", e);
            }
        }
        ClientNotification::ExtNotification(n) => {
            if let Err(e) = agent.ext_notification(n).await {
                tracing::error!("ext notification handler failed: {}", e);
            }
        }
        other => {
            tracing::debug!(
                "Ignoring unsupported ClientNotification variant: {}",
                other.method()
            );
        }
    }
}

// Avoid unused-import warning when only one factory is exercised in a
// given test build.
#[allow(dead_code)]
fn _ensure_used(_: SessionNotification, _: broadcast::Receiver<SessionNotification>) {}
