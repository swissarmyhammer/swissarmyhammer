//! Unified ACP agent creation and execution
//!
//! This crate provides a single entry point for creating and using ACP agents
//! (claude-agent and llama-agent) based on model configuration.
//!
//! # Architecture
//!
//! ```text
//! swissarmyhammer-agent
//!        │
//!        ├── create_agent(ModelConfig) -> AcpAgentHandle
//!        ├── execute_prompt(handle, prompt) -> AgentResponse
//!        └── Types: AcpAgentHandle, AcpError, AgentResponse, etc.
//!
//! Consumers:
//!   - swissarmyhammer-rules (for rule checking)
//!   - swissarmyhammer-cli (for agent commands)
//! ```
//!
//! # ACP Streaming Architecture
//!
//! ACP agents stream content via SessionNotifications during prompt execution.
//! The PromptResponse only indicates why the agent stopped, not the actual content.
//! This module handles subscribing to notifications, collecting streamed text,
//! and returning it as a simple response.
//!
//! # Session Loading
//!
//! This crate provides a simplified interface for one-shot prompt execution and does
//! not implement session loading. Each call to `execute_prompt` creates a new session.
//! Session loading (`load_session`) is a capability of the underlying ACP agents
//! (claude-agent, llama-agent), not this utility wrapper.
//!
//! For applications that need session persistence and history replay, use the
//! underlying agent implementations directly via the typed
//! `ConnectionTo<agent_client_protocol::Agent>` handle obtained by running
//! [`AcpAgentHandle::agent`] through `agent_client_protocol::Client::builder().connect_with(...)`.
//!
//! # ACP 0.11 redesign
//!
//! In ACP 0.10 the inner backend was an `Arc<dyn Agent + Send + Sync>` —
//! `Agent` was a trait the SDK invoked via dynamic dispatch. ACP 0.11
//! replaces that with a unit Role marker (`agent_client_protocol::Agent`)
//! and a typed builder/handler runtime. Backends are constructed by
//! registering handlers on `Agent.builder()` and yielding a
//! [`DynConnectTo<Client>`] component that callers compose with their own
//! middleware before running `Client::builder().connect_with(...)`.
//!
//! [`AcpAgentHandle::agent`] therefore stores a [`DynConnectTo<Client>`]
//! (the inner agent component, pre-wrapped with [`TracingAgent`] for
//! unified logging). [`execute_prompt`] consumes that component to spin up
//! its own `Client::builder().connect_with(...)` task, then issues
//! `initialize → new_session → set_session_mode → prompt` against the
//! resulting [`ConnectionTo<Agent>`] handle and collects streamed content
//! from [`AcpAgentHandle::notification_rx`].
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_agent::{create_agent, execute_prompt, McpServerConfig};
//! use swissarmyhammer_config::model::ModelConfig;
//!
//! let config = ModelConfig::load("model.yaml")?;
//! let mcp = McpServerConfig::from_port(8080);
//!
//! let mut handle = create_agent(&config, Some(mcp)).await?;
//! let response = execute_prompt(&mut handle, None, None, "Hello!".to_string()).await?;
//! println!("{}", response.content);
//! ```

use agent_client_protocol::schema::{
    self, ClientCapabilities, ClientNotification, ClientRequest, ContentBlock,
    FileSystemCapabilities, InitializeRequest, NewSessionRequest, PromptRequest, PromptResponse,
    SessionId, SessionModeId, SessionNotification, SessionUpdate, SetSessionModeRequest,
    StopReason, TextContent,
};
use agent_client_protocol::{Agent, Client, ConnectionTo, DynConnectTo, Responder};
use agent_client_protocol_extras::{trace_notifications, TracingAgent};
use llama_agent::types::AgentAPI;
use std::sync::Arc;
use std::time::Duration;
use swissarmyhammer_common::{ErrorSeverity, Pretty, Severity};
use swissarmyhammer_config::model::{ModelConfig, ModelExecutorConfig, ModelExecutorType};
use thiserror::Error;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

// ============================================================================
// Configuration Constants
// ============================================================================

/// Maximum prompt length in bytes (5MB for very large source files)
const MAX_PROMPT_LENGTH_BYTES: usize = 5_000_000;

/// Default maximum retry attempts for model operations
const DEFAULT_MAX_RETRIES: u32 = 2;

/// Initial delay in milliseconds before first retry
const DEFAULT_INITIAL_RETRY_DELAY_MS: u64 = 100;

/// Multiplier applied to delay between successive retries
const DEFAULT_BACKOFF_MULTIPLIER: f64 = 1.5;

/// Maximum delay in milliseconds between retries
const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 1000;

/// Default number of threads for model inference
const DEFAULT_NUM_THREADS: i32 = 4;

/// Default number of threads for batch processing
const DEFAULT_BATCH_THREADS: i32 = 4;

/// Keep-alive interval in seconds for SSE connections
const SSE_KEEP_ALIVE_SECONDS: u64 = 30;

/// Maximum size of the request queue
const DEFAULT_MAX_QUEUE_SIZE: usize = 100;

/// Delay in milliseconds to allow notification collector to finish processing
const NOTIFICATION_COLLECTION_DELAY_MS: u64 = 100;

/// Errors that can occur during ACP agent execution
#[derive(Debug, Error)]
pub enum AcpError {
    /// Agent initialization failed
    #[error("Agent initialization failed: {0}")]
    InitializationError(String),
    /// Session creation failed
    #[error("Session creation failed: {0}")]
    SessionError(String),
    /// Prompt execution failed
    #[error("Prompt execution failed: {0}")]
    PromptError(String),
    /// Agent not available (Claude CLI not found, model not loaded, etc.)
    #[error("Agent not available: {0}")]
    AgentNotAvailable(String),
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    /// Rate limit error with retry time
    #[error("Rate limit reached. Please wait {wait_time:?} and try again. Details: {message}")]
    RateLimit {
        /// The error message
        message: String,
        /// How long to wait before retrying
        wait_time: Duration,
    },
}

/// Convenience type alias for Results in ACP operations.
///
/// All functions in this crate that can fail return this Result type,
/// with errors represented by [`AcpError`].
pub type AcpResult<T> = std::result::Result<T, AcpError>;

impl Severity for AcpError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            AcpError::InitializationError(_) => ErrorSeverity::Critical,
            AcpError::SessionError(_) => ErrorSeverity::Error,
            AcpError::PromptError(_) => ErrorSeverity::Error,
            AcpError::AgentNotAvailable(_) => ErrorSeverity::Critical,
            AcpError::ConfigurationError(_) => ErrorSeverity::Error,
            AcpError::RateLimit { .. } => ErrorSeverity::Warning,
        }
    }
}

/// Response from ACP agent execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    /// The primary response content from the agent
    pub content: String,
    /// Optional metadata about the response
    pub metadata: Option<serde_json::Value>,
    /// Response status/type
    pub response_type: AgentResponseType,
}

/// Type of agent response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentResponseType {
    /// Standard successful text response
    Success,
    /// Partial response (streaming, timeout, etc.)
    Partial,
    /// Error response with error details
    Error,
}

impl AgentResponse {
    /// Internal constructor with all parameters
    fn new(
        content: String,
        response_type: AgentResponseType,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            content,
            metadata,
            response_type,
        }
    }

    /// Create a successful response
    pub fn success(content: String) -> Self {
        Self::new(content, AgentResponseType::Success, None)
    }

    /// Create a successful response with metadata
    pub fn success_with_metadata(content: String, metadata: serde_json::Value) -> Self {
        Self::new(content, AgentResponseType::Success, Some(metadata))
    }

    /// Create an error response
    pub fn error(content: String) -> Self {
        Self::new(content, AgentResponseType::Error, None)
    }

    /// Create a partial response
    pub fn partial(content: String) -> Self {
        Self::new(content, AgentResponseType::Partial, None)
    }

    /// Check if this is a successful response
    pub fn is_success(&self) -> bool {
        matches!(self.response_type, AgentResponseType::Success)
    }

    /// Check if this is an error response
    pub fn is_error(&self) -> bool {
        matches!(self.response_type, AgentResponseType::Error)
    }
}

/// MCP server configuration for ACP agents
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// URL of the MCP server (e.g., "http://localhost:8080/mcp")
    pub url: String,
}

impl McpServerConfig {
    /// Create a new MCP server config with the given URL
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// Create MCP server config from port number (assumes localhost HTTP)
    pub fn from_port(port: u16) -> Self {
        Self {
            url: format!("http://localhost:{}/mcp", port),
        }
    }
}

