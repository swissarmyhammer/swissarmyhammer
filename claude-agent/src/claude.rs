//! Claude process wrapper providing session-aware interactions

use agent_client_protocol::{ContentBlock, SessionUpdate, TextContent};
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::{
    claude_process::{ClaudeProcess, ClaudeProcessManager},
    config::ClaudeConfig,
    error::Result,
    protocol_translator::ProtocolTranslator,
    session::{MessageRole, SessionId},
};

/// Claude client wrapper with session management
pub struct ClaudeClient {
    process_manager: Arc<ClaudeProcessManager>,
    protocol_translator: Arc<ProtocolTranslator>,
    notification_sender: Option<Arc<crate::agent::NotificationSender>>,
    raw_message_manager: Option<crate::agent::RawMessageManager>,
}

impl ClaudeClient {
    /// Set notification sender for forwarding all notifications from protocol_translator
    pub fn set_notification_sender(&mut self, sender: Arc<crate::agent::NotificationSender>) {
        self.notification_sender = Some(sender);
    }

    /// Set raw message manager for recording JSON-RPC messages
    pub fn set_raw_message_manager(&mut self, manager: crate::agent::RawMessageManager) {
        self.raw_message_manager = Some(manager);
    }

    /// Terminate the Claude process for a session
    ///
    /// This kills the process and removes it from the process manager.
    /// The process will be automatically respawned on the next prompt.
    pub async fn terminate_session(&self, session_id: &crate::session::SessionId) -> Result<()> {
        self.process_manager.terminate_session(session_id).await
    }

