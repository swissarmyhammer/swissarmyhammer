//! Multi-turn conversation management with language model integration
//!
//! This module implements the ACP (Agent Client Protocol) specification for multi-turn
//! conversation flow with language models. It handles the complete cycle of:
//! 1. Sending user prompts to the language model
//! 2. Processing LM responses (text and tool calls)
//! 3. Executing requested tools
//! 4. Sending tool results back to LM
//! 5. Continuing until LM completes without tool requests
//!
//! ## ACP Multi-turn Flow
//!
//! According to the ACP specification (https://agentclientprotocol.com/protocol/prompt-turn):
//! - The agent sends the initial user prompt to the language model
//! - The LM responds with text content and/or tool call requests
//! - The agent executes the requested tools and collects results
//! - The agent sends tool results back to the LM as the next request
//! - This cycle continues until the LM completes its response without requesting tools
//! - The final response includes appropriate stop reason (end_turn, max_tokens, etc.)

use crate::{
    agent::{CancellationManager, NotificationSender},
    claude::{ChunkType, ClaudeClient, SessionContext},
    error::Result,
    session::{Session, SessionId},
    tools::{InternalToolRequest, ToolCallHandler, ToolCallResult},
};
use agent_client_protocol::{
    ContentBlock, PromptResponse, SessionNotification, SessionUpdate, StopReason, TextContent,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;

/// Result of a single language model request/response interaction
#[derive(Debug, Clone)]
pub struct LmTurnResult {
    /// Text content from the language model
    pub text_content: String,
    /// Tool calls requested by the language model (if any)
    pub tool_calls: Vec<ToolCallRequest>,
    /// Estimated token usage for this turn
    pub token_usage: TokenUsage,
}

/// A tool call request extracted from language model response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// Unique identifier for this tool call
    pub id: String,
    /// Name of the tool to execute
    pub name: String,
    /// Arguments for the tool as JSON
    pub arguments: serde_json::Value,
}

/// Result of executing a single tool
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    /// ID of the tool call that was executed
    pub tool_call_id: String,
    /// Status of the tool execution
    pub status: ToolExecutionStatus,
    /// Output from the tool execution
    pub output: String,
}

/// Status of a tool execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionStatus {
    /// Tool executed successfully
    Success,
    /// Tool execution failed with error
    Error,
    /// Tool requires permission (not yet implemented in this flow)
    PermissionRequired,
}

/// Token usage tracking for LM requests
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Estimated input tokens
    pub input_tokens: u64,
    /// Estimated output tokens
    pub output_tokens: u64,
}

impl TokenUsage {
    /// Create new token usage estimate from text lengths
    pub fn estimate_from_text(input_text: &str, output_text: &str) -> Self {
        // Rough approximation: 4 characters per token
        Self {
            input_tokens: (input_text.len() as u64) / 4,
            output_tokens: (output_text.len() as u64) / 4,
        }
    }

    /// Get total tokens used
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Message for language model conversation history
#[derive(Debug, Clone)]
pub enum LmMessage {
    /// User message
    User { content: String },
    /// Assistant message
    Assistant { content: String },
    /// Tool call request from assistant
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    /// Tool result to send back to LM
    ToolResult {
        tool_call_id: String,
        output: String,
    },
}

/// Manages multi-turn conversations with the language model
///
/// The conversation manager implements the complete ACP multi-turn flow:
/// - Maintains conversation history across LM requests
/// - Extracts and executes tool calls from LM responses
/// - Formats tool results for LM consumption
/// - Handles streaming and non-streaming modes
/// - Enforces turn limits and token limits
/// - Supports cancellation at any point
pub struct ConversationManager {
    claude_client: Arc<ClaudeClient>,
    tool_handler: Arc<RwLock<ToolCallHandler>>,
    notification_sender: Arc<NotificationSender>,
    cancellation_manager: Arc<CancellationManager>,
}

impl ConversationManager {
    /// Create a new conversation manager
    pub fn new(
        claude_client: Arc<ClaudeClient>,
        tool_handler: Arc<RwLock<ToolCallHandler>>,
        notification_sender: Arc<NotificationSender>,
        cancellation_manager: Arc<CancellationManager>,
    ) -> Self {
        Self {
            claude_client,
            tool_handler,
            notification_sender,
            cancellation_manager,
        }
    }