/// Wrapper around an ACP agent component with its notification receiver.
///
/// In ACP 0.11 the agent is no longer accessed via a `dyn Agent` trait
/// object. Instead, [`agent`](Self::agent) is a [`DynConnectTo<Client>`] —
/// the inner agent component (an `Agent.builder()` registration wrapped
/// with [`TracingAgent`]) that callers consume via
/// `agent_client_protocol::Client::builder().connect_with(...)` to obtain
/// a typed [`ConnectionTo<Agent>`] handle for issuing requests. Consumers
/// such as `avp-common` may compose additional `ConnectTo<Client>`
/// middleware (e.g. `RecordingAgent`) on top before connecting.
///
/// [`notification_rx`](Self::notification_rx) carries the broadcast
/// channel fed by the underlying backend (`claude_agent::ClaudeAgent` or
/// `llama_agent::AcpServer`) when its inherent `prompt`/`new_session`
/// methods stream `SessionNotification`s. The `Agent.builder()` inside
/// this crate also bridges that broadcast onto the JSON-RPC
/// `cx.send_notification` channel so consumers that prefer to capture
/// notifications via `Client.builder().on_receive_notification(...)` can
/// do so without needing access to this receiver.
pub struct AcpAgentHandle {
    /// The inner agent component as a [`DynConnectTo<Client>`].
    ///
    /// Callers that just want to run a prompt should pass the whole handle
    /// to [`execute_prompt`]. Callers that need to compose their own
    /// middleware can move `agent` out of the handle and feed it into
    /// `Client::builder().connect_with(agent, ...)` after wrapping with
    /// whatever `ConnectTo<Client>` middleware they want.
    pub agent: DynConnectTo<Client>,
    /// Notification receiver for streaming content from the inner backend.
    pub notification_rx: broadcast::Receiver<SessionNotification>,
}

/// Options for agent creation
#[derive(Debug, Clone, Default)]
pub struct CreateAgentOptions {
    /// Use ephemeral mode (haiku model, no session persistence).
    /// Ideal for quick, stateless operations like scaffold generation.
    pub ephemeral: bool,
    /// Override for Claude's built-in tools. When set to Some(""), disables all built-in tools.
    /// This is used for validator agents that should only have MCP-provided tools.
    pub tools_override: Option<String>,
}

/// Create an ACP agent based on model configuration
///
/// Returns an AcpAgentHandle containing the inner agent component (as a
/// `DynConnectTo<Client>`, pre-wrapped with `TracingAgent`) and a
/// broadcast notification receiver.
///
/// # Arguments
/// * `config` - Model configuration specifying which agent type to create
/// * `mcp_config` - Optional MCP server configuration for tool access
///
/// # Example
/// ```ignore
/// let config = ModelConfig::load("model.yaml")?;
/// let handle = create_agent(&config, None).await?;
/// ```
pub async fn create_agent(
    config: &ModelConfig,
    mcp_config: Option<McpServerConfig>,
) -> AcpResult<AcpAgentHandle> {
    create_agent_with_options(config, mcp_config, CreateAgentOptions::default()).await
}

/// Create an ACP agent with additional options
///
/// Like `create_agent` but accepts options for ephemeral mode, etc.
pub async fn create_agent_with_options(
    config: &ModelConfig,
    mcp_config: Option<McpServerConfig>,
    options: CreateAgentOptions,
) -> AcpResult<AcpAgentHandle> {
    match config.executor_type() {
        ModelExecutorType::ClaudeCode => {
            create_claude_agent(
                mcp_config,
                options.ephemeral,
                options.tools_override.clone(),
            )
            .await
        }
        ModelExecutorType::LlamaAgent => {
            let llama_config = match config.executor() {
                ModelExecutorConfig::LlamaAgent(cfg) => cfg.clone(),
                _ => {
                    return Err(AcpError::ConfigurationError(
                        "Expected LlamaAgent configuration".to_string(),
                    ))
                }
            };
            create_llama_agent(llama_config, mcp_config).await
        }
        ModelExecutorType::LlamaEmbedding | ModelExecutorType::AneEmbedding => Err(
            AcpError::ConfigurationError("Embedding models cannot be used as agents".to_string()),
        ),
    }
}

