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
//!   - swissarmyhammer-workflow (for workflow prompt actions)
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
//! underlying agent implementations directly via `agent_client_protocol::Agent`.
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
//! let response = execute_prompt(&mut handle, None, "Hello!".to_string()).await?;
//! println!("{}", response.content);
//! ```

use agent_client_protocol::{
    Agent, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, SessionModeId,
    SessionNotification, SessionUpdate, SetSessionModeRequest, StopReason, TextContent,
};
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

/// Wrapper around an ACP agent with notification receiver
pub struct AcpAgentHandle {
    /// The ACP agent
    pub agent: Arc<dyn Agent + Send + Sync>,
    /// Notification receiver for streaming content
    pub notification_rx: broadcast::Receiver<SessionNotification>,
}

/// Options for agent creation
#[derive(Debug, Clone, Default)]
pub struct CreateAgentOptions {
    /// Use ephemeral mode (haiku model, no session persistence).
    /// Ideal for quick, stateless operations like scaffold generation.
    pub ephemeral: bool,
}

/// Create an ACP agent based on model configuration
///
/// Returns an AcpAgentHandle containing the agent and notification receiver.
/// The agent is wrapped with TracingAgent for unified logging, and notifications
/// are traced through trace_notifications.
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
    let (agent_name, handle) = match config.executor_type() {
        ModelExecutorType::ClaudeCode => {
            let handle = create_claude_agent(mcp_config, options.ephemeral).await?;
            ("Claude", handle)
        }
        ModelExecutorType::LlamaAgent => {
            let llama_config = match &config.executor {
                ModelExecutorConfig::LlamaAgent(cfg) => cfg.clone(),
                _ => {
                    return Err(AcpError::ConfigurationError(
                        "Expected LlamaAgent configuration".to_string(),
                    ))
                }
            };
            let handle = create_llama_agent(llama_config, mcp_config).await?;
            ("Llama", handle)
        }
        ModelExecutorType::LlamaEmbedding => {
            return Err(AcpError::ConfigurationError(
                "Embedding models cannot be used as agents".to_string(),
            ))
        }
    };

    // Wrap agent with TracingAgent for unified logging
    let traced_agent = TracingAgent::new(handle.agent, agent_name);

    // Wrap notification receiver with tracing
    let traced_rx = trace_notifications(agent_name.to_string(), handle.notification_rx);

    Ok(AcpAgentHandle {
        agent: Arc::new(traced_agent),
        notification_rx: traced_rx,
    })
}

/// Create a Claude ACP agent
async fn create_claude_agent(
    mcp_config: Option<McpServerConfig>,
    ephemeral: bool,
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

    // Create the Claude agent
    let (agent, notification_rx) =
        claude_agent::ClaudeAgent::new(agent_config)
            .await
            .map_err(|e| {
                AcpError::InitializationError(format!("Failed to create Claude agent: {}", e))
            })?;

    Ok(AcpAgentHandle {
        agent: Arc::new(agent),
        notification_rx,
    })
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
) -> Vec<agent_client_protocol::McpServer> {
    mcp_servers
        .iter()
        .map(|server| match server {
            llama_agent::types::MCPServerConfig::Http(http_config) => {
                agent_client_protocol::McpServer::Http(agent_client_protocol::McpServerHttp::new(
                    http_config.name.clone(),
                    http_config.url.clone(),
                ))
            }
            llama_agent::types::MCPServerConfig::InProcess(process_config) => {
                let mut stdio_server = agent_client_protocol::McpServerStdio::new(
                    process_config.name.clone(),
                    &process_config.command,
                );
                stdio_server.args = process_config.args.clone();
                agent_client_protocol::McpServer::Stdio(stdio_server)
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

    // Create the ACP server (which implements Agent trait)
    let (acp_server, notification_rx) =
        llama_agent::AcpServer::new(Arc::new(agent_server), acp_config);

    Ok(AcpAgentHandle {
        agent: Arc::new(acp_server),
        notification_rx,
    })
}

/// Execute a prompt using an ACP agent
///
/// This creates a new session, optionally sets a mode, sends the prompt,
/// collects streamed content from notifications, and returns the response.
///
/// Note: This function uses a dedicated current-thread runtime because the ACP
/// Agent trait methods return non-Send futures.
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
    // Clone what we need to move into the blocking task
    let agent = Arc::clone(&handle.agent);
    let notification_rx = handle.notification_rx.resubscribe();

    // Run the agent interaction in a spawn_blocking task with its own runtime
    // because ACP Agent trait methods return non-Send futures
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                AcpError::InitializationError(format!("Failed to create runtime: {}", e))
            })?;

        rt.block_on(async move {
            execute_prompt_impl(agent, notification_rx, system_prompt, mode, user_prompt).await
        })
    })
    .await
    .map_err(|e| AcpError::PromptError(format!("Task join error: {:?}", e)))??;

    Ok(result)
}

/// Extract text content from an agent notification if it matches our session.
///
/// Returns Some(text) if the notification is an AgentMessageChunk containing text
/// for the specified session, None otherwise.
fn extract_text_from_notification<'a>(
    notification: &'a SessionNotification,
    session_id: &agent_client_protocol::SessionId,
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