    /// Process a complete prompt turn with multi-turn LM interaction
    ///
    /// This implements the ACP multi-turn conversation flow:
    /// 1. Send user prompt to language model
    /// 2. Process LM response for text and tool calls
    /// 3. If tool calls requested, execute them
    /// 4. Send tool results back to LM
    /// 5. Repeat steps 2-4 until LM completes without tool requests
    /// 6. Return final response with appropriate stop reason
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID for this conversation
    /// * `prompt_text` - The user's prompt text
    /// * `session` - The current session with conversation history
    /// * `streaming` - Whether to use streaming mode
    /// * `max_turn_requests` - Maximum number of LM requests allowed
    /// * `max_tokens_per_turn` - Maximum tokens allowed per turn
    ///
    /// # Returns
    ///
    /// Returns a `PromptResponse` with the appropriate stop reason:
    /// - `end_turn`: LM completed without tool requests
    /// - `max_tokens`: Token limit exceeded
    /// - `max_turn_requests`: Too many LM requests
    /// - `cancelled`: Session was cancelled
    /// - `error`: An error occurred during processing
    pub async fn process_turn(
        &self,
        session_id: &SessionId,
        prompt_text: &str,
        session: &Session,
        streaming: bool,
        max_turn_requests: u64,
        max_tokens_per_turn: u64,
    ) -> Result<PromptResponse> {
        tracing::info!(
            "Starting multi-turn conversation for session: {}",
            session_id
        );

        // ACP requires complete multi-turn conversation flow:
        // 1. Send user prompt to language model
        // 2. Process LM response for text and tool calls
        // 3. Execute tools and collect results
        // 4. Send tool results back to LM for continuation
        // 5. Repeat until LM completes without tool requests
        // 6. Return final response with appropriate stop reason
        //
        // Each turn may involve multiple LM requests with tool results.

        let mut conversation_history: Vec<LmMessage> = Vec::new();
        let mut turn_request_count = 0u64;
        let mut turn_token_count = 0u64;

        // Add initial user prompt to conversation history
        conversation_history.push(LmMessage::User {
            content: prompt_text.to_string(),
        });

        // Multi-turn loop: continue until LM completes or limits reached
        loop {
            // Check cancellation before each LM request
            if self
                .cancellation_manager
                .is_cancelled(&session_id.to_string())
                .await
            {
                tracing::info!(
                    "Session {} cancelled during multi-turn conversation",
                    session_id
                );
                return Ok(PromptResponse {
                    stop_reason: StopReason::Cancelled,
                    meta: Some(serde_json::json!({
                        "turn_requests": turn_request_count,
                        "turn_tokens": turn_token_count,
                    })),
                });
            }

            // Check turn request limit
            turn_request_count += 1;
            if turn_request_count > max_turn_requests {
                tracing::info!(
                    "Turn request limit exceeded ({} > {}) for session: {}",
                    turn_request_count,
                    max_turn_requests,
                    session_id
                );
                return Ok(PromptResponse {
                    stop_reason: StopReason::MaxTurnRequests,
                    meta: Some(serde_json::json!({
                        "turn_requests": turn_request_count,
                        "max_turn_requests": max_turn_requests,
                    })),
                });
            }

            // Send request to language model
            let lm_result = if streaming {
                self.send_to_lm_streaming(session_id, &conversation_history, session)
                    .await?
            } else {
                self.send_to_lm_non_streaming(&conversation_history, session)
                    .await?
            };

            // Update token count
            turn_token_count += lm_result.token_usage.total();
            if turn_token_count > max_tokens_per_turn {
                tracing::info!(
                    "Token limit exceeded ({} > {}) for session: {}",
                    turn_token_count,
                    max_tokens_per_turn,
                    session_id
                );
                return Ok(PromptResponse {
                    stop_reason: StopReason::MaxTokens,
                    meta: Some(serde_json::json!({
                        "turn_tokens": turn_token_count,
                        "max_tokens_per_turn": max_tokens_per_turn,
                    })),
                });
            }

            // Add assistant response to conversation history
            if !lm_result.text_content.is_empty() {
                conversation_history.push(LmMessage::Assistant {
                    content: lm_result.text_content.clone(),
                });
            }

            // Check if turn is complete (no tool calls)
            if lm_result.tool_calls.is_empty() {
                tracing::info!(
                    "Multi-turn conversation completed after {} LM requests",
                    turn_request_count
                );
                return Ok(PromptResponse {
                    stop_reason: StopReason::EndTurn,
                    meta: Some(serde_json::json!({
                        "turn_requests": turn_request_count,
                        "turn_tokens": turn_token_count,
                    })),
                });
            }

            // Execute tool calls
            tracing::info!(
                "Executing {} tool calls for session: {}",
                lm_result.tool_calls.len(),
                session_id
            );

            let tool_results = self
                .execute_tools(session_id, lm_result.tool_calls.clone())
                .await?;

            // Add tool calls and results to conversation history in chronological order
            // Each tool call should be immediately followed by its result to maintain
            // the actual execution order
            for tool_call in &lm_result.tool_calls {
                // Add the tool call
                conversation_history.push(LmMessage::ToolCall {
                    id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                });

                // Find and add the corresponding tool result
                if let Some(result) = tool_results.iter().find(|r| r.tool_call_id == tool_call.id) {
                    conversation_history.push(LmMessage::ToolResult {
                        tool_call_id: result.tool_call_id.clone(),
                        output: result.output.clone(),
                    });
                } else {
                    tracing::warn!(
                        "No result found for tool call: {} ({})",
                        tool_call.name,
                        tool_call.id
                    );
                }
            }

            // Continue loop to send tool results back to LM
        }
    }

