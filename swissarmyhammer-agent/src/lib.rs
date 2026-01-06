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

/// Result type for ACP operations
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
pub enum AgentResponseType {
    /// Standard successful text response
    Success,
    /// Partial response (streaming, timeout, etc.)
    Partial,
    /// Error response with error details
    Error,
}

impl AgentResponse {
    /// Create a successful response
    pub fn success(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Success,
        }
    }

    /// Create a successful response with metadata
    pub fn success_with_metadata(content: String, metadata: serde_json::Value) -> Self {
        Self {
            content,
            metadata: Some(metadata),
            response_type: AgentResponseType::Success,
        }
    }

    /// Create an error response
    pub fn error(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Error,
        }
    }

    /// Create a partial response
    pub fn partial(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Partial,
        }
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
    let (agent_name, handle) = match config.executor_type() {
        ModelExecutorType::ClaudeCode => {
            let handle = create_claude_agent(mcp_config).await?;
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
async fn create_claude_agent(mcp_config: Option<McpServerConfig>) -> AcpResult<AcpAgentHandle> {
    // Check if Claude CLI is available (claude-agent requires this)
    if which::which("claude").is_err() {
        return Err(AcpError::AgentNotAvailable(
            "Claude CLI not found in PATH. Install with: npm install -g @anthropic-ai/claude-code"
                .to_string(),
        ));
    }

    // Create Claude agent configuration with MCP servers
    let mut agent_config = claude_agent::config::AgentConfig::default();

    // Configure MCP server if provided (using HTTP transport)
    if let Some(mcp) = mcp_config {
        agent_config.mcp_servers = vec![claude_agent::config::McpServerConfig::Http(
            claude_agent::config::HttpTransport {
                transport_type: "http".to_string(),
                name: "swissarmyhammer".to_string(),
                url: mcp.url,
                headers: vec![],
            },
        )];
    }

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

/// Create a Llama ACP agent
async fn create_llama_agent(
    llama_config: swissarmyhammer_config::model::LlamaAgentConfig,
    mcp_config: Option<McpServerConfig>,
) -> AcpResult<AcpAgentHandle> {
    // Build llama-agent AgentConfig
    let model_source = match &llama_config.model.source {
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
    };

    let model_config = llama_agent::types::ModelConfig {
        source: model_source,
        batch_size: llama_config.model.batch_size,
        use_hf_params: llama_config.model.use_hf_params,
        retry_config: llama_agent::types::RetryConfig {
            max_retries: 2,
            initial_delay_ms: 100,
            backoff_multiplier: 1.5,
            max_delay_ms: 1000,
        },
        debug: false,
        n_seq_max: 1,
        n_threads: 4,
        n_threads_batch: 4,
    };

    // Configure MCP servers
    let mcp_servers = if let Some(mcp) = mcp_config {
        vec![llama_agent::types::MCPServerConfig::Http(
            llama_agent::types::HttpServerConfig {
                name: "swissarmyhammer".to_string(),
                url: mcp.url,
                timeout_secs: Some(llama_config.mcp_server.timeout_seconds),
                sse_keep_alive_secs: Some(30),
                stateful_mode: false,
            },
        )]
    } else {
        vec![]
    };

    // Convert MCP servers to ACP format before moving into agent_config
    let acp_mcp_servers: Vec<agent_client_protocol::McpServer> = mcp_servers
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
        .collect();

    let agent_config = llama_agent::types::AgentConfig {
        model: model_config,
        queue_config: llama_agent::types::QueueConfig {
            max_queue_size: 100,
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
            execute_prompt_inner(agent, notification_rx, system_prompt, mode, user_prompt).await
        })
    })
    .await
    .map_err(|e| AcpError::PromptError(format!("Task join error: {:?}", e)))??;

    Ok(result)
}

/// Inner implementation of execute_prompt that runs on a single-threaded runtime
async fn execute_prompt_inner(
    agent: Arc<dyn Agent + Send + Sync>,
    mut notification_rx: broadcast::Receiver<SessionNotification>,
    system_prompt: Option<String>,
    mode: Option<String>,
    user_prompt: String,
) -> AcpResult<AgentResponse> {
    // Initialize the agent if needed
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

    // Create a new session
    // Note: ACP sessions use cwd as the working directory.
    // System prompts are typically sent as part of the first prompt, not session creation.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut session_request = NewSessionRequest::new(cwd);

    // Add system prompt as metadata if provided
    if let Some(sys_prompt) = system_prompt {
        let mut meta = serde_json::Map::new();
        meta.insert("system_prompt".to_string(), serde_json::json!(sys_prompt));
        session_request = session_request.meta(meta);
    }

    let session_response = agent
        .new_session(session_request)
        .await
        .map_err(|e| AcpError::SessionError(format!("{:?}", e)))?;

    let session_id = session_response.session_id.clone();
    tracing::debug!("Session created: {}", session_id);

    // Set session mode if provided
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

    // Build prompt content
    let prompt_content = vec![ContentBlock::Text(TextContent::new(user_prompt))];

    // Collect content from notifications concurrently with the prompt
    let session_id_clone = session_id.clone();

    // Use tokio::task::spawn_local for collecting notifications on current-thread runtime
    let local_set = tokio::task::LocalSet::new();
    let collector_handle = local_set.spawn_local(async move {
        let mut text = String::new();
        loop {
            match notification_rx.recv().await {
                Ok(notification) => {
                    // Only process notifications for our session
                    if notification.session_id == session_id_clone {
                        // Extract text from AgentMessageChunk updates
                        if let SessionUpdate::AgentMessageChunk(content_chunk) =
                            &notification.update
                        {
                            if let ContentBlock::Text(text_content) = &content_chunk.content {
                                text.push_str(&text_content.text);
                            }
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    tracing::warn!("Notification receiver lagged, some text may be lost");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
        text
    });

    // Run the prompt and collection together
    let (prompt_result, response_text) = local_set
        .run_until(async {
            // Send the prompt
            let prompt_response = agent
                .prompt(PromptRequest::new(session_id.clone(), prompt_content))
                .await
                .map_err(|e| AcpError::PromptError(format!("{:?}", e)))?;

            // Give the collector a moment to finish processing remaining notifications
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Abort the collector and get collected text inside the LocalSet context
            collector_handle.abort();
            let response_text = collector_handle.await.unwrap_or_default();

            Ok::<_, AcpError>((prompt_response, response_text))
        })
        .await?;

    // Determine response type based on stop reason
    let response_type = match prompt_result.stop_reason {
        StopReason::EndTurn => AgentResponseType::Success,
        StopReason::MaxTokens | StopReason::MaxTurnRequests => AgentResponseType::Partial,
        StopReason::Refusal | StopReason::Cancelled => AgentResponseType::Error,
        _ => AgentResponseType::Partial, // Handle any future variants
    };

    // Convert metadata from Map<String, Value> to Value
    let metadata = prompt_result.meta.map(serde_json::Value::Object);

    // If response_text is empty, fall back to response from metadata
    // Try claude_response first (claude-agent), then llama_response (llama-agent)
    let content = if response_text.is_empty() {
        metadata
            .as_ref()
            .and_then(|m| m.get("claude_response").or_else(|| m.get("llama_response")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else {
        response_text
    };

    Ok(AgentResponse {
        content,
        metadata,
        response_type,
    })
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
    fn test_agent_response_error() {
        let response = AgentResponse::error("Failed".to_string());
        assert!(!response.is_success());
        assert!(response.is_error());
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
}