    /// Spawn Claude process and consume init message during session creation
    ///
    /// This ensures the Claude process is ready and we've processed the system/init
    /// message (which contains slash_commands and available_agents) before responding to new_session.
    ///
    /// # Arguments
    /// * `session_id` - Internal session identifier
    /// * `acp_session_id` - ACP protocol session identifier
    /// * `cwd` - Working directory for the Claude process
    ///
    /// # Returns
    /// Returns (available_agents, current_agent) if present in init message
    pub async fn spawn_process_and_consume_init(
        &self,
        session_id: &crate::session::SessionId,
        acp_session_id: &agent_client_protocol::SessionId,
        cwd: &std::path::Path,
        mcp_servers: Vec<crate::config::McpServerConfig>,
    ) -> Result<(
        Option<Vec<(String, String, Option<String>)>>,
        Option<String>,
    )> {
        tracing::debug!(
            "Spawning Claude process for session: {} in {} with {} MCP servers",
            session_id,
            cwd.display(),
            mcp_servers.len()
        );

        // Spawn the process (or get existing) with working directory
        // Note: During init, we don't have a mode yet, so pass None
        let process = self
            .process_manager
            .get_process(session_id, cwd, None, mcp_servers)
            .await?;

        // The Claude CLI emits a system/init message AFTER receiving the first input
        // Send a minimal greeting to trigger init without priming Claude's behavior
        // CRITICAL: This message stays in conversation history! Avoid "init"/"help"/"setup"
        // which prime Claude into initialization mode, causing generic responses.
        tracing::debug!("Sending init trigger message to Claude");
        {
            let mut proc = process.lock().await;
            // Use simple greeting that won't bias Claude's behavior on subsequent prompts
            let init_trigger = r#"{"type":"user","message":{"role":"user","content":"hi"}}"#;
            if let Err(e) = proc.write_line(init_trigger).await {
                tracing::error!("Failed to write init trigger to Claude: {}", e);
                return Err(e);
            }
            tracing::debug!("Init trigger sent successfully");
        }

        // Read the init message (should be first line after sending message)
        tracing::debug!("Reading system/init message from Claude CLI");

        let init_line = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let mut proc = process.lock().await;
            proc.read_line().await
        })
        .await;

        let mut available_agents = None;
        let mut current_agent = None;

        match init_line {
            Ok(Ok(Some(line))) => {
                tracing::info!("Received init line from Claude CLI ({} bytes)", line.len());

                // Record raw JSON-RPC message
                if let Some(ref manager) = self.raw_message_manager {
                    manager.record(line.clone());
                }

                // Parse through protocol_translator
                match self
                    .protocol_translator
                    .stream_json_to_acp(&line, acp_session_id)
                    .await
                {
                    Ok(Some(notification)) => {
                        tracing::info!(
                            "Protocol translator created notification from init message"
                        );

                        // Extract available_agents from metadata before forwarding
                        available_agents = notification
                            .meta
                            .as_ref()
                            .and_then(|meta| meta.get("available_agents"))
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|agent_arr| {
                                        let agent_tuple = agent_arr.as_array()?;
                                        let id = agent_tuple.first()?.as_str()?.to_string();
                                        let name = agent_tuple.get(1)?.as_str()?.to_string();
                                        let description = agent_tuple.get(2).and_then(|v| {
                                            if v.is_null() {
                                                None
                                            } else {
                                                v.as_str().map(|s| s.to_string())
                                            }
                                        });
                                        Some((id, name, description))
                                    })
                                    .collect::<Vec<_>>()
                            });

                        // Extract current_agent from metadata if present
                        current_agent = notification
                            .meta
                            .as_ref()
                            .and_then(|meta| meta.get("current_agent"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        // Forward the notification
                        if let Some(sender) = &self.notification_sender {
                            tracing::info!(
                                "Forwarding AvailableCommandsUpdate from Claude init message"
                            );
                            if let Err(e) = sender.send_update(notification).await {
                                tracing::warn!(
                                    "Failed to send init notification (expected in tests): {}",
                                    e
                                );
                            }
                        } else {
                            tracing::warn!("No notification sender configured - cannot forward init notification");
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("Init message produced no notification - check protocol_translator parsing");
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse init message: {}", e);
                    }
                }

                // Consume remaining response lines (assistant message, result)
                // CRITICAL: Must consume the complete response including final "result" message.
                // If we don't, the init trigger response leaks into real prompt history.
                tracing::debug!("Consuming remaining init response lines until result message");
                let mut lines_consumed = 0;
                let max_wait_seconds = 30;
                let start_time = std::time::Instant::now();

                loop {
                    if start_time.elapsed().as_secs() > max_wait_seconds {
                        tracing::error!(
                            "Timeout after {}s waiting for init result (consumed {} lines)",
                            max_wait_seconds,
                            lines_consumed
                        );
                        break;
                    }

                    let remaining =
                        tokio::time::timeout(std::time::Duration::from_secs(5), async {
                            let mut proc = process.lock().await;
                            proc.read_line().await
                        })
                        .await;

                    match remaining {
                        Ok(Ok(Some(line))) => {
                            lines_consumed += 1;
                            tracing::debug!(
                                "Init response line {}: {} bytes",
                                lines_consumed,
                                line.len()
                            );

                            // Record to raw transcript but don't forward as notification
                            if let Some(ref manager) = self.raw_message_manager {
                                manager.record(line.clone());
                            }

                            // Check if this is the result line (end of response)
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                let msg_type = json.get("type").and_then(|t| t.as_str());
                                tracing::debug!("Init response type: {:?}", msg_type);
                                if msg_type == Some("result") {
                                    tracing::info!(
                                        "✅ Consumed complete init response ({} lines)",
                                        lines_consumed
                                    );
                                    break;
                                }
                            }
                        }
                        Ok(Ok(None)) => {
                            tracing::warn!(
                                "EOF while consuming init after {} lines",
                                lines_consumed
                            );
                            break;
                        }
                        Ok(Err(e)) => {
                            tracing::error!(
                                "Error reading init line after {}: {}",
                                lines_consumed,
                                e
                            );
                            break;
                        }
                        Err(_) => {
                            tracing::debug!(
                                "Read timeout after {} lines (continuing...)",
                                lines_consumed
                            );
                            // Continue - don't give up on single timeout
                        }
                    }
                }
            }
            Ok(Ok(None)) => {
                tracing::error!("Claude process closed before sending init message");
            }
            Ok(Err(e)) => {
                tracing::error!("Error reading init message: {}", e);
            }
            Err(_) => {
                tracing::error!("Timeout waiting for init message from Claude CLI");
            }
        }

        Ok((available_agents, current_agent))
    }
}

