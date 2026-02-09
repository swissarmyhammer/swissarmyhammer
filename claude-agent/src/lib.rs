//! Claude Agent Library
//!
//! A Rust library that implements an Agent Client Protocol (ACP) server,
//! wrapping Claude Code functionality to enable any ACP-compatible client
//! to interact with Claude Code.

pub mod acp_error_conversion;
pub mod agent;
pub mod agent_cancellation;
pub mod agent_commands;
pub mod agent_file_handlers;
pub mod agent_file_operations;
pub mod agent_notifications;
pub mod agent_permissions;
pub mod agent_prompt_handling;
pub mod agent_raw_messages;
pub mod agent_reasoning;
pub mod agent_terminal_handlers;
pub mod agent_trait_impl;
pub mod agent_validation;
pub mod base64_processor;
pub mod base64_validation;
pub mod capability_validation;
pub mod claude;
pub mod claude_backend;
pub mod claude_process;
pub mod config;
pub mod constants;
pub mod content_block_processor;
pub mod content_capability_validator;
pub mod content_security_validator;
pub mod conversation_manager;
pub mod editor_state;
pub mod json_rpc_codes;
pub mod mime_type_validator;

#[cfg(test)]
mod content_security_integration_tests;
pub mod error;
pub mod mcp;
pub mod mcp_error_handling;
pub mod path_validator;
pub mod permission_storage;
pub mod permissions;
pub mod plan;
pub mod protocol_translator;
#[cfg(test)]
// mod permission_interaction_tests; // Disabled: tests MockPromptHandler which was deleted
pub mod request_validation;
pub mod server;
pub mod session;
pub mod session_errors;
pub mod session_loading;
pub mod session_validation;
pub mod size_validator;
pub mod terminal_manager;
mod tool_call_lifecycle_tests;
pub mod tool_classification;
pub mod tool_types;
pub mod tools;
pub mod url_validation;

// Re-exports for convenient access to main types
pub use agent::{ClaudeAgent, RawMessageManager};
pub use agent_notifications::NotificationSender;
pub use claude_process::SpawnConfig;
pub use config::{AgentConfig, McpServerConfig};
pub use error::{AgentError, Result};
pub use plan::{
    todowrite_to_acp_plan, todowrite_to_agent_plan, AgentPlan, PlanEntry, PlanEntryStatus, Priority,
};
pub use server::ClaudeAgentServer;
pub use tools::{ToolCallHandler, ToolCallResult, ToolPermissions};

