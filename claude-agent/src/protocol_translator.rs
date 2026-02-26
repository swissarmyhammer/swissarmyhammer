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
use std::sync::Arc;

/// Result information from stream-json result messages
#[derive(Debug, Clone)]
pub struct StreamResult {
    pub stop_reason: Option<String>,
}

/// Protocol translator for converting between ACP and stream-json formats
pub struct ProtocolTranslator {
    permission_engine: Arc<crate::permissions::PermissionPolicyEngine>,
}

impl ProtocolTranslator {
    /// Create a new protocol translator with a permission engine
    pub fn new(permission_engine: Arc<crate::permissions::PermissionPolicyEngine>) -> Self {
        Self { permission_engine }
    }

    /// Convert ACP ContentBlocks to stream-json for claude stdin
    ///
    /// Supports both simple text messages and complex content arrays with images.
    /// The claude CLI's stream-json format accepts:
    /// - Simple string for text-only messages (backward compatible)
    /// - Content array format for messages with images or multiple content blocks
    ///
    /// # Arguments
    /// * `content` - The content blocks to translate
    ///
    /// # Returns
    /// A JSON string formatted for stream-json input
    ///
    /// # Errors
    /// Returns error if content contains unsupported types (audio, resources), or if serialization fails
    pub fn acp_to_stream_json(&self, content: Vec<ContentBlock>) -> Result<String> {
        if Self::is_single_text_block(&content) {
            if let ContentBlock::Text(text_content) = &content[0] {
                return self.serialize_simple_text_message(&text_content.text);
            }
        }

        let content_items = self.convert_content_blocks_to_items(content)?;
        self.serialize_array_message(content_items)
    }

    /// Check if content is a single text block (for simple format).
    fn is_single_text_block(content: &[ContentBlock]) -> bool {
        content.len() == 1 && matches!(content.first(), Some(ContentBlock::Text(_)))
    }

    /// Serialize a simple text message to stream-json.
    fn serialize_simple_text_message(&self, text: &str) -> Result<String> {
        let message = StreamJsonUserMessage {
            r#type: "user".to_string(),
            message: UserMessage {
                role: "user".to_string(),
                content: UserMessageContent::String(text.to_string()),
            },
        };
        serde_json::to_string(&message).map_err(|e| {
            AgentError::Internal(format!("Failed to serialize stream-json message: {}", e))
        })
    }

    /// Convert content blocks to user content items.
    fn convert_content_blocks_to_items(
        &self,
        content: Vec<ContentBlock>,
    ) -> Result<Vec<UserContentItem>> {
        let mut items = Vec::new();
        for block in content {
            items.push(self.convert_block_to_item(block)?);
        }
        Ok(items)
    }

    /// Convert a single content block to a user content item.
    fn convert_block_to_item(&self, block: ContentBlock) -> Result<UserContentItem> {
        match block {
            ContentBlock::Text(text) => Ok(UserContentItem::Text { text: text.text }),
            ContentBlock::Image(img) => Ok(UserContentItem::Image {
                source: ImageSource {
                    source_type: "base64".to_string(),
                    media_type: img.mime_type,
                    data: img.data,
                },
            }),
            ContentBlock::Audio(_) => Err(AgentError::Internal(
                "Audio content blocks are not yet supported".to_string(),
            )),
            ContentBlock::Resource(res) => Ok(self.convert_resource_to_text_item(&res)),
            ContentBlock::ResourceLink(link) => Ok(UserContentItem::ResourceLink {
                uri: link.uri.clone(),
                name: link.name.clone(),
            }),
            _ => Err(AgentError::Internal(
                "Unknown content block type".to_string(),
            )),
        }
    }