/// Session context for managing conversation history
pub struct SessionContext {
    pub session_id: SessionId,
    pub messages: Vec<ClaudeMessage>,
    pub created_at: SystemTime,
    /// Working directory for this session
    pub cwd: std::path::PathBuf,
    /// Total cost in USD for all messages in this session
    pub total_cost_usd: f64,
    /// Total input tokens used across all messages
    pub total_input_tokens: u64,
    /// Total output tokens used across all messages
    pub total_output_tokens: u64,
}

/// Individual message in a conversation
#[derive(Debug, Clone)]
pub struct ClaudeMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
}

/// Streaming message chunk
#[derive(Debug, Clone)]
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
    /// Tool call information (only present when chunk_type is ToolCall)
    pub tool_call: Option<ToolCallInfo>,
    /// Token usage information (only present in Result messages)
    pub token_usage: Option<TokenUsageInfo>,
    /// Stop reason from Claude (only present in final chunk from result message)
    pub stop_reason: Option<String>,
}

/// Tool call information extracted from Message::Tool
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub parameters: serde_json::Value,
}

/// Token usage information extracted from Message metadata
#[derive(Debug, Clone)]
pub struct TokenUsageInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Types of message chunks in streaming responses
#[derive(Debug, Clone)]
pub enum ChunkType {
    Text,
    ToolCall,
    ToolResult,
}

impl ClaudeClient {
    /// Create a new Claude client with protocol translator
    pub fn new(protocol_translator: Arc<ProtocolTranslator>) -> Result<Self> {
        Ok(Self {
            process_manager: Arc::new(ClaudeProcessManager::new()),
            protocol_translator,
            notification_sender: None,
            raw_message_manager: None,
        })
    }

    /// Create a new Claude client with custom configuration and protocol translator
    pub fn new_with_config(
        _claude_config: &ClaudeConfig,
        protocol_translator: Arc<ProtocolTranslator>,
    ) -> Result<Self> {
        tracing::info!("Created ClaudeClient with process manager");
        Ok(Self {
            process_manager: Arc::new(ClaudeProcessManager::new()),
            protocol_translator,
            notification_sender: None,
            raw_message_manager: None,
        })
    }

    /// Check if the client supports streaming
    pub fn supports_streaming(&self) -> bool {
        true
    }

    /// Get the process manager (for session lifecycle integration)
    pub fn process_manager(&self) -> &Arc<ClaudeProcessManager> {
        &self.process_manager
    }

    /// Convert session::SessionId to agent_client_protocol::SessionId
    fn to_acp_session_id(session_id: &SessionId) -> agent_client_protocol::SessionId {
        agent_client_protocol::SessionId::new(session_id.to_string())
    }

    /// Convert ContentBlock to MessageChunk
    fn content_block_to_message_chunk(content: ContentBlock) -> MessageChunk {
        match content {
            ContentBlock::Text(text) => MessageChunk {
                content: text.text,
                chunk_type: ChunkType::Text,
                tool_call: None,
                token_usage: None,
                stop_reason: None,
            },
            // Handle other ContentBlock variants if they exist
            _ => MessageChunk {
                content: String::new(),
                chunk_type: ChunkType::Text,
                tool_call: None,
                token_usage: None,
                stop_reason: None,
            },
        }
    }

    /// Helper method to send prompt to process
    async fn send_prompt_to_process(
        &self,
        process: Arc<Mutex<ClaudeProcess>>,
        prompt: &str,
    ) -> Result<()> {
        let text_content = TextContent::new(prompt.to_string());
        let content = vec![ContentBlock::Text(text_content)];
        let stream_json = self.protocol_translator.acp_to_stream_json(content)?;

        // Debug logging to trace what's actually sent to Claude CLI
        tracing::debug!("📤 Sending to Claude CLI stdin:");
        tracing::debug!("  Prompt: {} chars", prompt.len());
        tracing::debug!("  Stream-JSON: {} chars", stream_json.len());
        tracing::debug!(
            "  Preview: {}",
            if stream_json.len() > 300 {
                format!("{}...", &stream_json[..300])
            } else {
                stream_json.clone()
            }
        );

        let mut proc = process.lock().await;
        proc.write_line(&stream_json).await?;
        Ok(())
    }