/// Wrap a `claude_agent::ClaudeAgent` into a `DynConnectTo<Client>`
/// component for ACP 0.11.
///
/// In 0.11 backends are not implemented as `impl Agent for ...` types.
/// Instead, you build a `ConnectTo<Client>` value by registering typed
/// handlers on `agent_client_protocol::Agent.builder()`. This helper
/// performs that wiring uniformly:
///
/// - The `on_receive_request` handler demultiplexes every incoming
///   `ClientRequest` enum variant onto the matching inherent method on
///   the inner backend.
/// - The `on_receive_notification` handler demultiplexes incoming
///   `ClientNotification` variants onto the matching inherent method.
/// - A `with_spawned` task forwards `SessionNotification`s emitted via
///   the backend's internal broadcast sender onto the connection's
///   typed `cx.send_notification` channel, so consumers that subscribe
///   via `Client.builder().on_receive_notification(...)` see them.
///
/// The result is wrapped with [`TracingAgent`] for unified per-backend
/// logging, type-erased into [`DynConnectTo<Client>`], and returned
/// alongside the broadcast receiver fed into `notification_rx` on the
/// returned [`AcpAgentHandle`].
fn wrap_claude_into_handle(
    agent: claude_agent::ClaudeAgent,
    notification_rx: broadcast::Receiver<SessionNotification>,
) -> AcpAgentHandle {
    let agent = Arc::new(agent);
    let bridge_rx = notification_rx.resubscribe();

    let builder = Agent
        .builder()
        .name("claude-agent")
        .on_receive_request(
            {
                let agent = Arc::clone(&agent);
                async move |req: ClientRequest, responder: Responder<serde_json::Value>, _cx| {
                    dispatch_claude_request(&agent, req, responder).await
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            {
                let agent = Arc::clone(&agent);
                async move |notif: ClientNotification, _cx| {
                    dispatch_claude_notification(&agent, notif).await;
                    Ok(())
                }
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .with_spawned(move |cx: ConnectionTo<Client>| async move {
            forward_session_notifications(bridge_rx, cx).await
        });

    let traced = TracingAgent::new(builder, "Claude");
    let traced_rx = trace_notifications("Claude".to_string(), notification_rx);

    AcpAgentHandle {
        agent: DynConnectTo::new(traced),
        notification_rx: traced_rx,
    }
}

/// Mirror of [`wrap_claude_into_handle`] for `llama_agent::AcpServer`.
fn wrap_llama_into_handle(
    agent: llama_agent::AcpServer,
    notification_rx: broadcast::Receiver<SessionNotification>,
) -> AcpAgentHandle {
    let agent = Arc::new(agent);
    let bridge_rx = notification_rx.resubscribe();

    let builder = Agent
        .builder()
        .name("llama-agent")
        .on_receive_request(
            {
                let agent = Arc::clone(&agent);
                async move |req: ClientRequest, responder: Responder<serde_json::Value>, _cx| {
                    dispatch_llama_request(&agent, req, responder).await
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            {
                let agent = Arc::clone(&agent);
                async move |notif: ClientNotification, _cx| {
                    dispatch_llama_notification(&agent, notif).await;
                    Ok(())
                }
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .with_spawned(move |cx: ConnectionTo<Client>| async move {
            forward_session_notifications(bridge_rx, cx).await
        });

    let traced = TracingAgent::new(builder, "Llama");
    let traced_rx = trace_notifications("Llama".to_string(), notification_rx);

    AcpAgentHandle {
        agent: DynConnectTo::new(traced),
        notification_rx: traced_rx,
    }
}

/// Forward `SessionNotification`s from the backend's broadcast channel onto
/// the connection's typed notification channel so JSON-RPC clients see them.
///
/// Exits cleanly when the broadcast channel closes (all senders dropped) or
/// when `cx.send_notification` errors (write side of transport torn down).
async fn forward_session_notifications(
    mut rx: broadcast::Receiver<SessionNotification>,
    cx: ConnectionTo<Client>,
) -> Result<(), agent_client_protocol::Error> {
    loop {
        match rx.recv().await {
            Ok(notification) => {
                if let Err(e) = cx.send_notification(notification) {
                    tracing::debug!(error = %e, "Failed to forward session/update; bridge stopping");
                    return Ok(());
                }
            }
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "Notification bridge lagged");
            }
        }
    }
}

/// Demultiplex an incoming `ClientRequest` onto `ClaudeAgent`'s inherent
/// methods. Mirrors the per-method handler registration that
/// `start_with_streams` would otherwise wire up.
async fn dispatch_claude_request(
    agent: &Arc<claude_agent::ClaudeAgent>,
    request: ClientRequest,
    responder: Responder<serde_json::Value>,
) -> Result<(), agent_client_protocol::Error> {
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

/// Demultiplex an incoming `ClientNotification` onto `ClaudeAgent`'s
/// inherent methods. Notifications are fire-and-forget; per-variant
/// errors are logged but not surfaced (returning Err here would tear the
/// connection down).
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
/// methods.
async fn dispatch_llama_request(
    agent: &Arc<llama_agent::AcpServer>,
    request: ClientRequest,
    responder: Responder<serde_json::Value>,
) -> Result<(), agent_client_protocol::Error> {
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

/// Demultiplex an incoming `ClientNotification` onto `AcpServer`'s
/// inherent methods.
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

/// Create a Claude ACP agent
async fn create_claude_agent(
    mcp_config: Option<McpServerConfig>,
    ephemeral: bool,
    tools_override: Option<String>,
) -> AcpResult<AcpAgentHandle> {
    // Check if Claude CLI is available (claude-agent requires this)
    if which::which("claude").is_err() {
        return Err(AcpError::AgentNotAvailable(
            "Claude CLI not found in PATH. Install with: npm install -g @anthropic-ai/claude-code"
                .to_string(),
        ));
    }

    // Create Claude agent configuration with MCP servers
    // Increase max prompt length for rule checking which may include very large files
    let mut agent_config = if let Some(mcp) = mcp_config {
        claude_agent::AgentConfig {
            max_prompt_length: MAX_PROMPT_LENGTH_BYTES,
            mcp_servers: vec![claude_agent::config::McpServerConfig::Http(
                claude_agent::config::HttpTransport {
                    transport_type: "http".to_string(),
                    name: "swissarmyhammer".to_string(),
                    url: mcp.url,
                    headers: vec![],
                },
            )],
            ..Default::default()
        }
    } else {
        claude_agent::AgentConfig {
            max_prompt_length: MAX_PROMPT_LENGTH_BYTES,
            ..Default::default()
        }
    };

    agent_config.claude.ephemeral = ephemeral;
    agent_config.claude.tools_override = tools_override;

    // Create the Claude agent
    let (agent, notification_rx) =
        claude_agent::ClaudeAgent::new(agent_config)
            .await
            .map_err(|e| {
                AcpError::InitializationError(format!("Failed to create Claude agent: {}", e))
            })?;

    Ok(wrap_claude_into_handle(agent, notification_rx))
}

/// Convert swissarmyhammer model source to llama-agent model source
fn convert_model_source(
    source: &swissarmyhammer_config::model::ModelSource,
) -> llama_agent::types::ModelSource {
    match source {
        swissarmyhammer_config::model::ModelSource::Local { filename, folder } => {
            llama_agent::types::ModelSource::Local {
                folder: folder.clone().unwrap_or_else(|| {
                    filename
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf()
                }),
                filename: filename
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string()),
            }
        }
        swissarmyhammer_config::model::ModelSource::HuggingFace {
            repo,
            filename,
            folder,
        } => llama_agent::types::ModelSource::HuggingFace {
            repo: repo.clone(),
            filename: if folder.is_some() {
                None
            } else {
                filename.clone()
            },
            folder: folder.clone(),
        },
    }
}

/// Build llama-agent ModelConfig from swissarmyhammer config
fn build_llama_model_config(
    llama_config: &swissarmyhammer_config::model::LlamaAgentConfig,
) -> llama_agent::types::ModelConfig {
    let model_source = convert_model_source(&llama_config.model.source);
    llama_agent::types::ModelConfig {
        source: model_source,
        batch_size: llama_config.model.batch_size,
        use_hf_params: llama_config.model.use_hf_params,
        retry_config: llama_agent::types::RetryConfig {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_delay_ms: DEFAULT_INITIAL_RETRY_DELAY_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            max_delay_ms: DEFAULT_MAX_RETRY_DELAY_MS,
        },
        debug: false,
        n_seq_max: 1,
        n_threads: DEFAULT_NUM_THREADS,
        n_threads_batch: DEFAULT_BATCH_THREADS,
    }
}

/// Build MCP server configuration for llama-agent
fn build_llama_mcp_servers(
    mcp_config: Option<&McpServerConfig>,
    timeout_seconds: u64,
) -> Vec<llama_agent::types::MCPServerConfig> {
    match mcp_config {
        Some(mcp) => vec![llama_agent::types::MCPServerConfig::Http(
            llama_agent::types::HttpServerConfig {
                name: "swissarmyhammer".to_string(),
                url: mcp.url.clone(),
                timeout_secs: Some(timeout_seconds),
                sse_keep_alive_secs: Some(SSE_KEEP_ALIVE_SECONDS),
                stateful_mode: false,
            },
        )],
        None => vec![],
    }
}

/// Convert llama-agent MCP servers to ACP format
fn convert_mcp_servers_to_acp(
    mcp_servers: &[llama_agent::types::MCPServerConfig],
) -> Vec<schema::McpServer> {
    mcp_servers
        .iter()
        .map(|server| match server {
            llama_agent::types::MCPServerConfig::Http(http_config) => schema::McpServer::Http(
                schema::McpServerHttp::new(http_config.name.clone(), http_config.url.clone()),
            ),
            llama_agent::types::MCPServerConfig::InProcess(process_config) => {
                let mut stdio_server = schema::McpServerStdio::new(
                    process_config.name.clone(),
                    &process_config.command,
                );
                stdio_server.args = process_config.args.clone();
                schema::McpServer::Stdio(stdio_server)
            }
        })
        .collect()
}

/// Create a Llama ACP agent
async fn create_llama_agent(
    llama_config: swissarmyhammer_config::model::LlamaAgentConfig,
    mcp_config: Option<McpServerConfig>,
) -> AcpResult<AcpAgentHandle> {
    let model_config = build_llama_model_config(&llama_config);
    let mcp_servers =
        build_llama_mcp_servers(mcp_config.as_ref(), llama_config.mcp_server.timeout_seconds);
    let acp_mcp_servers = convert_mcp_servers_to_acp(&mcp_servers);

    let agent_config = llama_agent::types::AgentConfig {
        model: model_config,
        queue_config: llama_agent::types::QueueConfig {
            max_queue_size: DEFAULT_MAX_QUEUE_SIZE,
            worker_threads: 1,
        },
        session_config: llama_agent::types::SessionConfig::default(),
        mcp_servers,
        parallel_execution_config: llama_agent::types::ParallelConfig::default(),
    };

    // Initialize the AgentServer using the AgentAPI trait
    let agent_server = llama_agent::AgentServer::initialize(agent_config)
        .await
        .map_err(|e| {
            AcpError::InitializationError(format!("Failed to initialize Llama agent server: {}", e))
        })?;

    // Create ACP server configuration with MCP servers
    let acp_config = llama_agent::AcpConfig {
        default_mcp_servers: acp_mcp_servers,
        ..Default::default()
    };

    // Create the ACP server (its inherent methods serve as the per-method
    // handlers wired into `Agent.builder()` by `wrap_llama_into_handle`).
    let (acp_server, notification_rx) =
        llama_agent::AcpServer::new(Arc::new(agent_server), acp_config);

    Ok(wrap_llama_into_handle(acp_server, notification_rx))
}

/// Execute a prompt using an ACP agent
///
/// Consumes the agent component on `handle` to spin up its own
/// `Client::builder().connect_with(...)` task, then drives the request
/// sequence `initialize → new_session → set_session_mode → prompt`
/// against the resulting [`ConnectionTo<Agent>`]. Streamed content is
/// captured concurrently via `handle.notification_rx`.
///
/// After this call returns, `handle.agent` has been replaced with a
/// fresh-but-empty placeholder; the handle is single-shot. The caller
/// should drop the handle (or call `create_agent` again) to issue
/// another prompt.
///
/// # Arguments
/// * `handle` - The agent handle from `create_agent`
/// * `system_prompt` - Optional system prompt (passed as session metadata)
/// * `mode` - Optional mode ID to set on the session (e.g., "planner", "implementer")
/// * `user_prompt` - The user's prompt text
///
/// # Example
/// ```ignore
/// let response = execute_prompt(&mut handle, None, Some("planner"), "Design a new feature".to_string()).await?;
/// println!("{}", response.content);
/// ```
pub async fn execute_prompt(
    handle: &mut AcpAgentHandle,
    system_prompt: Option<String>,
    mode: Option<String>,
    user_prompt: String,
) -> AcpResult<AgentResponse> {
    // Take the agent component out of the handle. `connect_with` consumes
    // its `ConnectTo<Client>` argument, so we have to move it. Replace it
    // with a placeholder so the handle remains valid (its other field —
    // `notification_rx` — is still usable for diagnostics).
    let agent = std::mem::replace(&mut handle.agent, DynConnectTo::new(NoopAgent));
    let notification_rx = handle.notification_rx.resubscribe();

    execute_prompt_with_agent(agent, notification_rx, system_prompt, mode, user_prompt).await
}

/// Drive a single-shot ACP turn against the supplied agent component.
///
/// Spins up `agent_client_protocol::Client::builder().connect_with(agent, main_fn)`
/// where `main_fn` issues the standard request sequence and returns the
/// assembled [`AgentResponse`]. Notifications flowing through the
/// connection are forwarded to handlers by the SDK, while the broadcast
/// receiver supplied here continues to receive copies for streaming
/// content collection.
async fn execute_prompt_with_agent(
    agent: DynConnectTo<Client>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    system_prompt: Option<String>,
    mode: Option<String>,
    user_prompt: String,
) -> AcpResult<AgentResponse> {
    // Run the connection in a current-thread tokio runtime on a blocking
    // thread. The SDK's `connect_with` future drives a `LocalSet`-style
    // dispatch loop that may produce `!Send` futures from per-handler
    // closures; staying on a single-thread runtime keeps Send/Sync
    // requirements relaxed and matches the pattern the legacy ACP 0.10
    // implementation used.
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                AcpError::InitializationError(format!("Failed to create runtime: {}", e))
            })?;

        rt.block_on(async move {
            run_prompt_connection(agent, notification_rx, system_prompt, mode, user_prompt).await
        })
    })
    .await
    .map_err(|e| AcpError::PromptError(format!("Task join error: {:?}", e)))??;

    Ok(result)
}

/// Run the `Client.builder().connect_with(...)` connection and drive the
/// full prompt turn inside its `main_fn` closure. Returns the assembled
/// [`AgentResponse`].
async fn run_prompt_connection(
    agent: DynConnectTo<Client>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    system_prompt: Option<String>,
    mode: Option<String>,
    user_prompt: String,
) -> AcpResult<AgentResponse> {
    let local_set = tokio::task::LocalSet::new();
    let cancel_token = CancellationToken::new();
    let cancel_for_main = cancel_token.clone();

    let result: AcpResult<AgentResponse> = local_set
        .run_until(async move {
            // Stand up a `ConnectionTo<Agent>` peer by running the inner
            // agent component as the server side of a freshly-built
            // Client connection. The main_fn body is where we issue
            // typed requests and assemble the final response.
            let connect_result = Client
                .builder()
                .name("swissarmyhammer-agent")
                .connect_with(agent, async move |cx: ConnectionTo<Agent>| {
                    let response = drive_prompt_turn(
                        &cx,
                        notification_rx,
                        cancel_for_main,
                        system_prompt,
                        mode,
                        user_prompt,
                    )
                    .await?;
                    Ok::<AgentResponse, agent_client_protocol::Error>(response)
                })
                .await;

            match connect_result {
                Ok(response) => Ok(response),
                Err(e) => Err(AcpError::PromptError(format!(
                    "ACP connection failed: {:?}",
                    e
                ))),
            }
        })
        .await;

    cancel_token.cancel();
    result
}

/// Issue the `initialize → new_session → set_session_mode → prompt`
/// sequence over `cx` and return the assembled response. Concurrent with
/// `prompt`, a notification collector drains streamed content from the
/// broadcast channel.
async fn drive_prompt_turn(
    cx: &ConnectionTo<Agent>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    cancel_token: CancellationToken,
    system_prompt: Option<String>,
    mode: Option<String>,
    user_prompt: String,
) -> Result<AgentResponse, agent_client_protocol::Error> {
    initialize_connection(cx).await.map_err(into_acp_error)?;
    let session_id = create_session_via_connection(cx, system_prompt)
        .await
        .map_err(into_acp_error)?;
    set_session_mode_via_connection(cx, &session_id, mode)
        .await
        .map_err(into_acp_error)?;

    // Spawn the per-session collector before issuing the prompt so it
    // captures streamed updates as they arrive.
    let collector = spawn_collector_task(notification_rx, session_id.clone(), cancel_token.clone());

    let prompt_request = PromptRequest::new(
        session_id,
        vec![ContentBlock::Text(TextContent::new(user_prompt))],
    );
    let prompt_response_result = cx
        .send_request(prompt_request)
        .block_task()
        .await
        .map_err(|e| {
            agent_client_protocol::Error::internal_error()
                .data(serde_json::json!({"error": format!("prompt failed: {}", e)}))
        });

    let collector_result = await_collector(collector, &cancel_token).await;
    let (response_text, messages_lost) = collector_result;

    let prompt_response = prompt_response_result?;
    Ok(build_agent_response(
        prompt_response,
        response_text,
        messages_lost,
    ))
}

/// Convert an `AcpError` into an `agent_client_protocol::Error` carrying
/// the original message in its `data` payload. Used to bubble setup
/// failures out of the `connect_with` main_fn closure.
fn into_acp_error(err: AcpError) -> agent_client_protocol::Error {
    agent_client_protocol::Error::internal_error()
        .data(serde_json::json!({"error": err.to_string()}))
}

/// Issue an `initialize` request advertising the same client
/// capabilities the legacy 0.10 helper used: read-only filesystem,
/// terminal disabled.
async fn initialize_connection(cx: &ConnectionTo<Agent>) -> AcpResult<()> {
    let init_request = InitializeRequest::new(1.into()).client_capabilities(
        ClientCapabilities::new()
            .fs(FileSystemCapabilities::new()
                .read_text_file(false)
                .write_text_file(false))
            .terminal(false),
    );

    let init_response = cx
        .send_request(init_request)
        .block_task()
        .await
        .map_err(|e| AcpError::InitializationError(format!("{:?}", e)))?;

    if let Some(ref info) = init_response.agent_info {
        tracing::debug!("Agent initialized: {}", Pretty(&info.name));
    }

    Ok(())
}

/// Create a new ACP session, optionally attaching a system prompt as
/// session metadata under the `system_prompt` key.
async fn create_session_via_connection(
    cx: &ConnectionTo<Agent>,
    system_prompt: Option<String>,
) -> AcpResult<SessionId> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut session_request = NewSessionRequest::new(cwd);

    if let Some(sys_prompt) = system_prompt {
        let mut meta = serde_json::Map::new();
        meta.insert("system_prompt".to_string(), serde_json::json!(sys_prompt));
        session_request = session_request.meta(meta);
    }

    let session_response = cx
        .send_request(session_request)
        .block_task()
        .await
        .map_err(|e| AcpError::SessionError(format!("{:?}", e)))?;

    tracing::debug!("Session created: {}", session_response.session_id);
    Ok(session_response.session_id)
}

/// Set the mode on `session_id` if a `mode` value is provided.
async fn set_session_mode_via_connection(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    mode: Option<String>,
) -> AcpResult<()> {
    if let Some(mode_id) = mode {
        let mode_id = SessionModeId::new(mode_id);
        let request = SetSessionModeRequest::new(session_id.clone(), mode_id.clone());
        cx.send_request(request).block_task().await.map_err(|e| {
            AcpError::SessionError(format!("Failed to set session mode '{}': {:?}", mode_id, e))
        })?;
        tracing::debug!("Session mode set to: {}", mode_id);
    }
    Ok(())
}

/// Extract text content from an agent notification if it matches our session.
///
/// Returns Some(text) if the notification is an AgentMessageChunk containing text
/// for the specified session, None otherwise.
fn extract_text_from_notification<'a>(
    notification: &'a SessionNotification,
    session_id: &SessionId,
) -> Option<&'a str> {
    // Check if notification is for our session
    if notification.session_id != *session_id {
        return None;
    }

    // Extract text from AgentMessageChunk updates
    let SessionUpdate::AgentMessageChunk(content_chunk) = &notification.update else {
        return None;
    };

    let ContentBlock::Text(text_content) = &content_chunk.content else {
        return None;
    };

    Some(&text_content.text)
}