    /// Convert a resource content block to a text item.
    fn convert_resource_to_text_item(
        &self,
        resource: &agent_client_protocol::EmbeddedResource,
    ) -> UserContentItem {
        use agent_client_protocol::EmbeddedResourceResource;

        let text = match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(text_res) => {
                format!("Resource ({}): {}", text_res.uri, text_res.text)
            }
            EmbeddedResourceResource::BlobResourceContents(blob_res) => {
                format!(
                    "Resource ({}): [binary data, {} bytes]",
                    blob_res.uri,
                    blob_res.blob.len()
                )
            }
            _ => "Resource: [unsupported type]".to_string(),
        };
        UserContentItem::Text { text }
    }

    /// Serialize array message to stream-json.
    fn serialize_array_message(&self, content_items: Vec<UserContentItem>) -> Result<String> {
        let message = StreamJsonUserMessage {
            r#type: "user".to_string(),
            message: UserMessage {
                role: "user".to_string(),
                content: UserMessageContent::Array(content_items),
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
    pub async fn stream_json_to_acp(
        &self,
        line: &str,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let parsed: JsonValue = serde_json::from_str(line)
            .map_err(|e| AgentError::Internal(format!("Malformed JSON: {}. Line: {}", e, line)))?;

        let msg_type = parsed.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            AgentError::Internal("Missing 'type' field in stream-json".to_string())
        })?;

        match msg_type {
            "assistant" => self.handle_assistant_message(&parsed, session_id).await,
            "user" => self.handle_user_message(&parsed, session_id),
            "system" => self.handle_system_message(&parsed, session_id),
            "result" => {
                tracing::debug!("Received result message (metadata only)");
                Ok(None)
            }
            "stream_event" => self.handle_stream_event(&parsed, session_id),
            _ => {
                tracing::warn!("Unknown stream-json message type: {}", msg_type);
                Ok(None)
            }
        }
    }

    /// Handle assistant message type.
    async fn handle_assistant_message(
        &self,
        parsed: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let content_array = parsed
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                AgentError::Internal("Missing content array in assistant message".to_string())
            })?;

        if let Some(first_item) = content_array.first() {
            let item_type = first_item.get("type").and_then(|t| t.as_str());

            match item_type {
                Some("tool_use") => {
                    return self.handle_tool_use(first_item, session_id).await;
                }
                Some("text") => {
                    return self.handle_assistant_text(first_item, session_id);
                }
                _ => {}
            }
        }

        Ok(None)
    }

    /// Handle tool_use content in assistant message.
    async fn handle_tool_use(
        &self,
        item: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        use agent_client_protocol::{ToolCall, ToolCallId};

        let id = item
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| AgentError::Internal("Missing id in tool_use".to_string()))?;
        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| AgentError::Internal("Missing name in tool_use".to_string()))?;
        let input = item
            .get("input")
            .ok_or_else(|| AgentError::Internal("Missing input in tool_use".to_string()))?;

        tracing::debug!("🔧 ASSISTANT tool_use: {} ({})", name, id);

        let policy_evaluation = self
            .permission_engine
            .evaluate_tool_call(name, input)
            .await?;
        let (status, meta) = self.evaluate_tool_policy(name, policy_evaluation);

        let tool_call = ToolCall::new(ToolCallId::new(id), name)
            .kind(Self::infer_tool_kind(name))
            .status(status)
            .raw_input(Some(input.clone()));

        let tool_call = if let Some(meta_map) = meta {
            tool_call.meta(meta_map)
        } else {
            tool_call
        };

        Ok(Some(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::ToolCall(tool_call),
        )))
    }

    /// Evaluate tool call policy and return status and metadata.
    fn evaluate_tool_policy(
        &self,
        name: &str,
        policy_evaluation: crate::permissions::PolicyEvaluation,
    ) -> (
        agent_client_protocol::ToolCallStatus,
        Option<serde_json::Map<String, JsonValue>>,
    ) {
        use crate::permissions::PolicyEvaluation;
        use agent_client_protocol::ToolCallStatus;

        match policy_evaluation {
            PolicyEvaluation::Allowed => {
                tracing::debug!("Tool call '{}' allowed by policy", name);
                (ToolCallStatus::Pending, None)
            }
            PolicyEvaluation::Denied { reason } => {
                tracing::warn!("Tool call '{}' denied by policy: {}", name, reason);
                let mut map = serde_json::Map::new();
                map.insert("permission_denied".to_string(), serde_json::json!(true));
                map.insert("reason".to_string(), serde_json::json!(reason));
                (ToolCallStatus::Failed, Some(map))
            }
            PolicyEvaluation::RequireUserConsent { options } => {
                tracing::debug!("Tool call '{}' requires user consent", name);
                let mut map = serde_json::Map::new();
                map.insert("requires_permission".to_string(), serde_json::json!(true));
                map.insert("permission_options".to_string(), serde_json::json!(options));
                (ToolCallStatus::Pending, Some(map))
            }
        }
    }

    /// Handle text content in assistant message.
    fn handle_assistant_text(
        &self,
        item: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let text = item
            .get("text")
            .and_then(|t| t.as_str())
            .ok_or_else(|| AgentError::Internal("Missing text in text content".to_string()))?;

        tracing::debug!("📨 ASSISTANT text: {} chars", text.len());
        let text_content = TextContent::new(text.to_string());
        let content_block = ContentBlock::Text(text_content);
        let content_chunk = agent_client_protocol::ContentChunk::new(content_block);

        Ok(Some(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::AgentMessageChunk(content_chunk),
        )))
    }

    /// Handle user message type.
    fn handle_user_message(
        &self,
        parsed: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        tracing::debug!("📥 USER message received, checking for tool_result");

        if let Some(message) = parsed.get("message") {
            if let Some(content_array) = message.get("content").and_then(|c| c.as_array()) {
                for content_item in content_array {
                    if let Some(notification) =
                        self.try_handle_tool_result(content_item, session_id)?
                    {
                        return Ok(Some(notification));
                    }
                }
            }
        }

        tracing::debug!("Received user message (keepalive ping or filtered)");
        Ok(None)
    }

    /// Try to handle a tool_result content item.
    fn try_handle_tool_result(
        &self,
        content_item: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        if content_item.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
            return Ok(None);
        }

        let tool_use_id = match content_item.get("tool_use_id").and_then(|id| id.as_str()) {
            Some(id) => id,
            None => return Ok(None),
        };

        tracing::trace!("🎯 TOOL_RESULT for tool_id: {}", tool_use_id);

        let tool_content = self.extract_tool_result_content(content_item);

        use agent_client_protocol::{
            ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
        };

        let mut fields = ToolCallUpdateFields::new().status(ToolCallStatus::Completed);
        if let Some(content) = tool_content {
            fields = fields.content(content);
        }

        let tool_call_update = ToolCallUpdate::new(ToolCallId::new(tool_use_id), fields);
        Ok(Some(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::ToolCallUpdate(tool_call_update),
        )))
    }

    /// Extract content from tool_result (string or array format).
    fn extract_tool_result_content(
        &self,
        content_item: &JsonValue,
    ) -> Option<Vec<agent_client_protocol::ToolCallContent>> {
        let content_value = content_item.get("content")?;

        if let Some(content_str) = content_value.as_str() {
            return Some(vec![agent_client_protocol::ToolCallContent::Content(
                agent_client_protocol::Content::new(ContentBlock::Text(TextContent::new(
                    content_str.to_string(),
                ))),
            )]);
        }

        if let Some(content_array) = content_value.as_array() {
            let result: Vec<_> = content_array
                .iter()
                .filter_map(|item| self.parse_tool_content_item(item))
                .collect();

            if !result.is_empty() {
                return Some(result);
            }
        }

        None
    }

    /// Parse a single tool content item.
    fn parse_tool_content_item(
        &self,
        item: &JsonValue,
    ) -> Option<agent_client_protocol::ToolCallContent> {
        let item_type = item.get("type").and_then(|t| t.as_str())?;

        match item_type {
            "text" => {
                let text = item.get("text").and_then(|t| t.as_str())?;
                Some(agent_client_protocol::ToolCallContent::Content(
                    agent_client_protocol::Content::new(ContentBlock::Text(TextContent::new(
                        text.to_string(),
                    ))),
                ))
            }
            _ => {
                tracing::debug!("Unknown tool_result content type: {}", item_type);
                None
            }
        }
    }

    /// Handle system message type.
    fn handle_system_message(
        &self,
        parsed: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let subtype = parsed.get("subtype").and_then(|v| v.as_str());

        if subtype == Some("init") {
            return self.handle_system_init(parsed, session_id);
        }

        tracing::debug!("Received system message (metadata only)");
        Ok(None)
    }

    /// Handle system init message with slash_commands and agents.
    fn handle_system_init(
        &self,
        parsed: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        tracing::debug!("Received system init message");

        let current_agent = parsed
            .get("current_agent")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let available_agents = self.extract_available_agents(parsed);

        if let Some(agents) = &available_agents {
            tracing::info!(
                "Claude CLI provided {} agents: {:?}",
                agents.len(),
                agents
                    .iter()
                    .map(|(id, name, _)| format!("{}:{}", id, name))
                    .collect::<Vec<_>>()
            );
        }

        let slash_commands = parsed.get("slash_commands").and_then(|v| v.as_array());
        if let Some(commands) = slash_commands {
            return self.build_commands_notification(
                commands,
                available_agents,
                current_agent,
                session_id,
            );
        }

        Ok(None)
    }

    /// Extract available agents from parsed JSON.
    fn extract_available_agents(
        &self,
        parsed: &JsonValue,
    ) -> Option<Vec<(String, String, Option<String>)>> {
        parsed
            .get("agents")
            .and_then(|v| v.as_array())
            .map(|agents| {
                agents
                    .iter()
                    .filter_map(|agent| {
                        let id = agent.as_str()?;
                        let name = Self::format_agent_name(id);
                        Some((id.to_string(), name, None::<String>))
                    })
                    .collect()
            })
    }

    /// Format agent ID as human-readable name.
    fn format_agent_name(id: &str) -> String {
        id.split('-')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Build AvailableCommandsUpdate notification.
    fn build_commands_notification(
        &self,
        slash_commands: &[JsonValue],
        available_agents: Option<Vec<(String, String, Option<String>)>>,
        current_agent: Option<String>,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let command_names: Vec<String> = slash_commands
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        tracing::info!(
            "Claude CLI provided {} slash commands: {:?}",
            command_names.len(),
            command_names
        );

        let available_commands = self.convert_commands_to_acp(command_names);
        let commands_update =
            agent_client_protocol::AvailableCommandsUpdate::new(available_commands);

        let mut meta_map = serde_json::Map::new();
        meta_map.insert("source".to_string(), serde_json::json!("claude_cli_init"));

        if let Some(agents) = available_agents {
            meta_map.insert("available_agents".to_string(), serde_json::json!(agents));
        }

        if let Some(current) = current_agent {
            meta_map.insert("current_agent".to_string(), serde_json::json!(current));
        }

        Ok(Some(
            SessionNotification::new(
                session_id.clone(),
                SessionUpdate::AvailableCommandsUpdate(commands_update),
            )
            .meta(meta_map),
        ))
    }

    /// Convert command names to ACP AvailableCommand format.
    fn convert_commands_to_acp(
        &self,
        command_names: Vec<String>,
    ) -> Vec<agent_client_protocol::AvailableCommand> {
        command_names
            .into_iter()
            .map(|cmd_name| {
                let (category, description) = Self::categorize_command(&cmd_name);
                let mut meta_map = serde_json::Map::new();
                meta_map.insert("source".to_string(), serde_json::json!("claude_cli"));
                meta_map.insert("category".to_string(), serde_json::json!(category));
                agent_client_protocol::AvailableCommand::new(cmd_name, description).meta(meta_map)
            })
            .collect()
    }

    /// Categorize a command by name prefix.
    fn categorize_command(cmd_name: &str) -> (&'static str, String) {
        if cmd_name.starts_with("mcp__sah__") {
            (
                "mcp_prompt",
                format!("SAH: {}", cmd_name.strip_prefix("mcp__sah__").unwrap()),
            )
        } else if cmd_name.starts_with("mcp__") {
            ("mcp_prompt", format!("MCP: {}", cmd_name))
        } else {
            ("claude_builtin", format!("Claude: {}", cmd_name))
        }
    }

    /// Handle stream_event message type.
    fn handle_stream_event(
        &self,
        parsed: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let event = match parsed.get("event") {
            Some(e) => e,
            None => {
                tracing::trace!("Received stream_event (no event field)");
                return Ok(None);
            }
        };

        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            "content_block_delta" => self.handle_content_block_delta(event, session_id),
            "content_block_start" => self.handle_content_block_start(event),
            _ => {
                tracing::trace!("Received stream_event (ignored)");
                Ok(None)
            }
        }
    }

    /// Handle content_block_delta stream event.
    fn handle_content_block_delta(
        &self,
        event: &JsonValue,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        // Check for text delta
        if let Some(text) = event
            .get("delta")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            return self.handle_text_delta(event, text, session_id);
        }

        // Check for input_json_delta
        if let Some(delta) = event.get("delta") {
            if let Some(input_json_delta) = delta.get("input_json_delta").and_then(|d| d.as_str()) {
                return self.handle_input_json_delta(event, input_json_delta);
            }
        }

        Ok(None)
    }

    /// Handle text delta in stream event.
    fn handle_text_delta(
        &self,
        event: &JsonValue,
        text: &str,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        let content_block_index = event.get("index").and_then(|i| i.as_u64());

        tracing::trace!(
            "📨 STREAM_EVENT chunk: index={:?}, {} chars: '{}'",
            content_block_index,
            text.len(),
            text
        );

        let text_content = TextContent::new(text.to_string());
        let content_block = ContentBlock::Text(text_content);
        let content_chunk = agent_client_protocol::ContentChunk::new(content_block);

        let mut notification = SessionNotification::new(
            session_id.clone(),
            SessionUpdate::AgentMessageChunk(content_chunk),
        );

        if let Some(idx) = content_block_index {
            let mut meta = serde_json::Map::new();
            meta.insert("content_block_index".to_string(), serde_json::json!(idx));
            meta.insert("source".to_string(), serde_json::json!("stream_event"));
            notification = notification.meta(meta);
        }

        Ok(Some(notification))
    }

    /// Handle input_json_delta in stream event.
    fn handle_input_json_delta(
        &self,
        event: &JsonValue,
        input_json_delta: &str,
    ) -> Result<Option<SessionNotification>> {
        if let Some(index) = event.get("index").and_then(|i| i.as_u64()) {
            tracing::trace!(
                "🔧 STREAM_EVENT input_json_delta: index={}, {} chars",
                index,
                input_json_delta.len()
            );
            tracing::debug!(
                "Tool input chunk received for content block {}: '{}'",
                index,
                input_json_delta
            );
        }
        Ok(None)
    }

    /// Handle content_block_start stream event.
    fn handle_content_block_start(&self, event: &JsonValue) -> Result<Option<SessionNotification>> {
        if let Some(content_block) = event.get("content_block") {
            if content_block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                let id = content_block
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("");
                let name = content_block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                tracing::debug!("🔧 STREAM_EVENT tool_use start: {} ({})", name, id);
            }
        }
        Ok(None)
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
    pub fn parse_result_message(&self, line: &str) -> Result<Option<StreamResult>> {
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
    pub fn tool_result_to_stream_json(&self, tool_call_id: &str, result: &str) -> Result<String> {
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
    content: UserMessageContent,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum UserMessageContent {
    String(String),
    Array(Vec<UserContentItem>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum UserContentItem {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "resource_link")]
    ResourceLink { uri: String, name: String },
}

#[derive(Serialize, Deserialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct StreamJsonAssistantMessage {
    r#type: String,
    message: AssistantMessage,
}

impl StreamJsonAssistantMessage {
    /// Validate that the message type is correct
    #[allow(dead_code)]
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
#[allow(dead_code)]
struct AssistantMessage {
    content: Vec<ContentItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
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
    use tempfile::tempdir;

    fn create_test_translator() -> ProtocolTranslator {
        let temp_dir = tempdir().unwrap();
        let storage = crate::permissions::FilePermissionStorage::new(temp_dir.path().to_path_buf());
        let permission_engine = Arc::new(crate::permissions::PermissionPolicyEngine::new(
            Box::new(storage),
        ));
        ProtocolTranslator::new(permission_engine)
    }

    #[test]
    fn test_acp_to_stream_json_simple_text() {
        // Test: Convert simple text message from ACP to stream-json
        let translator = create_test_translator();
        let content = vec![ContentBlock::Text(TextContent::new("Hello, world!"))];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert_eq!(parsed["message"]["content"], "Hello, world!");
    }

    #[test]
    fn test_acp_to_stream_json_single_image() {
        use agent_client_protocol::ImageContent;

        // Test: Convert image message from ACP to stream-json
        let translator = create_test_translator();
        let content = vec![ContentBlock::Image(ImageContent::new(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
            "image/png",
        ))];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");

        // Should be array format for images
        assert!(parsed["message"]["content"].is_array());
        let content_array = parsed["message"]["content"].as_array().unwrap();
        assert_eq!(content_array.len(), 1);
        assert_eq!(content_array[0]["type"], "image");
        assert_eq!(content_array[0]["source"]["type"], "base64");
        assert_eq!(content_array[0]["source"]["media_type"], "image/png");
        assert_eq!(content_array[0]["source"]["data"], "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==");
    }

    #[test]
    fn test_acp_to_stream_json_text_and_image() {
        use agent_client_protocol::ImageContent;

        // Test: Convert mixed content (text + image) from ACP to stream-json
        let translator = create_test_translator();
        let content = vec![
            ContentBlock::Text(TextContent::new("Here's an image:")),
            ContentBlock::Image(ImageContent::new("base64data", "image/jpeg")),
        ];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");

        // Should be array format for mixed content
        assert!(parsed["message"]["content"].is_array());
        let content_array = parsed["message"]["content"].as_array().unwrap();
        assert_eq!(content_array.len(), 2);

        // First item is text
        assert_eq!(content_array[0]["type"], "text");
        assert_eq!(content_array[0]["text"], "Here's an image:");

        // Second item is image
        assert_eq!(content_array[1]["type"], "image");
        assert_eq!(content_array[1]["source"]["type"], "base64");
        assert_eq!(content_array[1]["source"]["media_type"], "image/jpeg");
    }

    #[test]
    fn test_acp_to_stream_json_audio_unsupported() {
        use agent_client_protocol::AudioContent;

        // Test: Audio content should return an error
        let translator = create_test_translator();
        let content = vec![ContentBlock::Audio(AudioContent::new(
            "audiodata".to_string(),
            "audio/wav".to_string(),
        ))];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Audio content blocks are not yet supported"));
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_assistant_text() {
        // Test: Assistant text messages should be emitted as AgentMessageChunk
        // With --include-partial-messages, we receive:
        // 1. stream_event chunks with the text (processed in real-time)
        // 2. assistant message with full text (also processed - deduplication handled at higher level in claude.rs)
        let translator = create_test_translator();
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello back!"}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
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

    #[tokio::test]
    async fn test_stream_json_to_acp_system_message() {
        // Test: System messages should return None (metadata only)
        let translator = create_test_translator();
        let line = r#"{"type":"system","subtype":"init","session_id":"test"}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_result_message() {
        // Test: Result messages should return None (metadata only)
        let translator = create_test_translator();
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.114}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_user_message_filtered() {
        // Test: User messages (from keepalive pings) should be filtered
        let translator = create_test_translator();
        let line = r#"{"type":"user","message":{"role":"user","content":""}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Keepalive ping messages should be filtered"
        );
    }

    #[test]
    fn test_tool_result_to_stream_json() {
        // Test: Convert tool result to stream-json
        let translator = create_test_translator();
        let tool_call_id = "toolu_123";
        let result_text = "File contents here";

        let result = translator.tool_result_to_stream_json(tool_call_id, result_text);
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

    #[tokio::test]
    async fn test_stream_json_to_acp_malformed_json() {
        // Test: Malformed JSON should return error
        let translator = create_test_translator();
        let line = r#"{"type":"assistant", invalid json"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_missing_type() {
        // Test: Missing type field should return error
        let translator = create_test_translator();
        let line = r#"{"message":{"content":[{"type":"text","text":"Hello"}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_unknown_type() {
        // Test: Unknown type should return None (skip with warning)
        let translator = create_test_translator();
        let line = r#"{"type":"unknown_type","data":"something"}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_result_message_with_max_tokens() {
        // Test: Parse result message with max_tokens stop_reason
        let translator = create_test_translator();
        let line = r#"{"type":"result","subtype":"success","stop_reason":"max_tokens","usage":{}}"#;
        let result = translator.parse_result_message(line);
        assert!(result.is_ok());

        let stream_result = result.unwrap();
        assert!(stream_result.is_some());

        let stream_result = stream_result.unwrap();
        assert_eq!(stream_result.stop_reason, Some("max_tokens".to_string()));
    }

    #[test]
    fn test_parse_result_message_with_end_turn() {
        // Test: Parse result message with end_turn stop_reason
        let translator = create_test_translator();
        let line = r#"{"type":"result","subtype":"success","stop_reason":"end_turn","usage":{}}"#;
        let result = translator.parse_result_message(line);
        assert!(result.is_ok());

        let stream_result = result.unwrap();
        assert!(stream_result.is_some());

        let stream_result = stream_result.unwrap();
        assert_eq!(stream_result.stop_reason, Some("end_turn".to_string()));
    }

    #[test]
    fn test_parse_result_message_without_stop_reason() {
        // Test: Parse result message without stop_reason field
        let translator = create_test_translator();
        let line = r#"{"type":"result","subtype":"success","usage":{}}"#;
        let result = translator.parse_result_message(line);
        assert!(result.is_ok());

        let stream_result = result.unwrap();
        assert!(stream_result.is_some());

        let stream_result = stream_result.unwrap();
        assert_eq!(stream_result.stop_reason, None);
    }

    #[test]
    fn test_parse_result_message_not_result_type() {
        // Test: Non-result message should return None
        let translator = create_test_translator();
        let line = r#"{"type":"assistant","message":{"content":[]}}"#;
        let result = translator.parse_result_message(line);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_assistant_tool_use() {
        // Test: Convert assistant tool use message from stream-json to ACP ToolCall
        let translator = create_test_translator();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"mcp__sah__files","input":{"path":"test.txt"}}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(notification.is_some(), "Expected Some for tool_use message");

        let notification = notification.unwrap();
        match notification.update {
            SessionUpdate::ToolCall(tool_call) => {
                // Verify ToolCall structure per ACP spec
                assert_eq!(tool_call.tool_call_id.0.as_ref(), "toolu_123");
                assert_eq!(tool_call.title, "mcp__sah__files");
                assert_eq!(tool_call.kind, agent_client_protocol::ToolKind::Other);
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

    #[tokio::test]
    async fn test_duplicate_prevention_assistant_text_is_emitted() {
        // Test: Assistant messages with TEXT content should be emitted as AgentMessageChunk
        // Note: This may duplicate stream_event chunks, but that's handled at higher level in claude.rs
        let translator = create_test_translator();
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello back!"}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
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

    #[tokio::test]
    async fn test_duplicate_prevention_stream_events_are_processed() {
        // Test: stream_event with content_block_delta SHOULD be processed (real-time chunks)
        let translator = create_test_translator();
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"Hello"}}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
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

    #[tokio::test]
    async fn test_duplicate_prevention_tool_use_is_not_filtered() {
        // Test: Assistant messages with TOOL_USE content SHOULD be processed as ToolCall
        // because tool_use does NOT come through stream_events
        let translator = create_test_translator();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_456","name":"bash","input":{"command":"ls"}}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
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
                assert_eq!(tool_call.tool_call_id.0.as_ref(), "toolu_456");
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

    #[tokio::test]
    async fn test_duplicate_prevention_full_scenario() {
        // Test: Simulate the full scenario with chunks followed by full message
        // This is what the claude CLI actually sends with --include-partial-messages
        let translator = create_test_translator();
        let session_id = SessionId::new("test_session");

        // Step 1: Receive stream_event chunks (these should be processed)
        let chunk1 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"Hello"}}}"#;
        let result1 = translator.stream_json_to_acp(chunk1, &session_id).await;
        assert!(result1.is_ok());
        assert!(
            result1.unwrap().is_some(),
            "Expected chunk1 to be processed"
        );

        let chunk2 = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":" world"}}}"#;
        let result2 = translator.stream_json_to_acp(chunk2, &session_id).await;
        assert!(result2.is_ok());
        assert!(
            result2.unwrap().is_some(),
            "Expected chunk2 to be processed"
        );

        // Step 2: Receive assistant message with full text (this should also be processed)
        // Note: This creates duplication which must be handled at a higher level (in claude.rs)
        let full_message =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}"#;
        let result3 = translator
            .stream_json_to_acp(full_message, &session_id)
            .await;
        assert!(result3.is_ok());
        assert!(
            result3.unwrap().is_some(),
            "Expected full assistant text message to be processed (deduplication happens in claude.rs)"
        );
    }

    #[test]
    fn test_acp_to_stream_json_resource_link() {
        use agent_client_protocol::ResourceLink;

        // Test: ResourceLink should be converted to resource_link format
        let translator = create_test_translator();
        let content = vec![
            ContentBlock::Text(TextContent::new("Here's a document:")),
            ContentBlock::ResourceLink(ResourceLink::new(
                "https://example.com/document.pdf",
                "Example Document",
            )),
        ];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");

        // Should be array format
        assert!(parsed["message"]["content"].is_array());
        let content_array = parsed["message"]["content"].as_array().unwrap();
        assert_eq!(content_array.len(), 2);

        // First item is text
        assert_eq!(content_array[0]["type"], "text");
        assert_eq!(content_array[0]["text"], "Here's a document:");

        // Second item is resource_link
        assert_eq!(content_array[1]["type"], "resource_link");
        assert_eq!(content_array[1]["uri"], "https://example.com/document.pdf");
        assert_eq!(content_array[1]["name"], "Example Document");
    }

    #[test]
    fn test_acp_to_stream_json_embedded_resource_text() {
        use agent_client_protocol::{
            EmbeddedResource, EmbeddedResourceResource, TextResourceContents,
        };

        // Test: EmbeddedResource with text content should be converted to text format
        let translator = create_test_translator();
        let text_resource =
            TextResourceContents::new("file:///test.txt", "Test content").mime_type("text/plain");

        let content = vec![ContentBlock::Resource(EmbeddedResource::new(
            EmbeddedResourceResource::TextResourceContents(text_resource),
        ))];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");

        // Should be array format
        assert!(parsed["message"]["content"].is_array());
        let content_array = parsed["message"]["content"].as_array().unwrap();
        assert_eq!(content_array.len(), 1);

        // Should be text item with resource info
        assert_eq!(content_array[0]["type"], "text");
        let text = content_array[0]["text"].as_str().unwrap();
        assert!(text.contains("file:///test.txt"));
        assert!(text.contains("Test content"));
    }

    #[test]
    fn test_acp_to_stream_json_embedded_resource_blob() {
        use agent_client_protocol::{
            BlobResourceContents, EmbeddedResource, EmbeddedResourceResource,
        };

        // Test: EmbeddedResource with blob content should be converted to text format with size info
        let translator = create_test_translator();
        let blob_resource = BlobResourceContents::new("base64encodeddata", "file:///test.bin")
            .mime_type("application/octet-stream");

        let content = vec![ContentBlock::Resource(EmbeddedResource::new(
            EmbeddedResourceResource::BlobResourceContents(blob_resource),
        ))];

        let result = translator.acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");

        // Should be array format
        assert!(parsed["message"]["content"].is_array());
        let content_array = parsed["message"]["content"].as_array().unwrap();
        assert_eq!(content_array.len(), 1);

        // Should be text item with resource info
        assert_eq!(content_array[0]["type"], "text");
        let text = content_array[0]["text"].as_str().unwrap();
        assert!(text.contains("file:///test.bin"));
        assert!(text.contains("binary data"));
        assert!(text.contains("17 bytes")); // Length of "base64encodeddata"
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_input_json_delta() {
        // Test: input_json_delta chunks should be logged but not emitted
        // Full tool call will come in the assistant message
        let translator = create_test_translator();
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"input_json_delta":"{\"path\":"}}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());

        // Should return None - we don't emit individual input chunks
        // The complete tool call will be emitted from the assistant message
        assert!(
            result.unwrap().is_none(),
            "input_json_delta chunks should not be emitted as notifications"
        );
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_tool_call_streaming_scenario() {
        // Test: Simulate a complete tool call streaming scenario
        let translator = create_test_translator();
        let session_id = SessionId::new("test_session");

        // Step 1: content_block_start with tool_use (not emitted)
        let start = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_123","name":"read_file","input":{}}}}"#;
        let result1 = translator.stream_json_to_acp(start, &session_id).await;
        assert!(result1.is_ok());
        assert!(
            result1.unwrap().is_none(),
            "content_block_start should not emit"
        );

        // Step 2: Multiple input_json_delta chunks (not emitted individually)
        let delta1 = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"input_json_delta":"{\"path\":"}}}"#;
        let result2 = translator.stream_json_to_acp(delta1, &session_id).await;
        assert!(result2.is_ok());
        assert!(
            result2.unwrap().is_none(),
            "input_json_delta should not emit"
        );

        let delta2 = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"input_json_delta":"\"test.txt\"}"}}}"#;
        let result3 = translator.stream_json_to_acp(delta2, &session_id).await;
        assert!(result3.is_ok());
        assert!(
            result3.unwrap().is_none(),
            "input_json_delta should not emit"
        );

        // Step 3: Final assistant message with complete tool call (emitted as ToolCall)
        let complete = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{"path":"test.txt"}}]}}"#;
        let result4 = translator.stream_json_to_acp(complete, &session_id).await;
        assert!(result4.is_ok());

        let notification = result4.unwrap();
        assert!(
            notification.is_some(),
            "Complete tool call should be emitted"
        );

        match notification.unwrap().update {
            SessionUpdate::ToolCall(tool_call) => {
                assert_eq!(tool_call.tool_call_id.0.as_ref(), "toolu_123");
                assert_eq!(tool_call.title, "read_file");
                assert!(tool_call.raw_input.is_some());
                let input = tool_call.raw_input.unwrap();
                assert_eq!(input["path"], "test.txt");
            }
            _ => panic!("Expected ToolCall notification"),
        }
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_tool_result_string_content() {
        // Test: tool_result with string content (legacy format)
        let translator = create_test_translator();
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"File contents here"}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(
            notification.is_some(),
            "Expected Some for tool_result message"
        );

        match notification.unwrap().update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.tool_call_id.0.as_ref(), "toolu_123");
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Completed)
                );

                // Verify content was extracted
                assert!(update.fields.content.is_some());
                let content = update.fields.content.unwrap();
                assert_eq!(content.len(), 1);

                match &content[0] {
                    agent_client_protocol::ToolCallContent::Content(content_wrapper) => {
                        let block = &content_wrapper.content;
                        match block {
                            ContentBlock::Text(text) => {
                                assert_eq!(text.text, "File contents here");
                            }
                            _ => panic!("Expected text content block"),
                        }
                    }
                    _ => panic!("Expected Content variant"),
                }
            }
            _ => panic!("Expected ToolCallUpdate"),
        }
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_tool_result_array_content() {
        // Test: tool_result with array of content blocks (current format)
        let translator = create_test_translator();
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_456","content":[{"type":"text","text":"First chunk"},{"type":"text","text":"Second chunk"}]}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(
            notification.is_some(),
            "Expected Some for tool_result message with array content"
        );

        match notification.unwrap().update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.tool_call_id.0.as_ref(), "toolu_456");
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Completed)
                );

                // Verify content chunks were extracted
                assert!(update.fields.content.is_some());
                let content = update.fields.content.unwrap();
                assert_eq!(content.len(), 2, "Expected 2 content chunks");

                // Check first chunk
                match &content[0] {
                    agent_client_protocol::ToolCallContent::Content(content_wrapper) => {
                        let block = &content_wrapper.content;
                        match block {
                            ContentBlock::Text(text) => {
                                assert_eq!(text.text, "First chunk");
                            }
                            _ => panic!("Expected text content block"),
                        }
                    }
                    _ => panic!("Expected Content variant"),
                }

                // Check second chunk
                match &content[1] {
                    agent_client_protocol::ToolCallContent::Content(content_wrapper) => {
                        let block = &content_wrapper.content;
                        match block {
                            ContentBlock::Text(text) => {
                                assert_eq!(text.text, "Second chunk");
                            }
                            _ => panic!("Expected text content block"),
                        }
                    }
                    _ => panic!("Expected Content variant"),
                }
            }
            _ => panic!("Expected ToolCallUpdate"),
        }
    }

    #[tokio::test]
    async fn test_stream_json_to_acp_tool_result_empty_content() {
        // Test: tool_result with empty content
        let translator = create_test_translator();
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_789","content":[]}]}}"#;
        let session_id = SessionId::new("test_session");

        let result = translator.stream_json_to_acp(line, &session_id).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(
            notification.is_some(),
            "Expected Some for tool_result message"
        );

        match notification.unwrap().update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.tool_call_id.0.as_ref(), "toolu_789");
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Completed)
                );

                // Empty content array should result in None
                assert!(
                    update.fields.content.is_none(),
                    "Expected None for empty content array"
                );
            }
            _ => panic!("Expected ToolCallUpdate"),
        }
    }
}