    /// Send a request to the language model in streaming mode
    ///
    /// This method streams the response from the language model, extracting both
    /// text content and tool call requests. It sends streaming updates to the
    /// notification system as chunks arrive.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID for notification routing
    /// * `messages` - The conversation history to send to the LM
    /// * `session` - The current session with context
    ///
    /// # Returns
    ///
    /// Returns `LmTurnResult` containing:
    /// - `text_content`: Accumulated text from the LM
    /// - `tool_calls`: Extracted tool call requests (if any)
    /// - `token_usage`: Estimated token usage for this turn
    async fn send_to_lm_streaming(
        &self,
        session_id: &SessionId,
        messages: &[LmMessage],
        session: &Session,
    ) -> Result<LmTurnResult> {
        // Build prompt text from conversation history
        let prompt_text = self.build_prompt_from_messages(messages);

        // Create session context
        let context: SessionContext = session.into();

        // Get streaming response
        let mut stream = self
            .claude_client
            .query_stream_with_context(&prompt_text, &context)
            .await?;

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();
        let mut tool_call_counter = 0u32;
        let mut actual_token_usage: Option<TokenUsage> = None;

        // Process streaming chunks
        while let Some(chunk) = stream.next().await {
            // Check for actual token usage in Result messages
            if let Some(usage_info) = chunk.token_usage {
                actual_token_usage = Some(TokenUsage {
                    input_tokens: usage_info.input_tokens,
                    output_tokens: usage_info.output_tokens,
                });
            }

            match chunk.chunk_type {
                ChunkType::Text => {
                    text_content.push_str(&chunk.content);

                    // Send streaming update
                    if let Err(e) = self
                        .notification_sender
                        .send_update(SessionNotification {
                            session_id: agent_client_protocol::SessionId::new(
                                session_id.to_string().into(),
                            ),
                            update: SessionUpdate::AgentMessageChunk(
                                agent_client_protocol::ContentChunk {
                                    content: ContentBlock::Text(TextContent {
                                        text: chunk.content.clone(),
                                        annotations: None,
                                        meta: None,
                                    }),
                                    meta: None,
                                },
                            ),
                            meta: None,
                        })
                        .await
                    {
                        tracing::warn!("Failed to send streaming update: {}", e);
                    }
                }
                ChunkType::ToolCall => {
                    // Extract tool call from chunk
                    if let Some(tool_call_info) = chunk.tool_call {
                        tool_call_counter += 1;
                        let tool_call_id = format!("tool_call_{}", tool_call_counter);

                        tracing::debug!(
                            "Extracted tool call: {} ({})",
                            tool_call_info.name,
                            tool_call_id
                        );

                        tool_calls.push(ToolCallRequest {
                            id: tool_call_id,
                            name: tool_call_info.name,
                            arguments: tool_call_info.parameters,
                        });
                    } else {
                        tracing::warn!("Tool call chunk without tool call info");
                    }
                }
                ChunkType::ToolResult => {
                    // Tool results are inputs, not outputs from LM
                    tracing::debug!("Tool result chunk (unexpected in LM output)");
                }
            }
        }

        // Use actual token usage if available, otherwise estimate
        let token_usage = actual_token_usage
            .unwrap_or_else(|| TokenUsage::estimate_from_text(&prompt_text, &text_content));

        Ok(LmTurnResult {
            text_content,
            tool_calls,
            token_usage,
        })
    }