/// Extract response content from agent metadata.
///
/// Tries claude_response first (claude-agent), then llama_response (llama-agent).
/// Returns empty string if neither is found.
fn extract_response_from_metadata(metadata: &Option<serde_json::Value>) -> String {
    let Some(meta) = metadata.as_ref() else {
        return String::new();
    };

    // Try claude_response first, then llama_response
    let response_value = meta
        .get("claude_response")
        .or_else(|| meta.get("llama_response"));

    response_value
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Spawn a `LocalSet`-friendly task that drains the per-session
/// broadcast receiver into a `String` while watching for cancellation.
fn spawn_collector_task(
    notification_rx: broadcast::Receiver<SessionNotification>,
    session_id: SessionId,
    cancel_token: CancellationToken,
) -> tokio::task::JoinHandle<(String, u64)> {
    tokio::task::spawn_local(notification_collector(
        notification_rx,
        session_id,
        cancel_token,
    ))
}

/// Body of the per-session notification collector.
async fn notification_collector(
    mut notification_rx: broadcast::Receiver<SessionNotification>,
    session_id: SessionId,
    cancel_token: CancellationToken,
) -> (String, u64) {
    let mut text = String::new();
    let mut messages_lost = 0u64;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                tracing::debug!("Collector received cancellation signal");
                break;
            }
            result = notification_rx.recv() => {
                match result {
                    Ok(notification) => {
                        if let Some(chunk) = extract_text_from_notification(&notification, &session_id) {
                            text.push_str(chunk);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        messages_lost += count;
                        tracing::warn!(
                            messages_lost = count,
                            total_lost = messages_lost,
                            "Notification receiver lagged, some content may be lost"
                        );
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::debug!("Notification channel closed");
                        break;
                    }
                }
            }
        }
    }

    (text, messages_lost)
}

