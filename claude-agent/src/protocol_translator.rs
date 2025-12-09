//! Protocol translator between ACP and Claude CLI stream-json format
//!
//! This module provides translation between the Agent Client Protocol (ACP) message format
//! and the stream-json format used by the claude CLI for stdin/stdout communication.
//!
//! # Stream-JSON Format
//!
//! ## Input (stdin to claude)
//! ```json
//! {"type":"user","message":{"role":"user","content":"What is 2+2?"}}
//! ```
//!
//! ## Output (stdout from claude)
//! ```json
//! {"type":"system","subtype":"init","cwd":"/path","session_id":"uuid","tools":[...]}
//! {"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{...}}]}}
//! {"type":"result","subtype":"success","total_cost_usd":0.114}
//! ```

use crate::{AgentError, Result};
use agent_client_protocol::{
    ContentBlock, SessionId, SessionNotification, SessionUpdate, TextContent,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Result information from stream-json result messages
#[derive(Debug, Clone)]
pub struct StreamResult {
    pub stop_reason: Option<String>,
}

/// Protocol translator for converting between ACP and stream-json formats
pub struct ProtocolTranslator;

impl ProtocolTranslator {
    /// Convert ACP ContentBlocks to stream-json for claude stdin
    ///
    /// Currently only supports single text content blocks. The claude CLI's stream-json
    /// format accepts a simple string for user messages, which limits us to text-only content.
    /// Complex content arrays (images, audio, etc.) would require the full Messages API format
    /// which is not supported by the CLI's stream-json stdin interface.
    ///
    /// # Arguments
    /// * `content` - The content blocks to translate
    ///
    /// # Returns
    /// A JSON string formatted for stream-json input
    ///
    /// # Errors
    /// Returns error if content is not a single text block, or if serialization fails
    pub fn acp_to_stream_json(content: Vec<ContentBlock>) -> Result<String> {
        let content_str = if content.len() == 1 {
            if let ContentBlock::Text(text_content) = &content[0] {
                text_content.text.clone()
            } else {
                return Err(AgentError::Internal(
                    "Only text content blocks are currently supported".to_string(),
                ));
            }
        } else {
            return Err(AgentError::Internal(
                "Only single content blocks are currently supported".to_string(),
            ));
        };

        let message = StreamJsonUserMessage {
            r#type: "user".to_string(),
            message: UserMessage {
                role: "user".to_string(),
                content: content_str,
            },
        };

        serde_json::to_string(&message).map_err(|e| {
            AgentError::Internal(format!("Failed to serialize stream-json message: {}", e))
        })
    }

    /// Convert stream-json line from claude to ACP SessionNotification
    ///
    /// Converts a single line of stream-json output from the claude CLI into an ACP notification.
    /// Note: The claude CLI can output messages with multiple content blocks (e.g., text + tool_use),
    /// but ACP SessionUpdate::AgentMessageChunk only supports a single ContentBlock per notification.
    /// When multiple content items are present, only the first is returned, with a debug log for the rest.
    ///
    /// # Arguments
    /// * `line` - A single line of JSON from claude stdout
    /// * `session_id` - The session ID for the notification
    ///
    /// # Returns
    /// * `Ok(Some(notification))` - Successfully parsed into an ACP notification
    /// * `Ok(None)` - Valid message but no notification needed (e.g., metadata only)
    /// * `Err(...)` - Parse error or invalid message structure
    pub fn stream_json_to_acp(
        line: &str,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        // Parse the JSON line
        let parsed: JsonValue = serde_json::from_str(line).map_err(|e| {
            let truncated_line: String = line.chars().take(100).collect();
            AgentError::Internal(format!(
                "Malformed JSON: {}. Line: {}...",
                e, truncated_line
            ))
        })?;

        // Check the message type
        let msg_type = parsed.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            AgentError::Internal("Missing 'type' field in stream-json".to_string())
        })?;

        match msg_type {
            "assistant" => {
                // Parse assistant message to check if it contains tool_use
                let assistant_msg: StreamJsonAssistantMessage =
                    serde_json::from_value(parsed.clone()).map_err(|e| {
                        AgentError::Internal(format!("Failed to parse assistant message: {}", e))
                    })?;

                // Validate message type
                assistant_msg.validate()?;

                // Check first content item
                if let Some(first_item) = assistant_msg.message.content.first() {
                    match first_item {
                        ContentItem::ToolUse { id, name, input } => {
                            // Found tool_use - emit ToolCall event
                            tracing::debug!("🔧 ASSISTANT tool_use: {} ({})", name, id);

                            use agent_client_protocol::{ToolCall, ToolCallId, ToolCallStatus};
                            use std::sync::Arc;

                            let tool_call = ToolCall {
                                id: ToolCallId(Arc::from(id.as_str())),
                                title: name.clone(),
                                kind: Self::infer_tool_kind(name),
                                status: ToolCallStatus::Pending,
                                content: vec![],
                                locations: vec![],
                                raw_input: Some(input.clone()),
                                raw_output: None,
                                meta: None,
                            };

                            return Ok(Some(SessionNotification {
                                session_id: session_id.clone(),
                                update: SessionUpdate::ToolCall(tool_call),
                                meta: None,
                            }));
                        }
                        ContentItem::Text { text } => {
                            // Text content - emit as AgentMessageChunk
                            // Note: This may duplicate stream_event chunks when --include-partial-messages is used
                            // Higher-level code (in claude.rs) must filter duplicates based on streaming state
                            tracing::debug!("📨 ASSISTANT text: {} chars", text.len());
                            return Ok(Some(SessionNotification {
                                session_id: session_id.clone(),
                                update: SessionUpdate::AgentMessageChunk(
                                    agent_client_protocol::ContentChunk {
                                        content: ContentBlock::Text(TextContent {
                                            text: text.clone(),
                                            annotations: None,
                                            meta: None,
                                        }),
                                        meta: None,
                                    },
                                ),
                                meta: None,
                            }));
                        }
                    }
                }

                // No content or empty content
                Ok(None)
            }
            "user" => {
                tracing::debug!("📥 USER message received, checking for tool_result");

                // Check if this is a tool_result message (Claude reporting tool completion)
                if let Some(message) = parsed.get("message") {
                    tracing::debug!("  Has message field");
                    if let Some(content_array) = message.get("content").and_then(|c| c.as_array()) {
                        tracing::debug!("  Has content array with {} items", content_array.len());
                        for content_item in content_array {
                            tracing::debug!(
                                "    Content item type: {:?}",
                                content_item.get("type")
                            );
                            if content_item.get("type").and_then(|t| t.as_str())
                                == Some("tool_result")
                            {
                                // This is a tool completion!
                                tracing::info!("🎯 TOOL_RESULT detected!");
                                if let Some(tool_use_id) =
                                    content_item.get("tool_use_id").and_then(|id| id.as_str())
                                {
                                    tracing::info!("🎯 TOOL_RESULT for tool_id: {}", tool_use_id);

                                    // Extract content from tool_result
                                    let tool_content = if let Some(content_str) =
                                        content_item.get("content").and_then(|c| c.as_str())
                                    {
                                        Some(vec![
                                            agent_client_protocol::ToolCallContent::Content {
                                                content: ContentBlock::Text(TextContent {
                                                    text: content_str.to_string(),
                                                    annotations: None,
                                                    meta: None,
                                                }),
                                            },
                                        ])
                                    } else {
                                        None
                                    };

                                    // Emit ToolCallUpdate(Completed) with content
                                    use agent_client_protocol::{
                                        ToolCallId, ToolCallStatus, ToolCallUpdate,
                                        ToolCallUpdateFields,
                                    };
                                    use std::sync::Arc;

                                    return Ok(Some(SessionNotification {
                                        session_id: session_id.clone(),
                                        update: SessionUpdate::ToolCallUpdate(ToolCallUpdate {
                                            id: ToolCallId(Arc::from(tool_use_id)),
                                            fields: ToolCallUpdateFields {
                                                status: Some(ToolCallStatus::Completed),
                                                kind: None,
                                                title: None,
                                                content: tool_content,
                                                locations: None,
                                                raw_input: None,
                                                raw_output: None,
                                            },
                                            meta: None,
                                        }),
                                        meta: None,
                                    }));
                                }
                            }
                        }
                    }
                }

                // User messages from keepalive pings should be filtered.
                // We send empty user messages periodically to force the claude CLI to flush
                // its stdout buffer (solving a buffering bug), but these should not be
                // forwarded to clients.
                tracing::debug!("Received user message (keepalive ping or already processed tool_result, filtered)");
                Ok(None)
            }
            "system" => {
                // Check for init subtype with slash_commands
                if let Some(subtype) = parsed.get("subtype").and_then(|v| v.as_str()) {
                    if subtype == "init" {
                        tracing::debug!("Received system init message");

                        // Extract slash_commands array
                        if let Some(slash_commands) =
                            parsed.get("slash_commands").and_then(|v| v.as_array())
                        {
                            let command_names: Vec<String> = slash_commands
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();

                            tracing::info!(
                                "Claude CLI provided {} slash commands: {:?}",
                                command_names.len(),
                                command_names
                            );

                            // Convert to AvailableCommands
                            let available_commands: Vec<agent_client_protocol::AvailableCommand> =
                                command_names
                                    .into_iter()
                                    .map(|cmd_name| {
                                        let (category, description) = if cmd_name
                                            .starts_with("mcp__sah__")
                                        {
                                            (
                                                "mcp_prompt",
                                                format!(
                                                    "SAH: {}",
                                                    cmd_name.strip_prefix("mcp__sah__").unwrap()
                                                ),
                                            )
                                        } else if cmd_name.starts_with("mcp__") {
                                            ("mcp_prompt", format!("MCP: {}", cmd_name))
                                        } else {
                                            ("claude_builtin", format!("Claude: {}", cmd_name))
                                        };

                                        agent_client_protocol::AvailableCommand {
                                            name: cmd_name,
                                            description,
                                            input: None,
                                            meta: Some(serde_json::json!({
                                                "source": "claude_cli",
                                                "category": category,
                                            })),
                                        }
                                    })
                                    .collect();

                            // Return AvailableCommandsUpdate notification
                            return Ok(Some(SessionNotification {
                                session_id: session_id.clone(),
                                update: SessionUpdate::AvailableCommandsUpdate(
                                    agent_client_protocol::AvailableCommandsUpdate {
                                        available_commands,
                                        meta: None,
                                    },
                                ),
                                meta: Some(serde_json::json!({
                                    "source": "claude_cli_init",
                                })),
                            }));
                        }
                    }
                }

                // Other system messages are still metadata only
                tracing::debug!("Received system message (metadata only)");
                Ok(None)
            }
            "result" => {
                // Result messages are metadata only, don't notify
                tracing::debug!("Received result message (metadata only)");
                Ok(None)
            }
            "stream_event" => {
                // Stream events contain partial message chunks (when --include-partial-messages is used)
                // Handle both content_block_delta (for text) and content_block_start (for tool_use)
                if let Some(event) = parsed.get("event") {
                    if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                        match event_type {
                            "content_block_delta" => {
                                // Extract the text from delta.text
                                if let Some(text) = event
                                    .get("delta")
                                    .and_then(|d| d.get("text"))
                                    .and_then(|t| t.as_str())
                                {
                                    tracing::trace!(
                                        "📨 STREAM_EVENT chunk: {} chars: '{}'",
                                        text.len(),
                                        text.chars().take(50).collect::<String>()
                                    );
                                    return Ok(Some(SessionNotification {
                                        session_id: session_id.clone(),
                                        update: SessionUpdate::AgentMessageChunk(
                                            agent_client_protocol::ContentChunk {
                                                content: ContentBlock::Text(TextContent {
                                                    text: text.to_string(),
                                                    annotations: None,
                                                    meta: None,
                                                }),
                                                meta: None,
                                            },
                                        ),
                                        meta: None,
                                    }));
                                }
                            }
                            "content_block_start" => {
                                // Check if this is a tool_use content block
                                if let Some(content_block) = event.get("content_block") {
                                    if let Some(block_type) =
                                        content_block.get("type").and_then(|t| t.as_str())
                                    {
                                        if block_type == "tool_use" {
                                            // Extract tool call information from content_block_start
                                            let id = content_block
                                                .get("id")
                                                .and_then(|i| i.as_str())
                                                .unwrap_or("");
                                            let name = content_block
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("");

                                            tracing::debug!(
                                                "🔧 STREAM_EVENT tool_use start: {} ({})",
                                                name,
                                                id
                                            );

                                            // Note: At content_block_start, the input is empty {}
                                            // We'll get the actual input via content_block_delta events with input_json_delta
                                            // For now, we emit the tool call with empty input and rely on the assistant message
                                            // to provide the complete tool call with full input.

                                            // Actually, let's NOT emit here - we'll let the assistant message handle it
                                            // since it has the complete input. This avoids duplicate tool calls.
                                            return Ok(None);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Ignore other stream_event types (message_start, message_stop, etc.)
                tracing::trace!("Received stream_event (ignored)");
                Ok(None)
            }
            _ => {
                tracing::warn!("Unknown stream-json message type: {}", msg_type);
                Ok(None)
            }
        }
    }

    /// Infer ToolKind from tool name
    ///
    /// Maps common tool names to their appropriate ACP ToolKind categories
    fn infer_tool_kind(tool_name: &str) -> agent_client_protocol::ToolKind {
        use agent_client_protocol::ToolKind;

        // Check for common prefixes and patterns
        if tool_name.contains("read") || tool_name.contains("Read") || tool_name.ends_with("_read")
        {
            ToolKind::Read
        } else if tool_name.contains("write")
            || tool_name.contains("Write")
            || tool_name.ends_with("_write")
            || tool_name.contains("edit")
            || tool_name.contains("Edit")
            || tool_name.ends_with("_edit")
        {
            ToolKind::Edit
        } else if tool_name.contains("delete")
            || tool_name.contains("Delete")
            || tool_name.contains("remove")
        {
            ToolKind::Delete
        } else if tool_name.contains("move")
            || tool_name.contains("Move")
            || tool_name.contains("rename")
        {
            ToolKind::Move
        } else if tool_name.contains("search")
            || tool_name.contains("Search")
            || tool_name.contains("grep")
            || tool_name.contains("Grep")
        {
            ToolKind::Search
        } else if tool_name.contains("bash")
            || tool_name.contains("Bash")
            || tool_name.contains("execute")
            || tool_name.contains("Execute")
        {
            ToolKind::Execute
        } else if tool_name.contains("fetch")
            || tool_name.contains("Fetch")
            || tool_name.contains("web")
        {
            ToolKind::Fetch
        } else {
            ToolKind::Other
        }
    }

    /// Parse result message to extract stop_reason
    ///
    /// # Arguments
    /// * `line` - A single line of JSON from claude stdout
    ///
    /// # Returns
    /// * `Ok(Some(StreamResult))` - Successfully parsed result message
    /// * `Ok(None)` - Not a result message or no stop_reason present
    /// * `Err(...)` - Parse error
    pub fn parse_result_message(line: &str) -> Result<Option<StreamResult>> {
        let parsed: JsonValue = serde_json::from_str(line)
            .map_err(|e| AgentError::Internal(format!("Failed to parse result message: {}", e)))?;

        if parsed.get("type").and_then(|v| v.as_str()) == Some("result") {
            let stop_reason = parsed
                .get("stop_reason")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());

            return Ok(Some(StreamResult { stop_reason }));
        }

        Ok(None)
    }

    /// Convert tool result to stream-json for claude stdin
    ///
    /// # Arguments
    /// * `tool_call_id` - The ID of the tool call this result is for
    /// * `result` - The result content as a string
    ///
    /// # Returns
    /// A JSON string formatted for stream-json input
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn tool_result_to_stream_json(tool_call_id: &str, result: &str) -> Result<String> {
        let message = StreamJsonToolResultMessage {
            r#type: "user".to_string(),
            message: ToolResultMessage {
                role: "user".to_string(),
                content: vec![ToolResultContent {
                    r#type: "tool_result".to_string(),
                    tool_use_id: tool_call_id.to_string(),
                    content: vec![ToolResultTextContent {
                        r#type: "text".to_string(),
                        text: result.to_string(),
                    }],
                }],
            },
        };

        serde_json::to_string(&message).map_err(|e| {
            AgentError::Internal(format!("Failed to serialize tool result message: {}", e))
        })
    }
}

// Internal wire format types for stream-json

#[derive(Serialize, Deserialize)]
struct StreamJsonUserMessage {
    r#type: String,
    message: UserMessage,
}

#[derive(Serialize, Deserialize)]
struct UserMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct StreamJsonAssistantMessage {
    r#type: String,
    message: AssistantMessage,
}

impl StreamJsonAssistantMessage {
    /// Validate that the message type is correct
    fn validate(&self) -> Result<()> {
        if self.r#type != "assistant" {
            return Err(AgentError::Internal(format!(
                "Expected message type 'assistant', got '{}'",
                self.r#type
            )));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: Vec<ContentItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentItem {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: JsonValue,
    },
}

#[derive(Serialize)]
struct StreamJsonToolResultMessage {
    r#type: String,
    message: ToolResultMessage,
}

#[derive(Serialize)]
struct ToolResultMessage {
    role: String,
    content: Vec<ToolResultContent>,
}

#[derive(Serialize)]
struct ToolResultContent {
    r#type: String,
    tool_use_id: String,
    content: Vec<ToolResultTextContent>,
}

#[derive(Serialize)]
struct ToolResultTextContent {
    r#type: String,
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_to_stream_json_simple_text() {
        // Test: Convert simple text message from ACP to stream-json
        let content = vec![ContentBlock::Text(TextContent {
            text: "Hello, world!".to_string(),
            annotations: None,
            meta: None,
        })];

        let result = ProtocolTranslator::acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert_eq!(parsed["message"]["content"], "Hello, world!");
    }

    #[test]
    fn test_stream_json_to_acp_assistant_text() {
        // Test: Assistant text messages should be emitted as AgentMessageChunk
        // With --include-partial-messages, we receive:
        // 1. stream_event chunks with the text (processed in real-time)
        // 2. assistant message with full text (also processed - deduplication handled at higher level in claude.rs)
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello back!"}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        // Should return Some with AgentMessageChunk
        assert!(
            notification.is_some(),
            "Expected Some for assistant text message"
        );

        match notification.unwrap().update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    assert_eq!(text.text, "Hello back!");
                } else {
                    panic!("Expected text content block");
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn test_stream_json_to_acp_system_message() {
        // Test: System messages should return None (metadata only)
        let line = r#"{"type":"system","subtype":"init","session_id":"test"}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_stream_json_to_acp_result_message() {
        // Test: Result messages should return None (metadata only)
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.114}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_stream_json_to_acp_user_message_filtered() {
        // Test: User messages (from keepalive pings) should be filtered
        let line = r#"{"type":"user","message":{"role":"user","content":""}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Keepalive ping messages should be filtered"
        );
    }

    #[test]
    fn test_tool_result_to_stream_json() {
        // Test: Convert tool result to stream-json
        let tool_call_id = "toolu_123";
        let result_text = "File contents here";

        let result = ProtocolTranslator::tool_result_to_stream_json(tool_call_id, result_text);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert!(parsed["message"]["content"].is_array());

        let content = &parsed["message"]["content"][0];
        assert_eq!(content["type"], "tool_result");
        assert_eq!(content["tool_use_id"], tool_call_id);
        assert_eq!(content["content"][0]["type"], "text");
        assert_eq!(content["content"][0]["text"], result_text);
    }

    #[test]
    fn test_stream_json_to_acp_malformed_json() {
        // Test: Malformed JSON should return error
        let line = r#"{"type":"assistant", invalid json"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_json_to_acp_missing_type() {
        // Test: Missing type field should return error
        let line = r#"{"message":{"content":[{"type":"text","text":"Hello"}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_json_to_acp_unknown_type() {
        // Test: Unknown type should return None (skip with warning)
        let line = r#"{"type":"unknown_type","data":"something"}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_result_message_with_max_tokens() {
        // Test: Parse result message with max_tokens stop_reason
        let line = r#"{"type":"result","subtype":"success","stop_reason":"max_tokens","usage":{}}"#;
        let result = ProtocolTranslator::parse_result_message(line);
        assert!(result.is_ok());

        let stream_result = result.unwrap();
        assert!(stream_result.is_some());

        let stream_result = stream_result.unwrap();
        assert_eq!(stream_result.stop_reason, Some("max_tokens".to_string()));
    }

    #[test]
    fn test_parse_result_message_with_end_turn() {
        // Test: Parse result message with end_turn stop_reason
        let line = r#"{"type":"result","subtype":"success","stop_reason":"end_turn","usage":{}}"#;
        let result = ProtocolTranslator::parse_result_message(line);
        assert!(result.is_ok());

        let stream_result = result.unwrap();
        assert!(stream_result.is_some());

        let stream_result = stream_result.unwrap();
        assert_eq!(stream_result.stop_reason, Some("end_turn".to_string()));
    }

    #[test]
    fn test_parse_result_message_without_stop_reason() {
        // Test: Parse result message without stop_reason field
        let line = r#"{"type":"result","subtype":"success","usage":{}}"#;
        let result = ProtocolTranslator::parse_result_message(line);
        assert!(result.is_ok());

        let stream_result = result.unwrap();
        assert!(stream_result.is_some());

        let stream_result = stream_result.unwrap();
        assert_eq!(stream_result.stop_reason, None);
    }

    #[test]
    fn test_parse_result_message_not_result_type() {
        // Test: Non-result message should return None
        let line = r#"{"type":"assistant","message":{"content":[]}}"#;
        let result = ProtocolTranslator::parse_result_message(line);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_stream_json_to_acp_assistant_tool_use() {
        // Test: Convert assistant tool use message from stream-json to ACP ToolCall
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"mcp__sah__files_read","input":{"path":"test.txt"}}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(notification.is_some(), "Expected Some for tool_use message");

        let notification = notification.unwrap();
        match notification.update {
            SessionUpdate::ToolCall(tool_call) => {
                // Verify ToolCall structure per ACP spec
                assert_eq!(tool_call.id.0.as_ref(), "toolu_123");
                assert_eq!(tool_call.title, "mcp__sah__files_read");
                assert_eq!(tool_call.kind, agent_client_protocol::ToolKind::Read);
                assert_eq!(
                    tool_call.status,
                    agent_client_protocol::ToolCallStatus::Pending
                );
                assert!(tool_call.raw_input.is_some());

                // Verify input was preserved
                let input = tool_call.raw_input.unwrap();
                assert_eq!(input["path"], "test.txt");
            }
            _ => panic!(
                "Expected SessionUpdate::ToolCall, got {:?}",
                notification.update
            ),
        }
    }

    #[test]
    fn test_duplicate_prevention_assistant_text_is_emitted() {
        // Test: Assistant messages with TEXT content should be emitted as AgentMessageChunk
        // Note: This may duplicate stream_event chunks, but that's handled at higher level in claude.rs
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello back!"}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(
            notification.is_some(),
            "Expected Some for assistant text message"
        );

        match notification.unwrap().update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    assert_eq!(text.text, "Hello back!");
                } else {
                    panic!("Expected text content block");
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn test_duplicate_prevention_stream_events_are_processed() {
        // Test: stream_event with content_block_delta SHOULD be processed (real-time chunks)
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"Hello"}}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(
            notification.is_some(),
            "Expected Some for stream_event chunk, but got None"
        );

        let notification = notification.unwrap();
        match notification.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    assert_eq!(text.text, "Hello");
                } else {
                    panic!("Expected text content block");
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn test_duplicate_prevention_tool_use_is_not_filtered() {
        // Test: Assistant messages with TOOL_USE content SHOULD be processed as ToolCall
        // because tool_use does NOT come through stream_events
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_456","name":"bash","input":{"command":"ls"}}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(
            notification.is_some(),
            "Expected Some for assistant tool_use message, but got None"
        );

        let notification = notification.unwrap();
        match notification.update {
            SessionUpdate::ToolCall(tool_call) => {
                // Verify tool call is properly structured
                assert_eq!(tool_call.id.0.as_ref(), "toolu_456");
                assert_eq!(tool_call.title, "bash");
                assert_eq!(tool_call.kind, agent_client_protocol::ToolKind::Execute);
                assert_eq!(
                    tool_call.status,
                    agent_client_protocol::ToolCallStatus::Pending
                );

                // Verify input was preserved
                let input = tool_call.raw_input.as_ref().unwrap();
                assert_eq!(input["command"], "ls");
            }
            _ => panic!(
                "Expected SessionUpdate::ToolCall, got {:?}",
                notification.update
            ),
        }
    }

    #[test]
    fn test_duplicate_prevention_full_scenario() {
        // Test: Simulate the full scenario with chunks followed by full message
        // This is what the claude CLI actually sends with --include-partial-messages
        let session_id = SessionId("test_session".into());

        // Step 1: Receive stream_event chunks (these should be processed)
        let chunk1 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"Hello"}}}"#;
        let result1 = ProtocolTranslator::stream_json_to_acp(chunk1, &session_id);
        assert!(result1.is_ok());
        assert!(
            result1.unwrap().is_some(),
            "Expected chunk1 to be processed"
        );

        let chunk2 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":" world"}}}"#;
        let result2 = ProtocolTranslator::stream_json_to_acp(chunk2, &session_id);
        assert!(result2.is_ok());
        assert!(
            result2.unwrap().is_some(),
            "Expected chunk2 to be processed"
        );

        // Step 2: Receive assistant message with full text (this should also be processed)
        // Note: This creates duplication which must be handled at a higher level (in claude.rs)
        let full_message =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}"#;
        let result3 = ProtocolTranslator::stream_json_to_acp(full_message, &session_id);
        assert!(result3.is_ok());
        assert!(
            result3.unwrap().is_some(),
            "Expected full assistant text message to be processed (deduplication happens in claude.rs)"
        );
    }
}
