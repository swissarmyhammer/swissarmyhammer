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
pub mod agent_todo_handlers;
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

pub use agent::{ClaudeAgent, RawMessageManager};
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
) -> Result<(ClaudeAgent, broadcast::Receiver<SessionNotification>)> {
    let mut agent_config = AgentConfig::default();
    agent_config.claude.ephemeral = config.ephemeral;
    agent_config.mcp_servers = config.mcp_servers;
    ClaudeAgent::new(agent_config).await
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
    mut notifications: broadcast::Receiver<SessionNotification>,
    prompt: impl Into<String>,
) -> Result<CollectedResponse> {
    let prompt_text = prompt.into();

    // Initialize the agent first (required by ACP protocol)
    let init_request = InitializeRequest::new(1.into());
    agent
        .initialize(init_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to initialize agent: {}", e)))?;

    // Create a new session
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let session_request = NewSessionRequest::new(cwd);
    let session_response = agent
        .new_session(session_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to create session: {}", e)))?;

    let session_id = session_response.session_id;

    // Build the prompt request
    let prompt_request = PromptRequest::new(
        session_id.clone(),
        vec![ContentBlock::Text(TextContent::new(prompt_text))],
    );

    // Spawn a task to collect notifications
    let collected_text = Arc::new(tokio::sync::Mutex::new(String::new()));
    let collected_text_clone = Arc::clone(&collected_text);
    let target_session_id = session_id.clone();

    let collector = tokio::spawn(async move {
        while let Ok(notification) = notifications.recv().await {
            // Check if notification is for our session
            if notification.session_id != target_session_id {
                continue;
            }

            // Extract text from AgentMessageChunk updates
            if let SessionUpdate::AgentMessageChunk(content_chunk) = &notification.update {
                if let ContentBlock::Text(text_content) = &content_chunk.content {
                    let mut guard = collected_text_clone.lock().await;
                    guard.push_str(&text_content.text);
                }
            }
        }
    });

    // Send the prompt
    let prompt_response = agent
        .prompt(prompt_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to execute prompt: {}", e)))?;

    // Give the collector a moment to finish, then abort if still running
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    collector.abort();

    // Get the collected text
    let content = collected_text.lock().await.clone();

    Ok(CollectedResponse {
        content,
        stop_reason: prompt_response.stop_reason,
    })
}
