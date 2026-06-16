//! Test utilities for ACP server creation
//!
//! Helper functions for creating ACP servers in tests with minimal
//! configuration, plus shared test fixtures (e.g. [`StateDirGuard`]). This
//! module is compiled in both test and non-test builds so integration tests
//! can import it.

use crate::acp::{AcpConfig, AcpServer};
use crate::types::{AgentConfig, ModelConfig, ModelSource, SessionConfig};
use std::path::PathBuf;
use std::sync::Arc;

/// The cwd test sessions are created with — named once so a future change
/// (e.g. a per-test `TempDir`) edits a single place.
pub fn test_cwd() -> PathBuf {
    PathBuf::from("/tmp")
}

/// Model-free test `ModelConfig`: a `Local` source pointing at `folder` (no
/// `.gguf` need exist — the model is never loaded), single-sequence, with
/// every other knob (batch size, threads, retry policy) at the production
/// default via `..ModelConfig::default()` so test configs cannot drift from
/// it.
pub fn test_model_config(folder: &std::path::Path) -> ModelConfig {
    ModelConfig {
        source: ModelSource::Local {
            folder: folder.to_path_buf(),
            filename: Some("test.gguf".to_string()),
        },
        n_seq_max: 1,
        use_hf_params: false,
        ..ModelConfig::default()
    }
}

/// Minimal model-free `AgentConfig` around [`test_model_config`]; the
/// caller-supplied `session_config` lets a test enable session persistence
/// into its own temp dir. The model folder is a throwaway temp directory —
/// it is deleted on return, which is fine because the model is never loaded.
pub fn test_agent_config(session_config: SessionConfig) -> AgentConfig {
    let temp_dir = tempfile::TempDir::new().expect("temp dir for model folder");
    AgentConfig {
        model: test_model_config(temp_dir.path()),
        queue_config: crate::types::QueueConfig::default(),
        mcp_servers: Vec::new(),
        session_config,
        parallel_execution_config: crate::types::ParallelConfig::default(),
        tool_execution_config: Default::default(),
    }
}

/// The `AcpServer` plus the broadcast receiver for its session notifications.
pub type AcpServerWithNotifications = (
    AcpServer,
    tokio::sync::broadcast::Receiver<agent_client_protocol::schema::SessionNotification>,
);

/// Wire every `AcpServer` component the way the production bootstrap wires
/// them — ModelManager, RequestQueue, SessionManager, no-op MCP client, chat
/// template engine, dependency analyzer, and the in-process echo tools mount.
///
/// This is the single wiring path behind every test-server constructor in
/// this module; `load_model` decides whether the model is actually loaded
/// (session lifecycle and the fork/status/pin extension surface work without
/// one).
async fn build_acp_server(
    agent_config: AgentConfig,
    acp_config: AcpConfig,
    load_model: bool,
) -> Result<AcpServerWithNotifications, Box<dyn std::error::Error>> {
    let model_manager = Arc::new(crate::model::ModelManager::new(agent_config.model.clone())?);

    if load_model {
        model_manager.load_model().await?;
    }

    let request_queue = Arc::new(crate::queue::RequestQueue::new(
        model_manager.clone(),
        agent_config.queue_config.clone(),
        agent_config.session_config.clone(),
    ));
    let session_manager = Arc::new(crate::session::SessionManager::new(
        agent_config.session_config.clone(),
    ));
    let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
    let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
    let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
        agent_config.parallel_execution_config.clone(),
    ));

    let agent_server = Arc::new(crate::AgentServer::new(
        model_manager,
        request_queue,
        session_manager,
        mcp_client,
        chat_template,
        dependency_analyzer,
        agent_config,
    ));

    let mount = Arc::new(crate::mcp::InProcessMount::new(
        crate::echo::EchoService::new(),
    ));
    Ok(AcpServer::new(agent_server, acp_config, mount))
}

/// Create ACP server for testing with custom config
pub async fn create_acp_server(
    config: AgentConfig,
) -> Result<AcpServerWithNotifications, Box<dyn std::error::Error>> {
    build_acp_server(config, AcpConfig::default(), true).await
}

/// Create ACP server for testing with custom AgentConfig and AcpConfig
pub async fn create_acp_server_with_config(
    agent_config: AgentConfig,
    acp_config: AcpConfig,
) -> Result<AcpServerWithNotifications, Box<dyn std::error::Error>> {
    build_acp_server(agent_config, acp_config, true).await
}

/// Create a model-free ACP server: every component is wired the way the
/// production bootstrap wires them, but no model is ever loaded.
///
/// Session creation and the session-extension surface (`session/fork`,
/// `session/state_status`, `session/pin`, resume/load) all work without a
/// model, so tests of those paths stay milliseconds.
pub async fn create_acp_server_without_model(
    agent_config: AgentConfig,
) -> Result<AcpServerWithNotifications, Box<dyn std::error::Error>> {
    build_acp_server(agent_config, AcpConfig::default(), false).await
}

/// Build a `PromptRequest` carrying a single user text block — the minimal
/// prompt every prompt-path test sends. Shared by the in-crate `server.rs`
/// tests and the real-model integration tests so the two cannot drift.
pub fn text_prompt(
    session_id: agent_client_protocol::schema::SessionId,
    text: &str,
) -> agent_client_protocol::schema::PromptRequest {
    agent_client_protocol::schema::PromptRequest::new(
        session_id,
        vec![agent_client_protocol::schema::ContentBlock::from(
            text.to_string(),
        )],
    )
}

// The canonical XDG_STATE_HOME isolation guard lives in
// `agent-client-protocol-extras` next to the `SessionStore` it isolates
// (enabled via that dependency's `test-support` feature); re-export it rather
// than carrying a per-crate copy. `AcpServer::new_session` and
// `AcpServer::prompt` persist a `SessionRecord` to that store, so tests that
// exercise those paths must hold the guard for the whole test body and be
// `#[serial]` — see its docs.
pub use agent_client_protocol_extras::test_support::StateDirGuard;