    /// Helper method to check if a line indicates end of stream
    fn is_end_of_stream(line: &str) -> bool {
        // Parse JSON and check type field properly
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                return msg_type == "result";
            }
        }
        false
    }

    /// Execute a simple query without session context
    pub async fn query(
        &self,
        prompt: &str,
        session_id: &SessionId,
        cwd: &std::path::Path,
        agent_mode: Option<String>,
    ) -> Result<String> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        // Get the process for this session (will spawn with --agent flag if mode specified)
        // MCP servers are only configured during session creation, not on subsequent prompts
        let process = self
            .process_manager
            .get_process(session_id, cwd, agent_mode, vec![])
            .await?;

        // Send prompt to process
        self.send_prompt_to_process(process.clone(), prompt).await?;

        // Read response lines until we get a result
        let mut response_text = String::new();
        let acp_session_id = Self::to_acp_session_id(session_id);

        // Track whether we've seen stream_event chunks to enable duplicate filtering
        let mut saw_stream_events = false;
        let mut accumulated_text = String::new();

        loop {
            let line = {
                let mut proc = process.lock().await;
                proc.read_line().await?
            };

            match line {
                Some(line) => {
                    // Check if this is a stream_event to enable duplicate filtering
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        if json.get("type").and_then(|t| t.as_str()) == Some("stream_event") {
                            saw_stream_events = true;
                        }
                    }

                    if let Ok(Some(notification)) = self
                        .protocol_translator
                        .stream_json_to_acp(&line, &acp_session_id)
                        .await
                    {
                        if let SessionUpdate::AgentMessageChunk(chunk) = notification.update {
                            if let ContentBlock::Text(text) = chunk.content {
                                // If we've seen stream_events and this chunk's text equals
                                // the accumulated text, it's the duplicate assistant message - skip it
                                if saw_stream_events && text.text == accumulated_text {
                                    tracing::debug!(
                                        "Filtered duplicate assistant message ({} chars) in non-streaming query",
                                        text.text.len()
                                    );
                                    continue;
                                }

                                // Accumulate text from stream_event chunks
                                if saw_stream_events {
                                    accumulated_text.push_str(&text.text);
                                }

                                response_text.push_str(&text.text);
                            }
                        }
                    }
                    // Check if this is a result message (indicates end)
                    if Self::is_end_of_stream(&line) {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(response_text)
    }

    /// Execute a streaming query without session context
    pub async fn query_stream(
        &self,
        prompt: &str,
        session_id: &SessionId,
        cwd: &std::path::Path,
        agent_mode: Option<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = MessageChunk> + Send>>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        // Get the process for this session (will spawn with --agent flag if mode specified)
        // MCP servers are only configured during session creation, not on subsequent prompts
        let process = self
            .process_manager
            .get_process(session_id, cwd, agent_mode, vec![])
            .await?;

        // Send prompt to process
        self.send_prompt_to_process(process.clone(), prompt).await?;

        // Create a channel-based stream to avoid holding mutex across await
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let process_clone = process.clone();
        let acp_session_id = Self::to_acp_session_id(session_id);
        let notification_sender_clone = self.notification_sender.clone();
        let raw_message_manager_clone = self.raw_message_manager.clone();
        let protocol_translator = self.protocol_translator.clone();

        // Spawn an async task to read from the process and send chunks
        tokio::task::spawn(async move {
            // Track whether we've seen any stream_event chunks during this streaming session.
            // If true, we need to filter duplicate assistant messages that contain the full text.
            let mut saw_stream_events = false;

            // Accumulates text content from stream_event chunks to detect duplicate assistant messages.
            // When an assistant message text matches this accumulated text, it's a duplicate.
            let mut accumulated_text = String::new();

            loop {
                let line = {
                    let mut proc = process_clone.lock().await;
                    match proc.read_line().await {
                        Ok(Some(line)) => line,
                        Ok(None) => break,
                        Err(_) => break,
                    }
                };

                // Record raw JSON-RPC message
                if let Some(ref manager) = raw_message_manager_clone {
                    manager.record(line.clone());
                }

                // DEBUG: Capture ALL stdout to debug file
                {
                    use std::io::Write;
                    let debug_file = std::path::PathBuf::from("/tmp/claude_stdout_debug.jsonl");
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&debug_file)
                    {
                        let _ = writeln!(file, "{}", line);
                    }
                }

                // Check if this is a result message (indicates end)
                if Self::is_end_of_stream(&line) {
                    // Parse the result message to extract stop_reason
                    if let Ok(Some(result)) = protocol_translator.parse_result_message(&line) {
                        // Send a final chunk with the stop_reason
                        let final_chunk = MessageChunk {
                            content: String::new(),
                            chunk_type: ChunkType::Text,
                            tool_call: None,
                            token_usage: None,
                            stop_reason: result.stop_reason,
                        };
                        let _ = tx.send(final_chunk);
                    }
                    break;
                }

                // Check if this is a stream_event to enable duplicate filtering.
                // When --include-partial-messages is used, claude CLI sends:
                // 1. Real-time stream_event chunks (which we want)
                // 2. A final assistant message with full text (which duplicates #1)
                // We track stream_events to know when to filter the duplicate.
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    if json.get("type").and_then(|t| t.as_str()) == Some("stream_event") {
                        saw_stream_events = true;
                    }
                }

                // Translate to ACP notification
                if let Ok(Some(notification)) = protocol_translator
                    .stream_json_to_acp(&line, &acp_session_id)
                    .await
                {
                    // Forward notification first (if sender configured)
                    // This must clone to avoid moving from borrowed content
                    if let Some(ref sender) = notification_sender_clone {
                        let _ = sender.send_update(notification.clone()).await;
                    }

                    // Then convert to chunks for conversation flow
                    match notification.update {
                        SessionUpdate::AgentMessageChunk(content_chunk) => {
                            let chunk =
                                Self::content_block_to_message_chunk(content_chunk.content.clone());

                            // Track accumulated text to detect duplicate assistant messages
                            if let ContentBlock::Text(text) = &content_chunk.content {
                                // If we've seen stream_events and this chunk's text equals
                                // the accumulated text, it's the duplicate assistant message - skip it
                                if saw_stream_events && text.text == accumulated_text {
                                    tracing::debug!(
                                        "Filtered duplicate assistant message ({} chars) after stream_events",
                                        text.text.len()
                                    );
                                    continue;
                                }

                                // Accumulate text from stream_event chunks
                                if saw_stream_events {
                                    accumulated_text.push_str(&text.text);
                                }
                            }

                            if tx.send(chunk).is_err() {
                                break;
                            }
                        }
                        SessionUpdate::ToolCall(tool_call) => {
                            // Send ToolCall as a special MessageChunk with tool_call info
                            let tool_chunk = MessageChunk {
                                content: String::new(),
                                chunk_type: ChunkType::ToolCall,
                                tool_call: Some(ToolCallInfo {
                                    id: tool_call.tool_call_id.0.to_string(),
                                    name: tool_call.title.clone(),
                                    parameters: tool_call
                                        .raw_input
                                        .unwrap_or_else(|| serde_json::json!({})),
                                }),
                                token_usage: None,
                                stop_reason: None,
                            };
                            if tx.send(tool_chunk).is_err() {
                                break;
                            }
                        }
                        SessionUpdate::ToolCallUpdate(_update) => {
                            // ToolCallUpdate notifications were already forwarded above
                            tracing::debug!("ToolCallUpdate notification forwarded");
                        }
                        _ => {
                            // Other notification types forwarded above, no chunk conversion
                        }
                    }
                }
            }
        });

        // Convert receiver to stream
        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    /// Execute a query with full session context
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
        agent_mode: Option<String>,
    ) -> Result<String> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        // Build conversation history from context
        let mut full_conversation = String::new();

        for message in &context.messages {
            let role_str = match message.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
            };
            full_conversation.push_str(&format!("{}: {}\n", role_str, message.content));
        }
        full_conversation.push_str(&format!("User: {}", prompt));

        // Use the process manager for the query
        tracing::info!(
            "Sending request to Claude process (prompt length: {} chars)",
            full_conversation.len()
        );

        let response = self
            .query(
                &full_conversation,
                &context.session_id,
                &context.cwd,
                agent_mode,
            )
            .await?;

        tracing::info!(
            "Received response from Claude process (content length: {} chars)",
            response.len()
        );

        Ok(response)
    }

    /// Execute a streaming query with full session context
    pub async fn query_stream_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
        agent_mode: Option<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = MessageChunk> + Send>>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        // Claude CLI maintains conversation state internally, so we just send
        // the new prompt without rebuilding the full conversation history.
        // The process manager ensures we're using the same CLI process for this
        // session, which maintains context across calls.
        self.query_stream(prompt, &context.session_id, &context.cwd, agent_mode)
            .await
    }
}

