//! Claude process wrapper providing session-aware interactions

use agent_client_protocol::{ContentBlock, SessionUpdate, TextContent};
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use swissarmyhammer_common::Pretty;
use tokio::sync::Mutex;

use crate::{
    claude_process::{ClaudeProcess, ClaudeProcessManager, SpawnConfig},
    config::ClaudeConfig,
    error::Result,
    protocol_translator::ProtocolTranslator,
    session::{MessageRole, SessionId},
};

/// Timeout in seconds for reading the initial system/init message from Claude CLI.
const INIT_TIMEOUT_SECS: u64 = 15;

/// Maximum time in seconds to wait for init result message consumption.
const MAX_INIT_WAIT_SECS: u64 = 30;

/// Context for streaming operations from the Claude CLI process.
///
/// This struct bundles together all the resources needed during a streaming
/// session, allowing them to be passed as a single unit to helper methods.
struct StreamContext {
    /// Handle to the Claude CLI process for reading output lines.
    process: Arc<Mutex<ClaudeProcess>>,
    /// ACP session ID for correlating responses with the protocol translator.
    acp_session_id: agent_client_protocol::SessionId,
    /// Optional manager for recording raw JSON-RPC messages for debugging.
    raw_message_manager: Option<crate::agent::RawMessageManager>,
    /// Translator for converting Claude CLI output to ACP protocol messages.
    protocol_translator: Arc<ProtocolTranslator>,
    /// Channel sender for emitting parsed message chunks to the stream consumer.
    tx: tokio::sync::mpsc::UnboundedSender<MessageChunk>,
}