use agent_client_protocol::{
    Agent, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, SessionNotification,
    SessionUpdate, StopReason, TextContent,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use typed_builder::TypedBuilder;

/// Delay in milliseconds to allow notification collection to complete.
///
/// Notifications may flow through multiple forwarding hops, so we need
/// more time than just the prompt execution itself.
const NOTIFICATION_COLLECTION_DELAY_MS: u64 = 500;

/// Collected response from executing a prompt via streaming.
///
/// This collects the streamed content from SessionNotifications into a single response.
/// Note: This is different from `agent_client_protocol::CollectedResponse` which only
/// contains the stop_reason - the actual content arrives via streaming notifications.
#[derive(Debug, Clone)]
pub struct CollectedResponse {
    /// The collected text content from streaming notifications
    pub content: String,
    /// Why the agent stopped
    pub stop_reason: StopReason,
}

/// Configuration for creating a ClaudeAgent.
///
/// Uses builder pattern to allow flexible configuration without
/// breaking changes when new options are added.
#[derive(Debug, Clone, TypedBuilder)]
pub struct CreateAgentConfig {
    /// Use ephemeral mode (haiku model + no session persistence)
    /// Ideal for validators and quick, stateless operations
    #[builder(default)]
    pub ephemeral: bool,
    /// MCP servers to configure for the agent
    #[builder(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Create a ClaudeAgent with the given configuration.
///
/// This is a convenience function that wraps ClaudeAgent::new() with
/// a simpler configuration interface.
///
/// # Example
///
/// ```ignore
/// use claude_agent::{CreateAgentConfig, create_agent};
///
/// // Create an ephemeral agent for quick operations
/// let config = CreateAgentConfig::builder()
///     .ephemeral(true)
///     .build();
/// let (agent, notifications) = create_agent(config).await?;
/// ```
pub async fn create_agent(
    config: CreateAgentConfig,
) -> Result<(
    ClaudeAgent,
    Arc<crate::agent_notifications::NotificationSender>,
)> {
    let mut agent_config = AgentConfig::default();
    agent_config.claude.ephemeral = config.ephemeral;
    agent_config.mcp_servers = config.mcp_servers;
    let (agent, _receiver) = ClaudeAgent::new(agent_config).await?;
    let notifier = Arc::clone(&agent.notification_sender);
    Ok((agent, notifier))
}

/// Execute a prompt and collect the response content.
///
/// This function handles the ACP streaming protocol:
/// 1. Creates a new session
/// 2. Subscribes to notifications
/// 3. Sends the prompt
/// 4. Collects text from SessionNotifications
/// 5. Returns the complete response
///
/// # Example
///
/// ```ignore
/// use claude_agent::{CreateAgentConfig, create_agent, execute_prompt};
///
/// let config = CreateAgentConfig::builder().ephemeral(true).build();
/// let (agent, notifications) = create_agent(config).await?;
/// let response = execute_prompt(&agent, notifications, "Hello!").await?;
/// println!("{}", response.content);
/// ```
pub async fn execute_prompt(
    agent: &ClaudeAgent,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: impl Into<String>,
) -> Result<CollectedResponse> {
    execute_prompt_with_agent(agent, notifications, prompt).await
}

/// Execute a prompt with any Agent implementation.
///
/// This is a lower-level function that works with any type implementing the Agent trait,
/// making it suitable for use with PlaybackAgent in tests.
pub async fn execute_prompt_with_agent<A: Agent + ?Sized>(
    agent: &A,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: impl Into<String>,
) -> Result<CollectedResponse> {
    let prompt_text = prompt.into();

    initialize_agent(agent).await?;
    let session_id = create_session(agent).await?;
    let prompt_request = build_prompt_request(&session_id, prompt_text);

    let (collector, collected_text, notification_count, _matched_count) =
        spawn_notification_collector(notifications, session_id);

    let prompt_response = agent
        .prompt(prompt_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to execute prompt: {}", e)))?;

    let content = collect_response_content(
        collector,
        collected_text,
        notification_count,
        &prompt_response,
    )
    .await;

    Ok(CollectedResponse {
        content,
        stop_reason: prompt_response.stop_reason,
    })
}

/// Initialize the agent (required by ACP protocol).
async fn initialize_agent<A: Agent + ?Sized>(agent: &A) -> Result<()> {
    let init_request = InitializeRequest::new(1.into());
    agent
        .initialize(init_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to initialize agent: {}", e)))?;
    Ok(())
}

/// Create a new session with the agent.
async fn create_session<A: Agent + ?Sized>(agent: &A) -> Result<agent_client_protocol::SessionId> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let session_request = NewSessionRequest::new(cwd);
    let session_response = agent
        .new_session(session_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to create session: {}", e)))?;
    Ok(session_response.session_id)
}

/// Build a prompt request for the given session and text.
fn build_prompt_request(
    session_id: &agent_client_protocol::SessionId,
    prompt_text: String,
) -> PromptRequest {
    PromptRequest::new(
        session_id.clone(),
        vec![ContentBlock::Text(TextContent::new(prompt_text))],
    )
}

/// Extract text content from a notification if it matches our session.
async fn process_notification(
    notification: &SessionNotification,
    session_id: &agent_client_protocol::SessionId,
    collected_text: &tokio::sync::Mutex<String>,
    matched_count: &std::sync::atomic::AtomicUsize,
) {
    if notification.session_id != *session_id {
        return;
    }

    matched_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    if let SessionUpdate::AgentMessageChunk(chunk) = &notification.update {
        if let ContentBlock::Text(text) = &chunk.content {
            let mut guard = collected_text.lock().await;
            guard.push_str(&text.text);
            tracing::trace!(
                session = %session_id,
                chunk_len = text.text.len(),
                total_len = guard.len(),
                "Collected text chunk"
            );
        }
    }
}

/// Spawn a task to collect text from session notifications.
pub fn spawn_notification_collector(
    mut notifications: broadcast::Receiver<SessionNotification>,
    session_id: agent_client_protocol::SessionId,
) -> (
    tokio::task::JoinHandle<()>,
    Arc<tokio::sync::Mutex<String>>,
    Arc<std::sync::atomic::AtomicUsize>,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let collected_text = Arc::new(tokio::sync::Mutex::new(String::new()));
    let collected_text_clone = Arc::clone(&collected_text);
    let notification_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let notification_count_clone = Arc::clone(&notification_count);
    let matched_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let matched_count_clone = Arc::clone(&matched_count);

    tracing::debug!(session = %session_id, "Starting notification collector");

    let collector = tokio::spawn(async move {
        loop {
            match notifications.recv().await {
                Ok(notification) => {
                    notification_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    process_notification(
                        &notification,
                        &session_id,
                        &collected_text_clone,
                        &matched_count_clone,
                    )
                    .await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "Notification collector lagged");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    (collector, collected_text, notification_count, matched_count)
}

/// Collect the response content after prompt execution.
pub async fn collect_response_content(
    collector: tokio::task::JoinHandle<()>,
    collected_text: Arc<tokio::sync::Mutex<String>>,
    notification_count: Arc<std::sync::atomic::AtomicUsize>,
    prompt_response: &agent_client_protocol::PromptResponse,
) -> String {
    tokio::time::sleep(std::time::Duration::from_millis(
        NOTIFICATION_COLLECTION_DELAY_MS,
    ))
    .await;
    collector.abort();

    let content = collected_text.lock().await.clone();
    let total_notifications = notification_count.load(std::sync::atomic::Ordering::Relaxed);

    if content.is_empty() {
        tracing::error!(
            stop_reason = ?prompt_response.stop_reason,
            total_notifications = total_notifications,
            content_length = content.len(),
            "execute_prompt_with_agent received empty content"
        );
    } else {
        tracing::debug!(
            stop_reason = ?prompt_response.stop_reason,
            total_notifications = total_notifications,
            content_length = content.len(),
            "execute_prompt_with_agent collected content"
        );
    }

    content
}