impl SessionContext {
    /// Create a new session context
    pub fn new(session_id: SessionId, cwd: std::path::PathBuf) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            created_at: SystemTime::now(),
            cwd,
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let message = ClaudeMessage {
            role,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }

    /// Get total tokens used (input + output)
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Get the average cost per message (if any messages have been added)
    pub fn average_cost_per_message(&self) -> Option<f64> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.total_cost_usd / self.messages.len() as f64)
        }
    }
}

/// Convert from session module Session to claude module SessionContext
impl From<crate::session::Session> for SessionContext {
    fn from(session: crate::session::Session) -> Self {
        Self {
            session_id: session.id,
            messages: session.context.into_iter().map(|msg| msg.into()).collect(),
            created_at: session.created_at,
            cwd: session.cwd,
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }
}

/// Convert from session module Session reference to claude module SessionContext
impl From<&crate::session::Session> for SessionContext {
    fn from(session: &crate::session::Session) -> Self {
        Self {
            session_id: session.id,
            messages: session.context.iter().map(|msg| msg.into()).collect(),
            created_at: session.created_at,
            cwd: session.cwd.clone(),
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }
}

/// Convert session Message (with ACP SessionUpdate) to ClaudeMessage for LLM context
/// Only text chunks are converted - tool calls, thoughts, plans are session metadata
impl From<crate::session::Message> for ClaudeMessage {
    fn from(message: crate::session::Message) -> Self {
        match message.update {
            SessionUpdate::UserMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    Self {
                        role: MessageRole::User,
                        content: text.text,
                        timestamp: message.timestamp,
                    }
                } else {
                    // Non-text content doesn't go to LLM
                    Self {
                        role: MessageRole::System,
                        content: String::new(),
                        timestamp: message.timestamp,
                    }
                }
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    Self {
                        role: MessageRole::Assistant,
                        content: text.text,
                        timestamp: message.timestamp,
                    }
                } else {
                    // Non-text content doesn't go to LLM
                    Self {
                        role: MessageRole::System,
                        content: String::new(),
                        timestamp: message.timestamp,
                    }
                }
            }
            // Tool calls, thoughts, plans, etc. are session metadata, not LLM context
            _ => Self {
                role: MessageRole::System,
                content: String::new(),
                timestamp: message.timestamp,
            },
        }
    }
}