/// State tracked during streaming to detect duplicates and accumulate text.
///
/// This struct maintains the running state needed to properly parse and
/// deduplicate streaming output from the Claude CLI process.
#[derive(Default)]
struct StreamState {
    /// Whether we've seen any stream events (used for duplicate detection).
    saw_stream_events: bool,
    /// Accumulated text for detecting duplicate final messages.
    accumulated_text: String,
    /// Tool call IDs already seen (for deduplicating partial vs final assistant messages).
    seen_tool_call_ids: std::collections::HashSet<String>,
    /// Count of lines read from the process (for debugging/metrics).
    lines_read: u32,
    /// Count of chunks sent to the consumer (for debugging/metrics).
    chunks_sent: u32,
}

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
    /// * `config` - Spawn configuration including session IDs, working directory, and options
    ///
    /// # Returns
    /// Returns (available_agents, current_agent) if present in init message
    pub async fn spawn_process_and_consume_init(
        &self,
        config: SpawnConfig,
    ) -> Result<(
        Option<Vec<(String, String, Option<String>)>>,
        Option<String>,
    )> {
        self.log_spawn_config(&config);
        let acp_session_id = config.acp_session_id.clone();

        let process = self.process_manager.spawn_process(config).await?;
        self.send_init_trigger(&process).await?;

        let init_line = self.read_init_message(&process).await;
        let (available_agents, current_agent) = self
            .process_init_line(init_line, &acp_session_id, &process)
            .await;

        Ok((available_agents, current_agent))
    }

    /// Log spawn configuration details.
    fn log_spawn_config(&self, config: &SpawnConfig) {
        tracing::debug!(
            "Spawning Claude process for session: {} in {} with {} MCP servers, agent_mode={:?}, system_prompt={}, ephemeral={}",
            config.session_id,
            config.cwd.display(),
            config.mcp_servers.len(),
            config.agent_mode,
            config.system_prompt.as_ref().map(|s| format!("{} chars", s.len())).unwrap_or_else(|| "None".to_string()),
            config.ephemeral
        );
    }

    /// Send initialization trigger message to Claude process.
    async fn send_init_trigger(&self, process: &Arc<Mutex<ClaudeProcess>>) -> Result<()> {
        tracing::debug!("Sending init trigger message to Claude");
        let mut proc = process.lock().await;
        let init_trigger = r#"{"type":"user","message":{"role":"user","content":"hi"}}"#;
        if let Err(e) = proc.write_line(init_trigger).await {
            tracing::error!("Failed to write init trigger to Claude: {}", e);
            return Err(e);
        }
        tracing::debug!("Init trigger sent successfully");
        Ok(())
    }

    /// Read the init message from Claude process with timeout.
    async fn read_init_message(
        &self,
        process: &Arc<Mutex<ClaudeProcess>>,
    ) -> std::result::Result<
        std::result::Result<Option<String>, crate::error::AgentError>,
        tokio::time::error::Elapsed,
    > {
        tracing::debug!("Reading system/init message from Claude CLI");
        tokio::time::timeout(std::time::Duration::from_secs(INIT_TIMEOUT_SECS), async {
            let mut proc = process.lock().await;
            proc.read_line().await
        })
        .await
    }

    /// Process the init line and extract agents info.
    async fn process_init_line(
        &self,
        init_line: std::result::Result<
            std::result::Result<Option<String>, crate::error::AgentError>,
            tokio::time::error::Elapsed,
        >,
        acp_session_id: &agent_client_protocol::SessionId,
        process: &Arc<Mutex<ClaudeProcess>>,
    ) -> (
        Option<Vec<(String, String, Option<String>)>>,
        Option<String>,
    ) {
        match init_line {
            Ok(Ok(Some(line))) => {
                self.handle_init_line_received(&line, acp_session_id, process)
                    .await
            }
            Ok(Ok(None)) => {
                tracing::error!("Claude process closed before sending init message");
                (None, None)
            }
            Ok(Err(e)) => {
                tracing::error!("Error reading init message: {}", e);
                (None, None)
            }
            Err(_) => {
                tracing::error!("Timeout waiting for init message from Claude CLI");
                (None, None)
            }
        }
    }

    /// Handle a successfully received init line.
    async fn handle_init_line_received(
        &self,
        line: &str,
        acp_session_id: &agent_client_protocol::SessionId,
        process: &Arc<Mutex<ClaudeProcess>>,
    ) -> (
        Option<Vec<(String, String, Option<String>)>>,
        Option<String>,
    ) {
        tracing::info!("Received init line from Claude CLI ({} bytes)", line.len());

        if let Some(ref manager) = self.raw_message_manager {
            manager.record(line.to_string());
        }

        let (available_agents, current_agent) =
            self.parse_init_notification(line, acp_session_id).await;

        self.consume_remaining_init_response(process).await;

        (available_agents, current_agent)
    }

    /// Parse init notification and extract agents.
    async fn parse_init_notification(
        &self,
        line: &str,
        acp_session_id: &agent_client_protocol::SessionId,
    ) -> (
        Option<Vec<(String, String, Option<String>)>>,
        Option<String>,
    ) {
        match self
            .protocol_translator
            .stream_json_to_acp(line, acp_session_id)
            .await
        {
            Ok(Some(notification)) => {
                tracing::info!("Protocol translator created notification from init message");
                let available_agents = Self::extract_available_agents(&notification);
                let current_agent = Self::extract_current_agent(&notification);
                self.forward_init_notification(notification).await;
                (available_agents, current_agent)
            }
            Ok(None) => {
                tracing::warn!(
                    "Init message produced no notification - check protocol_translator parsing"
                );
                (None, None)
            }
            Err(e) => {
                tracing::error!("Failed to parse init message: {}", e);
                (None, None)
            }
        }
    }

    /// Extract available_agents from notification metadata.
    fn extract_available_agents(
        notification: &agent_client_protocol::SessionNotification,
    ) -> Option<Vec<(String, String, Option<String>)>> {
        notification
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
                    .collect()
            })
    }

    /// Extract current_agent from notification metadata.
    fn extract_current_agent(
        notification: &agent_client_protocol::SessionNotification,
    ) -> Option<String> {
        notification
            .meta
            .as_ref()
            .and_then(|meta| meta.get("current_agent"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Forward the init notification to subscribers.
    async fn forward_init_notification(
        &self,
        notification: agent_client_protocol::SessionNotification,
    ) {
        if let Some(sender) = &self.notification_sender {
            tracing::info!("Forwarding AvailableCommandsUpdate from Claude init message");
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

    /// Consume remaining init response lines until result message.
    async fn consume_remaining_init_response(&self, process: &Arc<Mutex<ClaudeProcess>>) {
        tracing::debug!("Consuming remaining init response lines until result message");
        let mut lines_consumed = 0;
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed().as_secs() > MAX_INIT_WAIT_SECS {
                tracing::error!(
                    "Timeout after {}s waiting for init result (consumed {} lines)",
                    MAX_INIT_WAIT_SECS,
                    lines_consumed
                );
                break;
            }

            match self.read_next_init_line(process).await {
                InitLineResult::Line(line) => {
                    lines_consumed += 1;
                    if self.is_result_message(&line, lines_consumed) {
                        break;
                    }
                }
                InitLineResult::Eof(count) => {
                    tracing::warn!("EOF while consuming init after {} lines", count);
                    break;
                }
                InitLineResult::Error(count, e) => {
                    tracing::error!("Error reading init line after {}: {}", count, e);
                    break;
                }
                InitLineResult::Timeout(count) => {
                    tracing::debug!("Read timeout after {} lines (continuing...)", count);
                }
            }
        }
    }

    /// Read next line during init consumption.
    async fn read_next_init_line(&self, process: &Arc<Mutex<ClaudeProcess>>) -> InitLineResult {
        let remaining = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut proc = process.lock().await;
            proc.read_line().await
        })
        .await;

        match remaining {
            Ok(Ok(Some(line))) => InitLineResult::Line(line),
            Ok(Ok(None)) => InitLineResult::Eof(0),
            Ok(Err(e)) => InitLineResult::Error(0, e),
            Err(_) => InitLineResult::Timeout(0),
        }
    }

    /// Check if line is a result message and log accordingly.
    fn is_result_message(&self, line: &str, lines_consumed: usize) -> bool {
        if let Some(ref manager) = self.raw_message_manager {
            manager.record(line.to_string());
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let msg_type = json.get("type").and_then(|t| t.as_str());
            tracing::trace!("Init response type: {}", Pretty(&msg_type));
            if msg_type == Some("result") {
                tracing::info!(
                    "✅ Consumed complete init response ({} lines)",
                    lines_consumed
                );
                return true;
            }
        }
        false
    }
}

/// Result of reading a line during init consumption.
enum InitLineResult {
    Line(String),
    Eof(usize),
    Error(usize, crate::error::AgentError),
    Timeout(usize),
}

/// Session context for managing conversation history.
///
/// Maintains the full conversation state including messages, token usage,
/// and cost tracking for a single Claude session.
pub struct SessionContext {
    /// Unique identifier for this session.
    pub session_id: SessionId,
    /// Ordered list of messages in the conversation.
    pub messages: Vec<ClaudeMessage>,
    /// When this session was created.
    pub created_at: SystemTime,
    /// Working directory for this session.
    pub cwd: std::path::PathBuf,
    /// Total cost in USD for all messages in this session.
    pub total_cost_usd: f64,
    /// Total input tokens used across all messages.
    pub total_input_tokens: u64,
    /// Total output tokens used across all messages.
    pub total_output_tokens: u64,
}

/// Individual message in a conversation.
///
/// Represents a single message exchange (user input or assistant response)
/// with its role, content, and timestamp.
#[derive(Debug, Clone)]
pub struct ClaudeMessage {
    /// The role of the message sender (user or assistant).
    pub role: MessageRole,
    /// The text content of the message.
    pub content: String,
    /// When this message was created.
    pub timestamp: SystemTime,
}

/// Streaming message chunk from Claude responses.
///
/// Represents a single chunk of data received during streaming,
/// which may contain text, tool calls, or metadata.
#[derive(Debug, Clone)]
pub struct MessageChunk {
    /// The text content of this chunk.
    pub content: String,
    /// The type of content in this chunk.
    pub chunk_type: ChunkType,
    /// Tool call information (only present when chunk_type is ToolCall).
    pub tool_call: Option<ToolCallInfo>,
    /// Token usage information (only present in Result messages).
    pub token_usage: Option<TokenUsageInfo>,
    /// Stop reason from Claude (only present in final chunk from result message).
    pub stop_reason: Option<String>,
}

/// Tool call information extracted from Claude Message::Tool responses.
///
/// Contains the tool call identifier, name, and parameters for execution.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Name of the tool to execute.
    pub name: String,
    /// Parameters to pass to the tool as JSON.
    pub parameters: serde_json::Value,
}