    /// Send a request to the language model in non-streaming mode
    ///
    /// Note: Non-streaming mode cannot extract tool calls because the
    /// Claude Code CLI only returns text in non-streaming mode.
    /// For multi-turn conversations with tool calls, use streaming mode.
    async fn send_to_lm_non_streaming(
        &self,
        messages: &[LmMessage],
        session: &Session,
    ) -> Result<LmTurnResult> {
        // Build prompt text from conversation history
        let prompt_text = self.build_prompt_from_messages(messages);

        // Create session context
        let context: SessionContext = session.into();

        // Get non-streaming response
        let response = self
            .claude_client
            .query_with_context(&prompt_text, &context)
            .await?;

        // Non-streaming mode cannot extract tool calls from text response
        // The Claude Code CLI does not expose tool calls in text mode
        // For tool call support, use streaming mode
        let tool_calls = Vec::new();

        tracing::warn!(
            "Non-streaming mode does not support tool call extraction. \
             Use streaming mode for multi-turn conversations with tools."
        );

        // Estimate token usage
        let token_usage = TokenUsage::estimate_from_text(&prompt_text, &response);

        Ok(LmTurnResult {
            text_content: response,
            tool_calls,
            token_usage,
        })
    }

    /// Build a prompt text from conversation messages
    ///
    /// Converts the conversation history into a text format for sending to the
    /// Claude Code CLI. Each message type is formatted differently:
    /// - User messages: "User: {content}"
    /// - Assistant messages: "Assistant: {content}"
    /// - Tool calls: "Tool Call [{id}]: {name} with arguments: {args}"
    /// - Tool results: "Tool Result [{id}]: {output}"
    ///
    /// # Arguments
    ///
    /// * `messages` - The conversation history to format
    ///
    /// # Returns
    ///
    /// A formatted string containing the conversation history
    ///
    /// # Note
    ///
    /// This returns a text string because the Claude Code CLI accepts text prompts.
    /// The structured `LmMessage` types are converted to text format for the CLI.
    fn build_prompt_from_messages(&self, messages: &[LmMessage]) -> String {
        let mut prompt = String::new();

        for message in messages {
            match message {
                LmMessage::User { content } => {
                    prompt.push_str("User: ");
                    prompt.push_str(content);
                    prompt.push('\n');
                }
                LmMessage::Assistant { content } => {
                    prompt.push_str("Assistant: ");
                    prompt.push_str(content);
                    prompt.push('\n');
                }
                LmMessage::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    prompt.push_str(&format!(
                        "Tool Call [{}]: {} with arguments: {}\n",
                        id, name, arguments
                    ));
                }
                LmMessage::ToolResult {
                    tool_call_id,
                    output,
                } => {
                    prompt.push_str(&format!("Tool Result [{}]: {}\n", tool_call_id, output));
                }
            }
        }

        prompt
    }