/// Spawn a notification collector against a caller-supplied `LocalSet`.
///
/// Test-only helper that lets the unit tests run their assertions inside
/// `local_set.run_until(...)` without going through the full
/// `connect_with` plumbing.
#[cfg(test)]
fn spawn_notification_collector(
    local_set: &tokio::task::LocalSet,
    notification_rx: broadcast::Receiver<SessionNotification>,
    session_id: SessionId,
    cancel_token: CancellationToken,
) -> tokio::task::JoinHandle<(String, u64)> {
    local_set.spawn_local(notification_collector(
        notification_rx,
        session_id,
        cancel_token,
    ))
}

/// Wait for the notification collector to finish with timeout
async fn await_collector(
    collector_handle: tokio::task::JoinHandle<(String, u64)>,
    cancel_token: &CancellationToken,
) -> (String, u64) {
    // Give the collector a moment to finish processing remaining notifications
    tokio::time::sleep(tokio::time::Duration::from_millis(
        NOTIFICATION_COLLECTION_DELAY_MS,
    ))
    .await;
    cancel_token.cancel();

    match tokio::time::timeout(Duration::from_millis(500), collector_handle).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            tracing::warn!(error = ?e, "Notification collector task error, content may be lost");
            (String::new(), 0)
        }
        Err(_) => {
            tracing::warn!("Notification collector timed out after 500ms, content may be lost");
            (String::new(), 0)
        }
    }
}

/// Build the final AgentResponse from prompt result and collected text
fn build_agent_response(
    prompt_result: PromptResponse,
    response_text: String,
    messages_lost: u64,
) -> AgentResponse {
    if messages_lost > 0 {
        tracing::warn!(
            messages_lost = messages_lost,
            "Streaming collection completed with potential content loss due to backpressure"
        );
    }

    let response_type = match prompt_result.stop_reason {
        StopReason::EndTurn => AgentResponseType::Success,
        StopReason::MaxTokens | StopReason::MaxTurnRequests => AgentResponseType::Partial,
        StopReason::Refusal | StopReason::Cancelled => AgentResponseType::Error,
        _ => AgentResponseType::Partial,
    };

    let metadata = prompt_result.meta.map(serde_json::Value::Object);
    let content = if response_text.is_empty() {
        extract_response_from_metadata(&metadata)
    } else {
        response_text
    };

    AgentResponse {
        content,
        metadata,
        response_type,
    }
}

/// Inert placeholder used to back-fill `AcpAgentHandle::agent` after
/// [`execute_prompt`] consumes the real component.
///
/// Implements `ConnectTo<Client>` by failing on first use — calling code
/// should never reach the connection layer because the handle is
/// effectively single-shot once `execute_prompt` has been invoked.
struct NoopAgent;