/// Convert session Message reference to ClaudeMessage for LLM context
impl From<&crate::session::Message> for ClaudeMessage {
    fn from(message: &crate::session::Message) -> Self {
        match &message.update {
            SessionUpdate::UserMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = &chunk.content {
                    Self {
                        role: MessageRole::User,
                        content: text.text.clone(),
                        timestamp: message.timestamp,
                    }
                } else {
                    // Non-text content doesn't go to LLM
                    Self {
                        role: MessageRole::System,
                        content: String::new(),
                        timestamp: message.timestamp,
                    }
                }
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = &chunk.content {
                    Self {
                        role: MessageRole::Assistant,
                        content: text.text.clone(),
                        timestamp: message.timestamp,
                    }
                } else {
                    // Non-text content doesn't go to LLM
                    Self {
                        role: MessageRole::System,
                        content: String::new(),
                        timestamp: message.timestamp,
                    }
                }
            }
            // Tool calls, thoughts, plans, etc. are session metadata, not LLM context
            _ => Self {
                role: MessageRole::System,
                content: String::new(),
                timestamp: message.timestamp,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = ClaudeClient::new().unwrap();
        assert!(client.supports_streaming());
    }

    #[tokio::test]
    async fn test_session_context() {
        let session_id = SessionId::new();
        let cwd = std::path::PathBuf::from("/tmp");
        let mut context = SessionContext::new(session_id, cwd.clone());
        assert_eq!(context.session_id, session_id);
        assert_eq!(context.cwd, cwd);
        assert_eq!(context.messages.len(), 0);

        context.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(context.messages.len(), 1);
        assert!(matches!(context.messages[0].role, MessageRole::User));
        assert_eq!(context.messages[0].content, "Hello");
    }

    #[test]
    fn test_message_roles() {
        let user_msg = ClaudeMessage {
            role: MessageRole::User,
            content: "User message".to_string(),
            timestamp: SystemTime::now(),
        };

        let assistant_msg = ClaudeMessage {
            role: MessageRole::Assistant,
            content: "Assistant message".to_string(),
            timestamp: SystemTime::now(),
        };

        let system_msg = ClaudeMessage {
            role: MessageRole::System,
            content: "System message".to_string(),
            timestamp: SystemTime::now(),
        };

        assert!(matches!(user_msg.role, MessageRole::User));
        assert!(matches!(assistant_msg.role, MessageRole::Assistant));
        assert!(matches!(system_msg.role, MessageRole::System));
    }

    #[test]
    fn test_chunk_types() {
        let text_chunk = MessageChunk {
            content: "text".to_string(),
            chunk_type: ChunkType::Text,
            tool_call: None,
            token_usage: None,
            stop_reason: None,
        };

        let tool_call_chunk = MessageChunk {
            content: "tool_call".to_string(),
            chunk_type: ChunkType::ToolCall,
            tool_call: Some(ToolCallInfo {
                id: "toolu_test123".to_string(),
                name: "test_tool".to_string(),
                parameters: serde_json::json!({"arg": "value"}),
            }),
            token_usage: None,
            stop_reason: None,
        };

        let tool_result_chunk = MessageChunk {
            content: "tool_result".to_string(),
            chunk_type: ChunkType::ToolResult,
            tool_call: None,
            token_usage: None,
            stop_reason: None,
        };

        let result_chunk = MessageChunk {
            content: String::new(),
            chunk_type: ChunkType::Text,
            tool_call: None,
            token_usage: Some(TokenUsageInfo {
                input_tokens: 100,
                output_tokens: 200,
            }),
            stop_reason: None,
        };

        assert!(matches!(text_chunk.chunk_type, ChunkType::Text));
        assert!(matches!(tool_call_chunk.chunk_type, ChunkType::ToolCall));
        assert!(matches!(
            tool_result_chunk.chunk_type,
            ChunkType::ToolResult
        ));
        assert!(tool_call_chunk.tool_call.is_some());
        assert_eq!(
            tool_call_chunk.tool_call.as_ref().unwrap().name,
            "test_tool"
        );
        assert!(result_chunk.token_usage.is_some());
        assert_eq!(result_chunk.token_usage.as_ref().unwrap().input_tokens, 100);
        assert_eq!(
            result_chunk.token_usage.as_ref().unwrap().output_tokens,
            200
        );
    }

    #[test]
    fn test_session_context_token_tracking() {
        let session_id = SessionId::new();
        let cwd = std::path::PathBuf::from("/tmp");
        let mut context = SessionContext::new(session_id, cwd);

        // Initial state - no cost or tokens
        assert_eq!(context.total_cost_usd, 0.0);
        assert_eq!(context.total_input_tokens, 0);
        assert_eq!(context.total_output_tokens, 0);
        assert_eq!(context.total_tokens(), 0);
        assert_eq!(context.average_cost_per_message(), None);

        // Add messages
        context.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(context.messages.len(), 1);

        context.add_message(MessageRole::Assistant, "Response".to_string());
        assert_eq!(context.messages.len(), 2);

        // Note: Token tracking would need to be updated separately via the public fields
        // This test now focuses on basic message addition functionality
    }
}