    /// Execute tool calls and collect results
    ///
    /// Executes all requested tool calls sequentially and collects their results.
    /// Each tool call is converted to the internal tool request format and
    /// executed via the ToolCallHandler.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID for tool execution context
    /// * `tool_calls` - The list of tool calls to execute
    ///
    /// # Returns
    ///
    /// Returns a vector of `ToolExecutionResult` containing:
    /// - `tool_call_id`: ID matching the original tool call
    /// - `status`: Success, Error, or PermissionRequired
    /// - `output`: Result text or error message
    ///
    /// # Note
    ///
    /// Currently executes tools sequentially. Future enhancement: concurrent execution.
    async fn execute_tools(
        &self,
        session_id: &SessionId,
        tool_calls: Vec<ToolCallRequest>,
    ) -> Result<Vec<ToolExecutionResult>> {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            // Convert to internal tool request format
            let internal_request = InternalToolRequest {
                id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            };

            // Execute tool via tool handler
            let result = {
                let tool_handler = self.tool_handler.write().await;
                tool_handler
                    .handle_tool_request(
                        &agent_client_protocol::SessionId::new(session_id.to_string().into()),
                        internal_request,
                    )
                    .await
            };

            // Convert result to execution result
            let execution_result = match result {
                Ok(ToolCallResult::Success(output)) => ToolExecutionResult {
                    tool_call_id: tool_call.id.clone(),
                    status: ToolExecutionStatus::Success,
                    output,
                },
                Ok(ToolCallResult::Error(error)) => ToolExecutionResult {
                    tool_call_id: tool_call.id.clone(),
                    status: ToolExecutionStatus::Error,
                    output: error,
                },
                Ok(ToolCallResult::PermissionRequired(permission_request)) => {
                    // Permission handling in multi-turn conversations:
                    // The ToolCallHandler has determined this tool requires user permission.
                    // In the context of multi-turn LM conversations, we convert this to
                    // an informative error that the LM can understand and respond to.
                    // This maintains conversation flow while respecting security boundaries.
                    //
                    // The permission request contains:
                    // - tool_name: What tool was requested
                    // - description: Human-readable explanation
                    // - options: Available permission choices
                    //
                    // We format this information so the LM understands the tool cannot
                    // execute without explicit user permission.
                    let permission_message = format!(
                        "Permission required: {} - {}. Available options: {}",
                        permission_request.tool_name,
                        permission_request.description,
                        permission_request
                            .options
                            .iter()
                            .map(|opt| format!("{} ({})", opt.name, opt.option_id))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );

                    tracing::info!(
                        tool_call_id = %tool_call.id,
                        tool_name = %permission_request.tool_name,
                        "Tool execution blocked pending permission"
                    );

                    ToolExecutionResult {
                        tool_call_id: tool_call.id.clone(),
                        status: ToolExecutionStatus::PermissionRequired,
                        output: permission_message,
                    }
                }
                Err(e) => ToolExecutionResult {
                    tool_call_id: tool_call.id.clone(),
                    status: ToolExecutionStatus::Error,
                    output: format!("Tool execution error: {}", e),
                },
            };

            results.push(execution_result);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{PermissionOption, PermissionOptionKind};

    #[test]
    fn test_token_usage_estimation() {
        let usage = TokenUsage::estimate_from_text("Hello world", "Hi there");
        assert_eq!(usage.input_tokens, 2); // "Hello world".len() / 4 = 11 / 4 = 2
        assert_eq!(usage.output_tokens, 2); // "Hi there".len() / 4 = 8 / 4 = 2
        assert_eq!(usage.total(), 4);
    }

    #[test]
    fn test_tool_execution_status() {
        assert_eq!(ToolExecutionStatus::Success, ToolExecutionStatus::Success);
        assert_ne!(ToolExecutionStatus::Success, ToolExecutionStatus::Error);
    }

    #[test]
    fn test_tool_execution_status_permission_required() {
        // Verify PermissionRequired is a distinct status
        assert_ne!(
            ToolExecutionStatus::PermissionRequired,
            ToolExecutionStatus::Success
        );
        assert_ne!(
            ToolExecutionStatus::PermissionRequired,
            ToolExecutionStatus::Error
        );
        assert_eq!(
            ToolExecutionStatus::PermissionRequired,
            ToolExecutionStatus::PermissionRequired
        );
    }

    #[test]
    fn test_permission_request_formatting() {
        // Test that permission request details are properly formatted
        let permission_request = crate::tools::PermissionRequest {
            tool_request_id: "test_tool_call_123".to_string(),
            tool_name: "fs_write".to_string(),
            description: "Write to file /etc/hosts".to_string(),
            arguments: serde_json::json!({"path": "/etc/hosts", "content": "test"}),
            options: vec![
                PermissionOption {
                    option_id: "allow_once".to_string(),
                    name: "Allow Once".to_string(),
                    kind: PermissionOptionKind::AllowOnce,
                },
                PermissionOption {
                    option_id: "allow_always".to_string(),
                    name: "Allow Always".to_string(),
                    kind: PermissionOptionKind::AllowAlways,
                },
                PermissionOption {
                    option_id: "reject_once".to_string(),
                    name: "Reject Once".to_string(),
                    kind: PermissionOptionKind::RejectOnce,
                },
            ],
        };

        // Format the permission message as done in execute_tools
        let permission_message = format!(
            "Permission required: {} - {}. Available options: {}",
            permission_request.tool_name,
            permission_request.description,
            permission_request
                .options
                .iter()
                .map(|opt| format!("{} ({})", opt.name, opt.option_id))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Verify the message contains all critical information
        assert!(permission_message.contains("Permission required"));
        assert!(permission_message.contains("fs_write"));
        assert!(permission_message.contains("/etc/hosts"));
        assert!(permission_message.contains("Allow Once"));
        assert!(permission_message.contains("allow_once"));
        assert!(permission_message.contains("Allow Always"));
        assert!(permission_message.contains("allow_always"));
        assert!(permission_message.contains("Reject Once"));
        assert!(permission_message.contains("reject_once"));
    }

    #[test]
    fn test_tool_call_request_structure() {
        // Verify ToolCallRequest has the expected structure for tool execution
        let tool_call = ToolCallRequest {
            id: "call_123".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        };

        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.name, "fs_read");
        assert_eq!(
            tool_call.arguments,
            serde_json::json!({"path": "/tmp/test.txt"})
        );
    }

    #[test]
    fn test_tool_execution_result_permission_required() {
        // Test that ToolExecutionResult can properly represent permission required status
        let result = ToolExecutionResult {
            tool_call_id: "call_456".to_string(),
            status: ToolExecutionStatus::PermissionRequired,
            output: "Permission required: fs_write - Write to protected file".to_string(),
        };

        assert_eq!(result.tool_call_id, "call_456");
        assert_eq!(result.status, ToolExecutionStatus::PermissionRequired);
        assert!(result.output.contains("Permission required"));
        assert!(result.output.contains("fs_write"));
    }

    #[test]
    fn test_lm_message_types() {
        // Verify all message types can be created and matched
        let user_msg = LmMessage::User {
            content: "Hello".to_string(),
        };
        let assistant_msg = LmMessage::Assistant {
            content: "Hi there".to_string(),
        };
        let tool_call_msg = LmMessage::ToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        let tool_result_msg = LmMessage::ToolResult {
            tool_call_id: "call_1".to_string(),
            output: "Success".to_string(),
        };

        // Verify pattern matching works
        match user_msg {
            LmMessage::User { content } => assert_eq!(content, "Hello"),
            _ => panic!("Wrong message type"),
        }
        match assistant_msg {
            LmMessage::Assistant { content } => assert_eq!(content, "Hi there"),
            _ => panic!("Wrong message type"),
        }
        match tool_call_msg {
            LmMessage::ToolCall { id, name, .. } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "test_tool");
            }
            _ => panic!("Wrong message type"),
        }
        match tool_result_msg {
            LmMessage::ToolResult {
                tool_call_id,
                output,
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(output, "Success");
            }
            _ => panic!("Wrong message type"),
        }
    }
}