impl agent_client_protocol::ConnectTo<Client> for NoopAgent {
    async fn connect_to(
        self,
        _client: impl agent_client_protocol::ConnectTo<
            <Client as agent_client_protocol::Role>::Counterpart,
        >,
    ) -> Result<(), agent_client_protocol::Error> {
        Err(agent_client_protocol::Error::internal_error().data(serde_json::json!({
            "error": "AcpAgentHandle::agent has been consumed by execute_prompt; create a fresh handle"
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_response_success() {
        let response = AgentResponse::success("Hello".to_string());
        assert!(response.is_success());
        assert!(!response.is_error());
        assert_eq!(response.content, "Hello");
    }

    #[test]
    fn test_agent_response_success_with_metadata() {
        let metadata = serde_json::json!({
            "model": "test-model",
            "tokens_used": 100
        });
        let response = AgentResponse::success_with_metadata("Hello".to_string(), metadata.clone());
        assert!(response.is_success());
        assert!(!response.is_error());
        assert_eq!(response.content, "Hello");
        assert!(response.metadata.is_some());
        let meta = response.metadata.unwrap();
        assert_eq!(
            meta.get("model").and_then(|v| v.as_str()),
            Some("test-model")
        );
        assert_eq!(meta.get("tokens_used").and_then(|v| v.as_i64()), Some(100));
    }

    #[test]
    fn test_agent_response_error() {
        let response = AgentResponse::error("Failed".to_string());
        assert!(!response.is_success());
        assert!(response.is_error());
    }

    #[test]
    fn test_agent_response_partial() {
        let response = AgentResponse::partial("Partial content".to_string());
        assert!(!response.is_success());
        assert!(!response.is_error());
        assert_eq!(response.content, "Partial content");
        assert!(response.metadata.is_none());
        // Verify it's actually a Partial type
        assert!(matches!(response.response_type, AgentResponseType::Partial));
    }

    #[test]
    fn test_agent_response_type_discrimination() {
        // Test that we can distinguish between all response types
        let success = AgentResponse::success("ok".to_string());
        let error = AgentResponse::error("err".to_string());
        let partial = AgentResponse::partial("part".to_string());

        assert!(matches!(success.response_type, AgentResponseType::Success));
        assert!(matches!(error.response_type, AgentResponseType::Error));
        assert!(matches!(partial.response_type, AgentResponseType::Partial));
    }

    #[test]
    fn test_mcp_config_from_port() {
        let config = McpServerConfig::from_port(8080);
        assert_eq!(config.url, "http://localhost:8080/mcp");
    }

    #[test]
    fn test_mcp_config_new() {
        let config = McpServerConfig::new("http://example.com/mcp");
        assert_eq!(config.url, "http://example.com/mcp");
    }

    #[test]
    fn test_acp_error_severity() {
        use swissarmyhammer_common::Severity;

        // Test that each error variant has the expected severity
        let init_err = AcpError::InitializationError("test".to_string());
        assert_eq!(init_err.severity(), ErrorSeverity::Critical);

        let session_err = AcpError::SessionError("test".to_string());
        assert_eq!(session_err.severity(), ErrorSeverity::Error);

        let prompt_err = AcpError::PromptError("test".to_string());
        assert_eq!(prompt_err.severity(), ErrorSeverity::Error);

        let not_avail_err = AcpError::AgentNotAvailable("test".to_string());
        assert_eq!(not_avail_err.severity(), ErrorSeverity::Critical);

        let config_err = AcpError::ConfigurationError("test".to_string());
        assert_eq!(config_err.severity(), ErrorSeverity::Error);

        let rate_limit_err = AcpError::RateLimit {
            message: "test".to_string(),
            wait_time: Duration::from_secs(60),
        };
        assert_eq!(rate_limit_err.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_acp_error_display() {
        // Test error message formatting for each variant
        let init_err = AcpError::InitializationError("init failed".to_string());
        assert_eq!(
            format!("{}", init_err),
            "Agent initialization failed: init failed"
        );

        let session_err = AcpError::SessionError("session failed".to_string());
        assert_eq!(
            format!("{}", session_err),
            "Session creation failed: session failed"
        );

        let prompt_err = AcpError::PromptError("prompt failed".to_string());
        assert_eq!(
            format!("{}", prompt_err),
            "Prompt execution failed: prompt failed"
        );

        let not_avail_err = AcpError::AgentNotAvailable("agent not found".to_string());
        assert_eq!(
            format!("{}", not_avail_err),
            "Agent not available: agent not found"
        );

        let config_err = AcpError::ConfigurationError("bad config".to_string());
        assert_eq!(format!("{}", config_err), "Configuration error: bad config");

        let rate_limit_err = AcpError::RateLimit {
            message: "too many requests".to_string(),
            wait_time: Duration::from_secs(60),
        };
        let display = format!("{}", rate_limit_err);
        assert!(display.contains("Rate limit reached"));
        assert!(display.contains("too many requests"));
    }

    #[test]
    fn test_extract_response_from_metadata_claude() {
        let metadata = Some(serde_json::json!({
            "claude_response": "Hello from Claude"
        }));
        let result = extract_response_from_metadata(&metadata);
        assert_eq!(result, "Hello from Claude");
    }

    #[test]
    fn test_extract_response_from_metadata_llama() {
        let metadata = Some(serde_json::json!({
            "llama_response": "Hello from Llama"
        }));
        let result = extract_response_from_metadata(&metadata);
        assert_eq!(result, "Hello from Llama");
    }

    #[test]
    fn test_extract_response_from_metadata_none() {
        let result = extract_response_from_metadata(&None);
        assert_eq!(result, "");
    }

    #[test]
    fn test_extract_response_from_metadata_empty() {
        let metadata = Some(serde_json::json!({}));
        let result = extract_response_from_metadata(&metadata);
        assert_eq!(result, "");
    }

    #[test]
    fn test_extract_response_from_metadata_prefers_claude() {
        // When both are present, claude_response should be preferred (it's checked first)
        let metadata = Some(serde_json::json!({
            "claude_response": "Claude wins",
            "llama_response": "Llama loses"
        }));
        let result = extract_response_from_metadata(&metadata);
        assert_eq!(result, "Claude wins");
    }

    #[test]
    fn test_build_agent_response_success() {
        let prompt_result = PromptResponse::new(StopReason::EndTurn);
        let response = build_agent_response(prompt_result, "Hello".to_string(), 0);
        assert!(response.is_success());
        assert_eq!(response.content, "Hello");
    }

    #[test]
    fn test_build_agent_response_partial_max_tokens() {
        let prompt_result = PromptResponse::new(StopReason::MaxTokens);
        let response = build_agent_response(prompt_result, "Partial".to_string(), 0);
        assert!(matches!(response.response_type, AgentResponseType::Partial));
    }

    #[test]
    fn test_build_agent_response_error_refusal() {
        let prompt_result = PromptResponse::new(StopReason::Refusal);
        let response = build_agent_response(prompt_result, "Refused".to_string(), 0);
        assert!(response.is_error());
    }

    #[test]
    fn test_build_agent_response_uses_metadata_when_text_empty() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "claude_response".to_string(),
            serde_json::json!("From metadata"),
        );
        let prompt_result = PromptResponse::new(StopReason::EndTurn).meta(meta);
        let response = build_agent_response(prompt_result, "".to_string(), 0);
        assert_eq!(response.content, "From metadata");
    }

    #[test]
    fn test_create_agent_options_defaults_to_non_ephemeral() {
        let options = CreateAgentOptions::default();
        assert!(!options.ephemeral);
    }

    #[test]
    fn test_create_agent_options_enables_ephemeral_mode() {
        let options = CreateAgentOptions {
            ephemeral: true,
            ..Default::default()
        };
        assert!(options.ephemeral);
    }

    // Note: Tests for create_agent() and execute_prompt() require external agent installations
    // (Claude CLI or Llama model). These functions are tested through integration tests
    // in the swissarmyhammer-cli crate where the agents are available in the test environment.
    // The helper functions they use (extract_response_from_metadata,
    // build_agent_response, etc.) are tested above to ensure correctness of the core logic.

    // ========================================================================
    // convert_model_source tests
    // ========================================================================

    #[test]
    fn test_convert_model_source_local_with_folder() {
        let source = swissarmyhammer_config::model::ModelSource::Local {
            filename: std::path::PathBuf::from("/models/my-model.gguf"),
            folder: Some(std::path::PathBuf::from("/models")),
        };
        let converted = convert_model_source(&source);
        match converted {
            llama_agent::types::ModelSource::Local { folder, filename } => {
                assert_eq!(folder, std::path::PathBuf::from("/models"));
                assert_eq!(filename, Some("my-model.gguf".to_string()));
            }
            _ => panic!("Expected Local variant"),
        }
    }

    #[test]
    fn test_convert_model_source_local_without_folder() {
        let source = swissarmyhammer_config::model::ModelSource::Local {
            filename: std::path::PathBuf::from("/models/my-model.gguf"),
            folder: None,
        };
        let converted = convert_model_source(&source);
        match converted {
            llama_agent::types::ModelSource::Local { folder, filename } => {
                assert_eq!(folder, std::path::PathBuf::from("/models"));
                assert_eq!(filename, Some("my-model.gguf".to_string()));
            }
            _ => panic!("Expected Local variant"),
        }
    }

    #[test]
    fn test_convert_model_source_local_bare_filename() {
        // When filename has no parent directory component, parent() returns Some("")
        let source = swissarmyhammer_config::model::ModelSource::Local {
            filename: std::path::PathBuf::from("model.gguf"),
            folder: None,
        };
        let converted = convert_model_source(&source);
        match converted {
            llama_agent::types::ModelSource::Local { folder, filename } => {
                // "model.gguf".parent() returns Some(""), not None
                assert_eq!(folder, std::path::PathBuf::from(""));
                assert_eq!(filename, Some("model.gguf".to_string()));
            }
            _ => panic!("Expected Local variant"),
        }
    }

    #[test]
    fn test_convert_model_source_huggingface_with_filename() {
        let source = swissarmyhammer_config::model::ModelSource::HuggingFace {
            repo: "TheBloke/Llama-2-7B-GGUF".to_string(),
            filename: Some("llama-2-7b.Q4_K_M.gguf".to_string()),
            folder: None,
        };
        let converted = convert_model_source(&source);
        match converted {
            llama_agent::types::ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                assert_eq!(repo, "TheBloke/Llama-2-7B-GGUF");
                assert_eq!(filename, Some("llama-2-7b.Q4_K_M.gguf".to_string()));
                assert_eq!(folder, None);
            }
            _ => panic!("Expected HuggingFace variant"),
        }
    }

    #[test]
    fn test_convert_model_source_huggingface_with_folder() {
        // When folder is Some, filename should be None regardless of input
        let source = swissarmyhammer_config::model::ModelSource::HuggingFace {
            repo: "org/model".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: Some("subfolder".to_string()),
        };
        let converted = convert_model_source(&source);
        match converted {
            llama_agent::types::ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                assert_eq!(repo, "org/model");
                assert_eq!(filename, None);
                assert_eq!(folder, Some("subfolder".to_string()));
            }
            _ => panic!("Expected HuggingFace variant"),
        }
    }

    #[test]
    fn test_convert_model_source_huggingface_no_filename_no_folder() {
        let source = swissarmyhammer_config::model::ModelSource::HuggingFace {
            repo: "org/model".to_string(),
            filename: None,
            folder: None,
        };
        let converted = convert_model_source(&source);
        match converted {
            llama_agent::types::ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                assert_eq!(repo, "org/model");
                assert_eq!(filename, None);
                assert_eq!(folder, None);
            }
            _ => panic!("Expected HuggingFace variant"),
        }
    }

    // ========================================================================
    // build_llama_model_config tests
    // ========================================================================

    #[test]
    fn test_build_llama_model_config_defaults() {
        let llama_config = swissarmyhammer_config::model::LlamaAgentConfig::default();
        let model_config = build_llama_model_config(&llama_config);

        assert_eq!(model_config.retry_config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(
            model_config.retry_config.initial_delay_ms,
            DEFAULT_INITIAL_RETRY_DELAY_MS
        );
        assert!(
            (model_config.retry_config.backoff_multiplier - DEFAULT_BACKOFF_MULTIPLIER).abs()
                < f64::EPSILON
        );
        assert_eq!(
            model_config.retry_config.max_delay_ms,
            DEFAULT_MAX_RETRY_DELAY_MS
        );
        assert!(!model_config.debug);
        assert_eq!(model_config.n_seq_max, 1);
        assert_eq!(model_config.n_threads, DEFAULT_NUM_THREADS);
        assert_eq!(model_config.n_threads_batch, DEFAULT_BATCH_THREADS);
    }

    #[test]
    fn test_build_llama_model_config_preserves_model_params() {
        let mut llama_config = swissarmyhammer_config::model::LlamaAgentConfig::default();
        llama_config.model.batch_size = 128;
        llama_config.model.use_hf_params = true;

        let model_config = build_llama_model_config(&llama_config);
        assert_eq!(model_config.batch_size, 128);
        assert!(model_config.use_hf_params);
    }

    // ========================================================================
    // build_llama_mcp_servers tests
    // ========================================================================

    #[test]
    fn test_build_llama_mcp_servers_with_config() {
        let mcp = McpServerConfig::from_port(9090);
        let servers = build_llama_mcp_servers(Some(&mcp), 120);

        assert_eq!(servers.len(), 1);
        match &servers[0] {
            llama_agent::types::MCPServerConfig::Http(http) => {
                assert_eq!(http.name, "swissarmyhammer");
                assert_eq!(http.url, "http://localhost:9090/mcp");
                assert_eq!(http.timeout_secs, Some(120));
                assert_eq!(http.sse_keep_alive_secs, Some(SSE_KEEP_ALIVE_SECONDS));
                assert!(!http.stateful_mode);
            }
            _ => panic!("Expected Http variant"),
        }
    }

    #[test]
    fn test_build_llama_mcp_servers_without_config() {
        let servers = build_llama_mcp_servers(None, 60);
        assert!(servers.is_empty());
    }

    // ========================================================================
    // convert_mcp_servers_to_acp tests
    // ========================================================================

    #[test]
    fn test_convert_mcp_servers_to_acp_http() {
        let servers = vec![llama_agent::types::MCPServerConfig::Http(
            llama_agent::types::HttpServerConfig {
                name: "test-http".to_string(),
                url: "http://localhost:8080/mcp".to_string(),
                timeout_secs: Some(30),
                sse_keep_alive_secs: Some(15),
                stateful_mode: false,
            },
        )];
        let acp_servers = convert_mcp_servers_to_acp(&servers);

        assert_eq!(acp_servers.len(), 1);
        match &acp_servers[0] {
            schema::McpServer::Http(http) => {
                assert_eq!(http.name, "test-http");
                assert_eq!(http.url, "http://localhost:8080/mcp");
            }
            other => panic!("Expected Http variant, got {:?}", other),
        }
    }

    #[test]
    fn test_convert_mcp_servers_to_acp_in_process() {
        let servers = vec![llama_agent::types::MCPServerConfig::InProcess(
            llama_agent::types::ProcessServerConfig {
                name: "test-stdio".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string(), "world".to_string()],
                timeout_secs: None,
            },
        )];
        let acp_servers = convert_mcp_servers_to_acp(&servers);

        assert_eq!(acp_servers.len(), 1);
        match &acp_servers[0] {
            schema::McpServer::Stdio(stdio) => {
                assert_eq!(stdio.name, "test-stdio");
                assert_eq!(stdio.args, vec!["hello".to_string(), "world".to_string()]);
            }
            other => panic!("Expected Stdio variant, got {:?}", other),
        }
    }

    #[test]
    fn test_convert_mcp_servers_to_acp_empty() {
        let servers: Vec<llama_agent::types::MCPServerConfig> = vec![];
        let acp_servers = convert_mcp_servers_to_acp(&servers);
        assert!(acp_servers.is_empty());
    }

    #[test]
    fn test_convert_mcp_servers_to_acp_mixed() {
        let servers = vec![
            llama_agent::types::MCPServerConfig::Http(llama_agent::types::HttpServerConfig {
                name: "http-server".to_string(),
                url: "http://localhost:8080/mcp".to_string(),
                timeout_secs: None,
                sse_keep_alive_secs: None,
                stateful_mode: false,
            }),
            llama_agent::types::MCPServerConfig::InProcess(
                llama_agent::types::ProcessServerConfig {
                    name: "stdio-server".to_string(),
                    command: "node".to_string(),
                    args: vec!["server.js".to_string()],
                    timeout_secs: None,
                },
            ),
        ];
        let acp_servers = convert_mcp_servers_to_acp(&servers);
        assert_eq!(acp_servers.len(), 2);
        assert!(matches!(&acp_servers[0], schema::McpServer::Http(_)));
        assert!(matches!(&acp_servers[1], schema::McpServer::Stdio(_)));
    }

    // ========================================================================
    // extract_text_from_notification tests
    // ========================================================================

    /// Helper to create a text notification for a given session
    fn make_text_notification(session_id: &SessionId, text: &str) -> SessionNotification {
        SessionNotification::new(
            session_id.clone(),
            SessionUpdate::AgentMessageChunk(schema::ContentChunk::new(ContentBlock::Text(
                TextContent::new(text),
            ))),
        )
    }

    #[test]
    fn test_extract_text_from_notification_matching_session() {
        let session_id = SessionId::new("test-session".to_string());
        let notification = make_text_notification(&session_id, "Hello, world!");

        let result = extract_text_from_notification(&notification, &session_id);
        assert_eq!(result, Some("Hello, world!"));
    }

    #[test]
    fn test_extract_text_from_notification_wrong_session() {
        let session_id = SessionId::new("session-1".to_string());
        let other_session = SessionId::new("session-2".to_string());
        let notification = make_text_notification(&other_session, "Hello");

        let result = extract_text_from_notification(&notification, &session_id);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_text_from_notification_non_text_content() {
        let session_id = SessionId::new("test-session".to_string());
        // Use Image content block as a non-Text variant
        let image = schema::ImageContent::new("data".to_string(), "image/png".to_string());
        let notification = SessionNotification::new(
            session_id.clone(),
            SessionUpdate::AgentMessageChunk(schema::ContentChunk::new(ContentBlock::Image(image))),
        );

        let result = extract_text_from_notification(&notification, &session_id);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_text_from_notification_non_chunk_update() {
        let session_id = SessionId::new("test-session".to_string());
        // Use ToolCall update as a non-AgentMessageChunk variant
        let tool_call = schema::ToolCall::new(
            schema::ToolCallId::new("tc-1".to_string()),
            "test-tool".to_string(),
        );
        let notification =
            SessionNotification::new(session_id.clone(), SessionUpdate::ToolCall(tool_call));

        let result = extract_text_from_notification(&notification, &session_id);
        assert_eq!(result, None);
    }

    // ========================================================================
    // build_agent_response - additional stop reasons
    // ========================================================================

    #[test]
    fn test_build_agent_response_cancelled() {
        let prompt_result = PromptResponse::new(StopReason::Cancelled);
        let response = build_agent_response(prompt_result, "Cancelled content".to_string(), 0);
        assert!(response.is_error());
        assert_eq!(response.content, "Cancelled content");
    }

    #[test]
    fn test_build_agent_response_max_turn_requests() {
        let prompt_result = PromptResponse::new(StopReason::MaxTurnRequests);
        let response = build_agent_response(prompt_result, "Turn limit".to_string(), 0);
        assert!(matches!(response.response_type, AgentResponseType::Partial));
    }

    #[test]
    fn test_build_agent_response_with_messages_lost() {
        let prompt_result = PromptResponse::new(StopReason::EndTurn);
        let response = build_agent_response(prompt_result, "Some content".to_string(), 5);
        // Even with messages lost, response is still built based on stop reason
        assert!(response.is_success());
        assert_eq!(response.content, "Some content");
    }

    #[test]
    fn test_build_agent_response_with_metadata() {
        let mut meta = serde_json::Map::new();
        meta.insert("key".to_string(), serde_json::json!("value"));
        let prompt_result = PromptResponse::new(StopReason::EndTurn).meta(meta);
        let response = build_agent_response(prompt_result, "Content".to_string(), 0);
        assert!(response.metadata.is_some());
        let metadata = response.metadata.unwrap();
        assert_eq!(metadata.get("key").and_then(|v| v.as_str()), Some("value"));
    }

    #[test]
    fn test_build_agent_response_empty_text_uses_metadata() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "llama_response".to_string(),
            serde_json::json!("From llama metadata"),
        );
        let prompt_result = PromptResponse::new(StopReason::EndTurn).meta(meta);
        let response = build_agent_response(prompt_result, "".to_string(), 0);
        assert_eq!(response.content, "From llama metadata");
    }

    #[test]
    fn test_build_agent_response_no_metadata_empty_text() {
        let prompt_result = PromptResponse::new(StopReason::EndTurn);
        let response = build_agent_response(prompt_result, "".to_string(), 0);
        assert_eq!(response.content, "");
    }

    // ========================================================================
    // extract_response_from_metadata - non-string values
    // ========================================================================

    #[test]
    fn test_extract_response_from_metadata_numeric_value() {
        let metadata = Some(serde_json::json!({
            "claude_response": 42
        }));
        // Non-string values should return empty string
        let result = extract_response_from_metadata(&metadata);
        assert_eq!(result, "");
    }

    // ========================================================================
    // AgentResponse serialization
    // ========================================================================

    #[test]
    fn test_agent_response_serialization_roundtrip() {
        let response = AgentResponse::success("test content".to_string());
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: AgentResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, "test content");
        assert!(deserialized.is_success());
        assert!(deserialized.metadata.is_none());
    }

    #[test]
    fn test_agent_response_serialization_with_metadata() {
        let response = AgentResponse::success_with_metadata(
            "content".to_string(),
            serde_json::json!({"model": "test"}),
        );
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: AgentResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, "content");
        assert!(deserialized.metadata.is_some());
    }