/// Initialize an ACP agent with standard client capabilities
async fn initialize_agent(agent: &Arc<dyn Agent + Send + Sync>) -> AcpResult<()> {
    let init_request = InitializeRequest::new(agent_client_protocol::ProtocolVersion::V1)
        .client_capabilities(
            agent_client_protocol::ClientCapabilities::new()
                .fs(agent_client_protocol::FileSystemCapability::new()
                    .read_text_file(false)
                    .write_text_file(false))
                .terminal(false),
        );

    let init_response = agent
        .initialize(init_request)
        .await
        .map_err(|e| AcpError::InitializationError(format!("{:?}", e)))?;

    if let Some(ref info) = init_response.agent_info {
        tracing::debug!("Agent initialized: {}", Pretty(&info.name));
    }

    Ok(())
}

/// Create a new ACP session with optional system prompt
async fn create_session(
    agent: &Arc<dyn Agent + Send + Sync>,
    system_prompt: Option<String>,
) -> AcpResult<agent_client_protocol::SessionId> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut session_request = NewSessionRequest::new(cwd);

    if let Some(sys_prompt) = system_prompt {
        let mut meta = serde_json::Map::new();
        meta.insert("system_prompt".to_string(), serde_json::json!(sys_prompt));
        session_request = session_request.meta(meta);
    }

    let session_response = agent
        .new_session(session_request)
        .await
        .map_err(|e| AcpError::SessionError(format!("{:?}", e)))?;

    tracing::debug!("Session created: {}", session_response.session_id);
    Ok(session_response.session_id)
}

/// Set session mode if provided
async fn set_session_mode(
    agent: &Arc<dyn Agent + Send + Sync>,
    session_id: &agent_client_protocol::SessionId,
    mode: Option<String>,
) -> AcpResult<()> {
    if let Some(mode_id) = mode {
        let mode_id = SessionModeId::new(mode_id);
        let set_mode_request = SetSessionModeRequest::new(session_id.clone(), mode_id.clone());
        agent
            .set_session_mode(set_mode_request)
            .await
            .map_err(|e| {
                AcpError::SessionError(format!("Failed to set session mode '{}': {:?}", mode_id, e))
            })?;
        tracing::debug!("Session mode set to: {}", mode_id);
    }
    Ok(())
}

/// Spawn a local task to collect notifications for a session
fn spawn_notification_collector(
    local_set: &tokio::task::LocalSet,
    mut notification_rx: broadcast::Receiver<SessionNotification>,
    session_id: agent_client_protocol::SessionId,
    cancel_token: CancellationToken,
) -> tokio::task::JoinHandle<(String, u64)> {
    local_set.spawn_local(async move {
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
    })
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
    prompt_result: agent_client_protocol::PromptResponse,
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

/// Implementation of execute_prompt that runs on a single-threaded runtime
async fn execute_prompt_impl(
    agent: Arc<dyn Agent + Send + Sync>,
    notification_rx: broadcast::Receiver<SessionNotification>,
    system_prompt: Option<String>,
    mode: Option<String>,
    user_prompt: String,
) -> AcpResult<AgentResponse> {
    initialize_agent(&agent).await?;
    let session_id = create_session(&agent, system_prompt).await?;
    set_session_mode(&agent, &session_id, mode).await?;

    let prompt_content = vec![ContentBlock::Text(TextContent::new(user_prompt))];
    let cancel_token = CancellationToken::new();
    let local_set = tokio::task::LocalSet::new();

    let collector_handle = spawn_notification_collector(
        &local_set,
        notification_rx,
        session_id.clone(),
        cancel_token.clone(),
    );

    let (prompt_result, collector_result) = local_set
        .run_until(async {
            let prompt_response = agent
                .prompt(PromptRequest::new(session_id.clone(), prompt_content))
                .await
                .map_err(|e| AcpError::PromptError(format!("{:?}", e)))?;

            let collector_result = await_collector(collector_handle, &cancel_token).await;
            Ok::<_, AcpError>((prompt_response, collector_result))
        })
        .await?;

    let (response_text, messages_lost) = collector_result;
    Ok(build_agent_response(
        prompt_result,
        response_text,
        messages_lost,
    ))
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
        let prompt_result = agent_client_protocol::PromptResponse::new(StopReason::EndTurn);
        let response = build_agent_response(prompt_result, "Hello".to_string(), 0);
        assert!(response.is_success());
        assert_eq!(response.content, "Hello");
    }

    #[test]
    fn test_build_agent_response_partial_max_tokens() {
        let prompt_result = agent_client_protocol::PromptResponse::new(StopReason::MaxTokens);
        let response = build_agent_response(prompt_result, "Partial".to_string(), 0);
        assert!(matches!(response.response_type, AgentResponseType::Partial));
    }

    #[test]
    fn test_build_agent_response_error_refusal() {
        let prompt_result = agent_client_protocol::PromptResponse::new(StopReason::Refusal);
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
        let prompt_result =
            agent_client_protocol::PromptResponse::new(StopReason::EndTurn).meta(meta);
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
        let options = CreateAgentOptions { ephemeral: true };
        assert!(options.ephemeral);
    }

    // Note: Tests for create_agent() and execute_prompt() require external agent installations
    // (Claude CLI or Llama model). These functions are tested through integration tests
    // in the swissarmyhammer-workflow and swissarmyhammer-cli crates where the agents are
    // available in the test environment. The helper functions they use (extract_response_from_metadata,
    // build_agent_response, etc.) are tested above to ensure correctness of the core logic.
}