/// Token usage information extracted from Message metadata.
///
/// Tracks the number of tokens consumed for billing and context management.
#[derive(Debug, Clone)]
pub struct TokenUsageInfo {
    /// Number of tokens in the input prompt.
    pub input_tokens: u64,
    /// Number of tokens in the model's output.
    pub output_tokens: u64,
}

/// Types of message chunks in streaming responses.
///
/// Identifies the content type of each chunk received during streaming.
#[derive(Debug, Clone)]
pub enum ChunkType {
    /// Plain text content from the model.
    Text,
    /// A tool call request from the model.
    ToolCall,
    /// Result from a completed tool execution.
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

    /// Convert internal session ID to ACP protocol session ID.
    ///
    /// Maps between the internal `SessionId` type and the protocol-level
    /// `agent_client_protocol::SessionId` for ACP communication.
    fn to_acp_session_id(session_id: &SessionId) -> agent_client_protocol::SessionId {
        agent_client_protocol::SessionId::new(session_id.to_string())
    }

    /// Convert ACP ContentBlock to internal MessageChunk format.
    ///
    /// Transforms protocol-level content blocks into the internal streaming
    /// chunk representation used for response processing.
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

    /// Send a prompt to the Claude process via stream-json protocol.
    ///
    /// Converts the prompt text to the stream-json format and writes it
    /// to the process stdin.
    async fn send_prompt_to_process(
        &self,
        process: Arc<Mutex<ClaudeProcess>>,
        prompt: &str,
    ) -> Result<()> {
        let text_content = TextContent::new(prompt.to_string());
        let content = vec![ContentBlock::Text(text_content)];
        let stream_json = self.protocol_translator.acp_to_stream_json(content)?;

        let mut proc = process.lock().await;
        proc.write_line(&stream_json).await?;
        Ok(())
    }