    #[test]
    fn test_agent_response_camel_case_serialization() {
        let response = AgentResponse::success("test".to_string());
        let json = serde_json::to_string(&response).unwrap();
        // Verify camelCase field names
        assert!(json.contains("responseType"));
        assert!(!json.contains("response_type"));
    }

    // ========================================================================
    // CreateAgentOptions tests
    // ========================================================================

    #[test]
    fn test_create_agent_options_tools_override_none_by_default() {
        let options = CreateAgentOptions::default();
        assert!(options.tools_override.is_none());
    }

    #[test]
    fn test_create_agent_options_tools_override_empty_string() {
        let options = CreateAgentOptions {
            ephemeral: false,
            tools_override: Some("".to_string()),
        };
        assert_eq!(options.tools_override, Some("".to_string()));
    }

    #[test]
    fn test_create_agent_options_debug() {
        let options = CreateAgentOptions {
            ephemeral: true,
            tools_override: Some("custom".to_string()),
        };
        let debug = format!("{:?}", options);
        assert!(debug.contains("ephemeral: true"));
        assert!(debug.contains("custom"));
    }

    #[test]
    fn test_create_agent_options_clone() {
        let options = CreateAgentOptions {
            ephemeral: true,
            tools_override: Some("tools".to_string()),
        };
        let cloned = options.clone();
        assert!(cloned.ephemeral);
        assert_eq!(cloned.tools_override, Some("tools".to_string()));
    }