    /// Check if a JSON line indicates the end of a streaming response.
    ///
    /// Returns true if the line contains a "result" type message,
    /// signaling that the response stream has completed.
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
    pub async fn query(&self, prompt: &str, session_id: &SessionId) -> Result<String> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        let process = self.process_manager.get_process(session_id)?;
        self.send_prompt_to_process(process.clone(), prompt).await?;

        let acp_session_id = Self::to_acp_session_id(session_id);
        self.collect_query_response(process, &acp_session_id).await
    }

    /// Collect response text from a query by processing stream lines.
    async fn collect_query_response(
        &self,
        process: Arc<Mutex<ClaudeProcess>>,
        acp_session_id: &agent_client_protocol::SessionId,
    ) -> Result<String> {
        let mut response_text = String::new();
        let mut saw_stream_events = false;
        let mut accumulated_text = String::new();

        loop {
            let line = Self::read_process_line(&process).await?;

            match line {
                Some(line) => {
                    Self::update_stream_event_flag(&line, &mut saw_stream_events);

                    if let Some(text) = self.extract_text_from_line(&line, acp_session_id).await {
                        if self.should_filter_duplicate(&text, saw_stream_events, &accumulated_text)
                        {
                            continue;
                        }
                        if saw_stream_events {
                            accumulated_text.push_str(&text);
                        }
                        response_text.push_str(&text);
                    }

                    if Self::is_end_of_stream(&line) {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(response_text)
    }

    /// Read a line from the process with proper locking.
    async fn read_process_line(process: &Arc<Mutex<ClaudeProcess>>) -> Result<Option<String>> {
        let mut proc = process.lock().await;
        proc.read_line().await
    }

    /// Extract text content from a stream-json line if present.
    async fn extract_text_from_line(
        &self,
        line: &str,
        acp_session_id: &agent_client_protocol::SessionId,
    ) -> Option<String> {
        let notification = self
            .protocol_translator
            .stream_json_to_acp(line, acp_session_id)
            .await
            .ok()??;

        if let SessionUpdate::AgentMessageChunk(chunk) = notification.update {
            if let ContentBlock::Text(text) = chunk.content {
                return Some(text.text);
            }
        }
        None
    }

    /// Check if a text chunk should be filtered as a duplicate.
    fn should_filter_duplicate(
        &self,
        text: &str,
        saw_stream_events: bool,
        accumulated_text: &str,
    ) -> bool {
        if saw_stream_events && text == accumulated_text {
            tracing::debug!(
                "Filtered duplicate assistant message ({} chars) in non-streaming query",
                text.len()
            );
            true
        } else {
            false
        }
    }

    /// Execute a streaming query without session context
    pub async fn query_stream(
        &self,
        prompt: &str,
        session_id: &SessionId,
    ) -> Result<Pin<Box<dyn Stream<Item = MessageChunk> + Send>>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        let process = self.process_manager.get_process(session_id)?;
        self.send_prompt_to_process(process.clone(), prompt).await?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = StreamContext {
            process: process.clone(),
            acp_session_id: Self::to_acp_session_id(session_id),
            raw_message_manager: self.raw_message_manager.clone(),
            protocol_translator: self.protocol_translator.clone(),
            tx,
        };

        tokio::task::spawn(Self::run_stream_loop(ctx));

        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    /// Run the streaming read loop.
    async fn run_stream_loop(ctx: StreamContext) {
        let mut state = StreamState::default();

        loop {
            let line = match Self::read_stream_line(&ctx.process).await {
                Some(line) => line,
                None => break,
            };
            state.lines_read += 1;

            Self::record_raw_message(&ctx.raw_message_manager, &line);
            Self::write_debug_log(&line);

            if Self::is_end_of_stream(&line) {
                Self::send_final_chunk(&ctx, &line);
                break;
            }

            Self::update_stream_event_flag(&line, &mut state.saw_stream_events);

            let notification = match ctx
                .protocol_translator
                .stream_json_to_acp(&line, &ctx.acp_session_id)
                .await
            {
                Ok(Some(n)) => n,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!("stream_json_to_acp error: {}", e);
                    continue;
                }
            };

            // Don't forward notifications here — process_stream_chunks in
            // agent_prompt_handling.rs sends its own richer notifications (with tool
            // kind, raw_input, session storage) for each MessageChunk it consumes.
            // Forwarding here would cause every notification to appear twice.

            if !Self::process_notification(&ctx, notification, &mut state) {
                break;
            }
        }
    }

    /// Read a line from the stream process.
    async fn read_stream_line(process: &Arc<Mutex<ClaudeProcess>>) -> Option<String> {
        let mut proc = process.lock().await;
        match proc.read_line().await {
            Ok(Some(line)) => Some(line),
            Ok(None) => {
                tracing::debug!("Stream reading loop: EOF");
                None
            }
            Err(e) => {
                tracing::warn!("Stream reading loop error: {}", e);
                None
            }
        }
    }

    /// Record raw message to manager.
    fn record_raw_message(manager: &Option<crate::agent::RawMessageManager>, line: &str) {
        if let Some(ref m) = manager {
            m.record(line.to_string());
        }
    }

    /// Write line to debug log file.
    fn write_debug_log(line: &str) {
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

    /// Send final chunk with stop reason.
    fn send_final_chunk(ctx: &StreamContext, line: &str) {
        if let Ok(Some(result)) = ctx.protocol_translator.parse_result_message(line) {
            let chunk = MessageChunk {
                content: String::new(),
                chunk_type: ChunkType::Text,
                tool_call: None,
                token_usage: None,
                stop_reason: result.stop_reason.clone(),
            };
            tracing::debug!(
                "Sending final chunk with stop_reason={:?}",
                result.stop_reason
            );
            let _ = ctx.tx.send(chunk);
        }
    }

    /// Update stream_event flag for duplicate detection.
    fn update_stream_event_flag(line: &str, saw_stream_events: &mut bool) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if json.get("type").and_then(|t| t.as_str()) == Some("stream_event") {
                *saw_stream_events = true;
            }
        }
    }

    /// Process notification and send chunks. Returns false if loop should break.
    fn process_notification(
        ctx: &StreamContext,
        notification: agent_client_protocol::SessionNotification,
        state: &mut StreamState,
    ) -> bool {
        match notification.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                Self::handle_message_chunk(ctx, chunk, state)
            }
            SessionUpdate::ToolCall(tool_call) => Self::handle_tool_call(ctx, tool_call, state),
            SessionUpdate::ToolCallUpdate(_) => {
                tracing::debug!("ToolCallUpdate notification forwarded");
                true
            }
            _ => true,
        }
    }

    /// Handle agent message chunk. Returns false if channel closed.
    fn handle_message_chunk(
        ctx: &StreamContext,
        content_chunk: agent_client_protocol::ContentChunk,
        state: &mut StreamState,
    ) -> bool {
        let chunk = Self::content_block_to_message_chunk(content_chunk.content.clone());

        if let ContentBlock::Text(text) = &content_chunk.content {
            if state.saw_stream_events && text.text == state.accumulated_text {
                tracing::debug!("Filtered duplicate assistant message");
                return true;
            }
            state.accumulated_text.push_str(&text.text);
        }

        if ctx.tx.send(chunk).is_err() {
            tracing::debug!("Channel closed");
            return false;
        }
        state.chunks_sent += 1;
        true
    }

    /// Handle tool call. Returns false if channel closed.
    fn handle_tool_call(
        ctx: &StreamContext,
        tool_call: agent_client_protocol::ToolCall,
        state: &mut StreamState,
    ) -> bool {
        // Deduplicate: --include-partial-messages causes Claude to emit the same
        // tool_use in both partial and final assistant messages.
        let tool_id = tool_call.tool_call_id.0.to_string();
        if !state.seen_tool_call_ids.insert(tool_id) {
            tracing::debug!("Filtered duplicate tool call: {}", tool_call.title);
            return true;
        }

        let chunk = MessageChunk {
            content: String::new(),
            chunk_type: ChunkType::ToolCall,
            tool_call: Some(ToolCallInfo {
                id: tool_call.tool_call_id.0.to_string(),
                name: tool_call.title.clone(),
                parameters: tool_call.raw_input.unwrap_or_else(|| serde_json::json!({})),
            }),
            token_usage: None,
            stop_reason: None,
        };

        if ctx.tx.send(chunk).is_err() {
            tracing::debug!("Channel closed");
            return false;
        }
        state.chunks_sent += 1;
        true
    }

    /// Execute a query with full session context
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
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

        let response = self.query(&full_conversation, &context.session_id).await?;

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
    ) -> Result<Pin<Box<dyn Stream<Item = MessageChunk> + Send>>> {
        // Claude CLI maintains conversation state internally, so we just send
        // the new prompt without rebuilding the full conversation history.
        self.query_stream(prompt, &context.session_id).await
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
    use tempfile::tempdir;

    fn create_test_translator() -> Arc<ProtocolTranslator> {
        let temp_dir = tempdir().unwrap();
        let storage = crate::permissions::FilePermissionStorage::new(temp_dir.path().to_path_buf());
        let permission_engine = Arc::new(crate::permissions::PermissionPolicyEngine::new(
            Box::new(storage),
        ));
        Arc::new(ProtocolTranslator::new(permission_engine))
    }

    #[tokio::test]
    async fn test_client_creation() {
        let protocol_translator = create_test_translator();
        let client = ClaudeClient::new(protocol_translator).unwrap();
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