    // ========================================================================
    // McpServerConfig tests
    // ========================================================================

    #[test]
    fn test_mcp_server_config_debug() {
        let config = McpServerConfig::new("http://example.com/mcp");
        let debug = format!("{:?}", config);
        assert!(debug.contains("http://example.com/mcp"));
    }

    #[test]
    fn test_mcp_server_config_clone() {
        let config = McpServerConfig::from_port(3000);
        let cloned = config.clone();
        assert_eq!(cloned.url, "http://localhost:3000/mcp");
    }

    #[test]
    fn test_mcp_server_config_various_ports() {
        for port in [0, 80, 443, 8080, 65535] {
            let config = McpServerConfig::from_port(port);
            assert_eq!(config.url, format!("http://localhost:{}/mcp", port));
        }
    }

    // ========================================================================
    // AcpError Debug/Display tests
    // ========================================================================

    #[test]
    fn test_acp_error_debug_format() {
        let err = AcpError::RateLimit {
            message: "slow down".to_string(),
            wait_time: Duration::from_secs(30),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("RateLimit"));
        assert!(debug.contains("slow down"));
    }

    // ========================================================================
    // AcpResult type alias tests
    // ========================================================================

    #[test]
    fn test_acp_result_ok() {
        let result: AcpResult<i32> = Ok(42);
        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn test_acp_result_err() {
        let result: AcpResult<i32> = Err(AcpError::PromptError("fail".to_string()));
        assert!(result.is_err());
    }

    // ========================================================================
    // Notification collector async tests
    // ========================================================================

    #[tokio::test]
    async fn test_spawn_notification_collector_receives_text() {
        let (tx, rx) = broadcast::channel::<SessionNotification>(16);
        let session_id = SessionId::new("test-session".to_string());
        let cancel_token = CancellationToken::new();
        let local_set = tokio::task::LocalSet::new();

        let collector_handle =
            spawn_notification_collector(&local_set, rx, session_id.clone(), cancel_token.clone());

        let cancel_clone = cancel_token.clone();
        let session_clone = session_id.clone();
        local_set
            .run_until(async move {
                // Send a few text notifications
                tx.send(make_text_notification(&session_clone, "Hello "))
                    .unwrap();
                tx.send(make_text_notification(&session_clone, "World"))
                    .unwrap();

                // Give the collector time to process
                tokio::time::sleep(Duration::from_millis(50)).await;
                cancel_clone.cancel();

                let (text, lost) = collector_handle.await.unwrap();
                assert_eq!(text, "Hello World");
                assert_eq!(lost, 0);
            })
            .await;
    }

    #[tokio::test]
    async fn test_spawn_notification_collector_ignores_other_sessions() {
        let (tx, rx) = broadcast::channel::<SessionNotification>(16);
        let session_id = SessionId::new("my-session".to_string());
        let other_session = SessionId::new("other-session".to_string());
        let cancel_token = CancellationToken::new();
        let local_set = tokio::task::LocalSet::new();

        let collector_handle =
            spawn_notification_collector(&local_set, rx, session_id.clone(), cancel_token.clone());

        let cancel_clone = cancel_token.clone();
        local_set
            .run_until(async move {
                // Send notification for different session
                tx.send(make_text_notification(&other_session, "Should be ignored"))
                    .unwrap();

                tokio::time::sleep(Duration::from_millis(50)).await;
                cancel_clone.cancel();

                let (text, lost) = collector_handle.await.unwrap();
                assert_eq!(text, "");
                assert_eq!(lost, 0);
            })
            .await;
    }

    #[tokio::test]
    async fn test_spawn_notification_collector_channel_closed() {
        let (tx, rx) = broadcast::channel::<SessionNotification>(16);
        let session_id = SessionId::new("test".to_string());
        let cancel_token = CancellationToken::new();
        let local_set = tokio::task::LocalSet::new();

        let collector_handle =
            spawn_notification_collector(&local_set, rx, session_id, cancel_token.clone());

        local_set
            .run_until(async move {
                // Drop the sender to close the channel
                drop(tx);

                let (text, lost) = collector_handle.await.unwrap();
                assert_eq!(text, "");
                assert_eq!(lost, 0);
            })
            .await;
    }

    #[tokio::test]
    async fn test_spawn_notification_collector_ignores_non_text_updates() {
        let (tx, rx) = broadcast::channel::<SessionNotification>(16);
        let session_id = SessionId::new("test".to_string());
        let cancel_token = CancellationToken::new();
        let local_set = tokio::task::LocalSet::new();

        let collector_handle =
            spawn_notification_collector(&local_set, rx, session_id.clone(), cancel_token.clone());

        let cancel_clone = cancel_token.clone();
        let session_clone = session_id.clone();
        local_set
            .run_until(async move {
                // Send non-chunk update (ToolCall, not AgentMessageChunk)
                let tool_call = schema::ToolCall::new(
                    schema::ToolCallId::new("tc-1".to_string()),
                    "test-tool".to_string(),
                );
                tx.send(SessionNotification::new(
                    session_clone.clone(),
                    SessionUpdate::ToolCall(tool_call),
                ))
                .unwrap();

                // Send text
                tx.send(make_text_notification(&session_clone, "Only this"))
                    .unwrap();

                tokio::time::sleep(Duration::from_millis(50)).await;
                cancel_clone.cancel();

                let (text, _) = collector_handle.await.unwrap();
                assert_eq!(text, "Only this");
            })
            .await;
    }

    // ========================================================================
    // await_collector tests
    // ========================================================================

    #[tokio::test]
    async fn test_await_collector_success() {
        let local_set = tokio::task::LocalSet::new();
        let cancel_token = CancellationToken::new();

        let handle = local_set.spawn_local(async { ("collected text".to_string(), 2u64) });

        local_set
            .run_until(async {
                let (text, lost) = await_collector(handle, &cancel_token).await;
                assert_eq!(text, "collected text");
                assert_eq!(lost, 2);
            })
            .await;
    }

    // ========================================================================
    // Constants verification tests
    // ========================================================================

    #[test]
    fn test_constants_have_sane_values() {
        assert_ne!(MAX_PROMPT_LENGTH_BYTES, 0);
        assert_ne!(DEFAULT_MAX_RETRIES, 0);
        assert_ne!(DEFAULT_INITIAL_RETRY_DELAY_MS, 0);
        // Backoff multiplier must exceed 1.0
        let multiplier = DEFAULT_BACKOFF_MULTIPLIER;
        assert!(
            multiplier > 1.0,
            "backoff multiplier must be > 1.0, got {multiplier}"
        );
        // Max retry delay must be >= initial
        let (max_delay, init_delay) = (DEFAULT_MAX_RETRY_DELAY_MS, DEFAULT_INITIAL_RETRY_DELAY_MS);
        assert!(
            max_delay >= init_delay,
            "max retry delay must be >= initial, got {max_delay} < {init_delay}"
        );
        assert_ne!(DEFAULT_NUM_THREADS, 0);
        assert_ne!(DEFAULT_BATCH_THREADS, 0);
        assert_ne!(SSE_KEEP_ALIVE_SECONDS, 0);
        assert_ne!(DEFAULT_MAX_QUEUE_SIZE, 0);
        assert_ne!(NOTIFICATION_COLLECTION_DELAY_MS, 0);
    }

    // ========================================================================
    // AgentResponseType discrimination tests
    // ========================================================================

    #[test]
    fn test_agent_response_type_debug() {
        assert!(format!("{:?}", AgentResponseType::Success).contains("Success"));
        assert!(format!("{:?}", AgentResponseType::Partial).contains("Partial"));
        assert!(format!("{:?}", AgentResponseType::Error).contains("Error"));
    }

    #[test]
    fn test_agent_response_type_clone() {
        let t = AgentResponseType::Partial;
        let cloned = t.clone();
        assert!(matches!(cloned, AgentResponseType::Partial));
    }
}
