//! TracingAgent - Wrapper that logs all Agent method calls at INFO level
//!
//! Provides unified tracing for all ACP agent implementations.

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
    ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse,
};
use std::collections::HashMap;
use tokio::sync::broadcast;

/// Extract text content from ACP ContentBlocks for logging
fn extract_prompt_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text(text) = block {
                Some(text.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Buffers for accumulating message chunks per session.
/// Chunks are stored with their content block index and source for proper assembly.
struct ChunkBuffer {
    /// List of (content_block_index, is_stream_event, text) tuples in arrival order
    /// is_stream_event=true means this came from Claude's stream_event (real-time chunks)
    /// is_stream_event=false means this came from assistant message (duplicate full text)
    chunks: Vec<(u64, bool, String)>,
    session_id: String,
}

impl ChunkBuffer {
    fn new(session_id: String) -> Self {
        Self {
            chunks: Vec::new(),
            session_id,
        }
    }

    /// Append a chunk with its content block index and source
    fn append(&mut self, index: u64, is_stream_event: bool, text: &str) {
        self.chunks.push((index, is_stream_event, text.to_string()));
    }

    fn flush(&mut self, agent_name: &str) {
        if self.chunks.is_empty() {
            return;
        }

        // Separate stream_event chunks from non-stream_event chunks
        // stream_event chunks are the real-time incremental pieces
        // non-stream_event chunks are typically duplicate full messages
        let mut stream_chunks: HashMap<u64, String> = HashMap::new();
        let mut other_chunks: HashMap<u64, String> = HashMap::new();

        for (index, is_stream_event, text) in &self.chunks {
            if text.is_empty() {
                continue;
            }

            if *is_stream_event {
                stream_chunks.entry(*index).or_default().push_str(text);
            } else {
                // For non-stream chunks, only keep if we don't have stream chunks for this index
                other_chunks.entry(*index).or_default().push_str(text);
            }
        }

        // Prefer stream_event chunks when available (they're the real-time source)
        // Fall back to other chunks only if no stream_event chunks exist for an index
        let mut final_chunks: HashMap<u64, String> = HashMap::new();

        // Collect all indices
        let all_indices: std::collections::HashSet<u64> = stream_chunks
            .keys()
            .chain(other_chunks.keys())
            .copied()
            .collect();

        for index in all_indices {
            if let Some(stream_text) = stream_chunks.get(&index) {
                // Prefer stream_event content
                final_chunks.insert(index, stream_text.clone());
            } else if let Some(other_text) = other_chunks.get(&index) {
                // Fall back to other content only if no stream content exists
                final_chunks.insert(index, other_text.clone());
            }
        }

        // Assemble final text by concatenating content blocks in index order
        let mut indices: Vec<u64> = final_chunks.keys().copied().collect();
        indices.sort();

        let text: String = indices
            .iter()
            .filter_map(|idx| final_chunks.get(idx))
            .cloned()
            .collect();

        if !text.is_empty() {
            tracing::info!(
                "[{}] session={}, AgentMessage ({} chars): {}",
                agent_name,
                self.session_id,
                text.len(),
                text
            );
        }

        self.chunks.clear();
    }
}

/// Log a single notification, with chunk buffering support
/// Returns true if this was a chunk (buffered), false otherwise (logged immediately)
fn log_notification(
    agent_name: &str,
    notification: &SessionNotification,
    buffers: &mut HashMap<String, ChunkBuffer>,
) -> bool {
    let session_id = &notification.session_id;
    let session_key = session_id.to_string();

    match &notification.update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            if let ContentBlock::Text(text) = &chunk.content {
                // Extract content block index and source from notification meta
                let meta = notification.meta.as_ref();
                let content_block_index = meta
                    .and_then(|m| m.get("content_block_index"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let is_stream_event = meta.and_then(|m| m.get("source")).and_then(|v| v.as_str())
                    == Some("stream_event");

                tracing::debug!(
                    "[{}] session={}, AgentMessageChunk (index={}, stream={}, {} chars)",
                    agent_name,
                    session_id,
                    content_block_index,
                    is_stream_event,
                    text.text.len()
                );
                // Buffer the chunk with its content block index and source
                buffers
                    .entry(session_key)
                    .or_insert_with(|| ChunkBuffer::new(session_id.to_string()))
                    .append(content_block_index, is_stream_event, &text.text);
            } else {
                tracing::debug!(
                    "[{}] session={}, AgentMessageChunk (non-text)",
                    agent_name,
                    session_id
                );
            }
            true // was a chunk
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            // Flush any pending message chunks first
            if let Some(buffer) = buffers.get_mut(&session_key) {
                buffer.flush(agent_name);
            }
            if let ContentBlock::Text(text) = &chunk.content {
                tracing::info!(
                    "[{}] session={}, AgentThoughtChunk ({} chars): {}",
                    agent_name,
                    session_id,
                    text.text.len(),
                    text.text
                );
            }
            false
        }
        SessionUpdate::ToolCall(tool_call) => {
            // Flush any pending message chunks first
            if let Some(buffer) = buffers.get_mut(&session_key) {
                buffer.flush(agent_name);
            }
            let input = tool_call
                .raw_input
                .as_ref()
                .map(|v| {
                    let s = v.to_string();
                    if s.len() > 200 {
                        format!("{}...", &s[..200])
                    } else {
                        s
                    }
                })
                .unwrap_or_else(|| "(no input)".to_string());
            tracing::info!(
                "[{}] session={}, ToolCall: {} | input: {}",
                agent_name,
                session_id,
                tool_call.title,
                input
            );
            false
        }
        SessionUpdate::ToolCallUpdate(update) => {
            tracing::debug!(
                "[{}] session={}, ToolCallUpdate: {}",
                agent_name,
                session_id,
                update.tool_call_id
            );
            false
        }
        SessionUpdate::CurrentModeUpdate(mode) => {
            // Flush any pending message chunks first
            if let Some(buffer) = buffers.get_mut(&session_key) {
                buffer.flush(agent_name);
            }
            tracing::info!(
                "[{}] session={}, CurrentModeUpdate: {}",
                agent_name,
                session_id,
                mode.current_mode_id
            );
            false
        }
        SessionUpdate::AvailableCommandsUpdate(update) => {
            // Flush any pending message chunks first
            if let Some(buffer) = buffers.get_mut(&session_key) {
                buffer.flush(agent_name);
            }
            tracing::info!(
                "[{}] session={}, AvailableCommandsUpdate: {} commands",
                agent_name,
                session_id,
                update.available_commands.len()
            );
            false
        }
        SessionUpdate::Plan(plan) => {
            // Flush any pending message chunks first
            if let Some(buffer) = buffers.get_mut(&session_key) {
                buffer.flush(agent_name);
            }
            tracing::info!(
                "[{}] session={}, Plan: {} entries",
                agent_name,
                session_id,
                plan.entries.len()
            );
            false
        }
        _ => {
            tracing::debug!("[{}] session={}, other update type", agent_name, session_id);
            false
        }
    }
}

/// Spawn a task that logs all notifications from the receiver
///
/// Returns a new receiver that can be used by consumers (the original is consumed by the logger).
/// Message chunks are buffered and logged as a single INFO message when a non-chunk notification
/// arrives or when the channel closes.
pub fn trace_notifications(
    agent_name: String,
    receiver: broadcast::Receiver<SessionNotification>,
) -> broadcast::Receiver<SessionNotification> {
    // Create a new channel to forward notifications after logging
    let (tx, rx) = broadcast::channel(256);

    let mut recv = receiver;
    tokio::spawn(async move {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        loop {
            match recv.recv().await {
                Ok(notification) => {
                    log_notification(&agent_name, &notification, &mut buffers);
                    // Forward to consumers (ignore send errors if no receivers)
                    let _ = tx.send(notification);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("[{}] notification receiver lagged by {}", agent_name, n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // Flush any remaining buffered chunks before closing
                    for (_, mut buffer) in buffers.drain() {
                        buffer.flush(&agent_name);
                    }
                    tracing::debug!("[{}] notification channel closed", agent_name);
                    break;
                }
            }
        }
    });

    rx
}

/// TracingAgent wraps any Agent and logs all method calls
///
/// Uses Arc<dyn Agent> internally to work with dynamically dispatched agents.
pub struct TracingAgent {
    inner: std::sync::Arc<dyn Agent + Send + Sync>,
    agent_name: String,
}

impl TracingAgent {
    /// Create a new TracingAgent wrapping the given agent
    pub fn new(
        inner: std::sync::Arc<dyn Agent + Send + Sync>,
        agent_name: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            agent_name: agent_name.into(),
        }
    }

    /// Get the agent name for logging
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// Get reference to inner agent
    pub fn inner(&self) -> &std::sync::Arc<dyn Agent + Send + Sync> {
        &self.inner
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for TracingAgent {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        tracing::info!(
            "[{}] initialize: protocol={:?}",
            self.agent_name,
            request.protocol_version
        );

        let response = self.inner.initialize(request).await?;

        if let Some(ref info) = response.agent_info {
            tracing::info!(
                "[{}] response: agent={}, version={}",
                self.agent_name,
                info.name,
                info.version
            );
        }

        Ok(response)
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        tracing::info!("[{}] authenticate", self.agent_name);
        self.inner.authenticate(request).await
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        tracing::info!(
            "[{}] new_session: cwd={}",
            self.agent_name,
            request.cwd.display()
        );

        let response = self.inner.new_session(request).await?;

        tracing::info!(
            "[{}] response: session_id={}",
            self.agent_name,
            response.session_id
        );

        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let prompt_text = extract_prompt_text(&request.prompt);
        tracing::info!(
            "[{}] prompt ({} chars): {}",
            self.agent_name,
            prompt_text.len(),
            prompt_text
        );

        let response = self.inner.prompt(request).await?;

        tracing::info!(
            "[{}] response: stop_reason={:?}",
            self.agent_name,
            response.stop_reason
        );

        Ok(response)
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        tracing::info!(
            "[{}] cancel: session_id={}",
            self.agent_name,
            request.session_id
        );
        self.inner.cancel(request).await
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        tracing::info!(
            "[{}] load_session: session_id={}",
            self.agent_name,
            request.session_id
        );

        let response = self.inner.load_session(request).await?;

        tracing::info!("[{}] response: session loaded", self.agent_name);

        Ok(response)
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        tracing::info!(
            "[{}] set_session_mode: session={}, mode={}",
            self.agent_name,
            request.session_id,
            request.mode_id
        );

        let response = self.inner.set_session_mode(request).await?;

        tracing::info!("[{}] response: mode set", self.agent_name);

        Ok(response)
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        tracing::info!("[{}] ext_method", self.agent_name);

        let response = self.inner.ext_method(request).await?;

        tracing::info!("[{}] response: ext_method complete", self.agent_name);

        Ok(response)
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        tracing::info!("[{}] ext_notification", self.agent_name);
        self.inner.ext_notification(notification).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        AuthenticateRequest, AuthenticateResponse, AvailableCommandsUpdate, CancelNotification,
        ContentChunk, CurrentModeUpdate, ExtNotification, ExtRequest, ExtResponse, Implementation,
        InitializeRequest, InitializeResponse, LoadSessionRequest, LoadSessionResponse,
        NewSessionRequest, NewSessionResponse, Plan, PromptRequest, PromptResponse, SessionId,
        SetSessionModeRequest, SetSessionModeResponse, StopReason, TextContent, ToolCall,
        ToolCallUpdate, ToolCallUpdateFields,
    };
    use serde_json::value::RawValue;
    use std::sync::Arc;

    // -- Mock agent for TracingAgent tests --

    struct MockAgent;

    #[async_trait::async_trait(?Send)]
    impl Agent for MockAgent {
        async fn initialize(
            &self,
            _request: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            Ok(
                InitializeResponse::new(agent_client_protocol::ProtocolVersion::LATEST)
                    .agent_info(Implementation::new("test-agent", "1.0.0")),
            )
        }

        async fn authenticate(
            &self,
            _request: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }

        async fn new_session(
            &self,
            _request: NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("test-session"))
        }

        async fn prompt(
            &self,
            _request: PromptRequest,
        ) -> agent_client_protocol::Result<PromptResponse> {
            Ok(PromptResponse::new(StopReason::EndTurn))
        }

        async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }

        async fn load_session(
            &self,
            _request: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }

        async fn set_session_mode(
            &self,
            _request: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }

        async fn ext_method(
            &self,
            _request: ExtRequest,
        ) -> agent_client_protocol::Result<ExtResponse> {
            let raw = RawValue::from_string("null".to_string()).unwrap();
            Ok(ExtResponse::new(Arc::from(raw)))
        }

        async fn ext_notification(
            &self,
            _notification: ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    fn make_ext_request() -> ExtRequest {
        let raw = RawValue::from_string("{}".to_string()).unwrap();
        ExtRequest::new("custom/method", Arc::from(raw))
    }

    fn make_ext_notification() -> ExtNotification {
        let raw = RawValue::from_string("{}".to_string()).unwrap();
        ExtNotification::new("custom/notify", Arc::from(raw))
    }

    // -- extract_prompt_text tests --

    #[test]
    fn test_extract_prompt_text_from_text_blocks() {
        let content = vec![
            ContentBlock::Text(TextContent::new("Hello")),
            ContentBlock::Text(TextContent::new("World")),
        ];
        assert_eq!(extract_prompt_text(&content), "Hello\nWorld");
    }

    #[test]
    fn test_extract_prompt_text_empty() {
        let content: Vec<ContentBlock> = vec![];
        assert_eq!(extract_prompt_text(&content), "");
    }

    #[test]
    fn test_extract_prompt_text_skips_non_text_blocks() {
        let content = vec![ContentBlock::Text(TextContent::new("only text"))];
        assert_eq!(extract_prompt_text(&content), "only text");
    }

    // -- ChunkBuffer tests --

    #[test]
    fn test_chunk_buffer_new() {
        let buffer = ChunkBuffer::new("sess-1".to_string());
        assert_eq!(buffer.session_id, "sess-1");
        assert!(buffer.chunks.is_empty());
    }

    #[test]
    fn test_chunk_buffer_append() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.append(0, true, "hello ");
        buffer.append(0, true, "world");
        assert_eq!(buffer.chunks.len(), 2);
    }

    #[test]
    fn test_chunk_buffer_flush_concatenates_stream_chunks() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.append(0, true, "hello ");
        buffer.append(0, true, "world");
        buffer.flush("test-agent");
        assert!(buffer.chunks.is_empty(), "flush should clear chunks");
    }

    #[test]
    fn test_chunk_buffer_flush_empty_is_noop() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.flush("test-agent");
        assert!(buffer.chunks.is_empty());
    }

    #[test]
    fn test_chunk_buffer_prefers_stream_chunks_over_non_stream() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.append(0, true, "stream text");
        buffer.append(0, false, "non-stream text");
        buffer.flush("test-agent");
        assert!(buffer.chunks.is_empty());
    }

    #[test]
    fn test_chunk_buffer_falls_back_to_non_stream_when_no_stream() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.append(0, false, "non-stream only");
        buffer.flush("test-agent");
        assert!(buffer.chunks.is_empty());
    }

    #[test]
    fn test_chunk_buffer_multiple_content_block_indices() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.append(0, true, "first block ");
        buffer.append(1, true, "second block");
        buffer.flush("test-agent");
        assert!(buffer.chunks.is_empty());
    }

    #[test]
    fn test_chunk_buffer_skips_empty_text() {
        let mut buffer = ChunkBuffer::new("sess-1".to_string());
        buffer.append(0, true, "");
        buffer.append(0, true, "actual text");
        buffer.flush("test-agent");
        assert!(buffer.chunks.is_empty());
    }

    // -- log_notification tests --

    #[test]
    fn test_log_notification_agent_message_chunk_buffers() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();
        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hello"),
            ))),
        );

        let was_chunk = log_notification("test", &notification, &mut buffers);
        assert!(was_chunk, "AgentMessageChunk should be treated as chunk");
        assert!(buffers.contains_key("sess-1"));
    }

    #[test]
    fn test_log_notification_agent_thought_chunk_not_chunk() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();
        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("thought"),
            ))),
        );

        let was_chunk = log_notification("test", &notification, &mut buffers);
        assert!(
            !was_chunk,
            "AgentThoughtChunk should not be treated as chunk"
        );
    }

    #[test]
    fn test_log_notification_agent_thought_chunk_flushes_buffer() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let msg_notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("buffered"),
            ))),
        );
        log_notification("test", &msg_notif, &mut buffers);
        assert!(!buffers["sess-1"].chunks.is_empty());

        let thought_notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("thinking"),
            ))),
        );
        log_notification("test", &thought_notif, &mut buffers);
        assert!(
            buffers["sess-1"].chunks.is_empty(),
            "Buffer should be flushed"
        );
    }

    #[test]
    fn test_log_notification_tool_call_flushes_and_logs() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let msg_notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("buffered"),
            ))),
        );
        log_notification("test", &msg_notif, &mut buffers);

        let tool_call = ToolCall::new("call-1", "Bash");
        let notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::ToolCall(tool_call),
        );
        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
        assert!(buffers["sess-1"].chunks.is_empty());
    }

    #[test]
    fn test_log_notification_tool_call_with_raw_input() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let mut tool_call = ToolCall::new("call-1", "Edit");
        tool_call.raw_input = Some(serde_json::json!({"path": "/tmp/file.txt"}));
        let notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::ToolCall(tool_call),
        );

        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
    }

    #[test]
    fn test_log_notification_tool_call_with_long_input_truncated() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let long_input = "x".repeat(300);
        let mut tool_call = ToolCall::new("call-1", "Read");
        tool_call.raw_input = Some(serde_json::Value::String(long_input));
        let notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::ToolCall(tool_call),
        );

        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
    }

    #[test]
    fn test_log_notification_tool_call_update() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let update = ToolCallUpdate::new("call-1", ToolCallUpdateFields::new());
        let notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::ToolCallUpdate(update),
        );

        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
    }

    #[test]
    fn test_log_notification_current_mode_update_flushes() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let msg = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("buf"),
            ))),
        );
        log_notification("test", &msg, &mut buffers);

        let mode = CurrentModeUpdate::new("plan");
        let notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::CurrentModeUpdate(mode),
        );

        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
        assert!(buffers["sess-1"].chunks.is_empty());
    }

    #[test]
    fn test_log_notification_available_commands_update() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let update = AvailableCommandsUpdate::new(vec![]);
        let notif = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AvailableCommandsUpdate(update),
        );

        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
    }

    #[test]
    fn test_log_notification_plan_update() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let plan = Plan::new(vec![]);
        let notif = SessionNotification::new(SessionId::from("sess-1"), SessionUpdate::Plan(plan));

        let was_chunk = log_notification("test", &notif, &mut buffers);
        assert!(!was_chunk);
    }

    #[test]
    fn test_log_notification_with_meta_content_block_index() {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        let mut meta = serde_json::Map::new();
        meta.insert(
            "content_block_index".to_string(),
            serde_json::Value::Number(2.into()),
        );
        meta.insert(
            "source".to_string(),
            serde_json::Value::String("stream_event".to_string()),
        );

        let mut notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("chunk with meta"),
            ))),
        );
        notification.meta = Some(meta);

        let was_chunk = log_notification("test", &notification, &mut buffers);
        assert!(was_chunk);

        let buf = buffers.get("sess-1").unwrap();
        assert_eq!(buf.chunks.len(), 1);
        assert_eq!(buf.chunks[0].0, 2);
        assert!(buf.chunks[0].1);
    }

    // -- TracingAgent tests --

    #[test]
    fn test_tracing_agent_new_and_accessors() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock.clone(), "my-agent");

        assert_eq!(agent.agent_name(), "my-agent");
        let _ = agent.inner();
    }

    #[tokio::test]
    async fn test_tracing_agent_initialize_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let response = agent
            .initialize(InitializeRequest::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
            .await
            .unwrap();

        assert!(response.agent_info.is_some());
        assert_eq!(response.agent_info.unwrap().name, "test-agent");
    }

    #[tokio::test]
    async fn test_tracing_agent_authenticate_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let _response = agent
            .authenticate(AuthenticateRequest::new("test-method"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_tracing_agent_new_session_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let response = agent
            .new_session(NewSessionRequest::new("/tmp"))
            .await
            .unwrap();

        assert_eq!(response.session_id.to_string(), "test-session");
    }

    #[tokio::test]
    async fn test_tracing_agent_prompt_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let request = PromptRequest::new(
            SessionId::from("sess-1"),
            vec![ContentBlock::Text(TextContent::new("hello world"))],
        );
        let response = agent.prompt(request).await.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
    }

    #[tokio::test]
    async fn test_tracing_agent_cancel_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        agent
            .cancel(CancelNotification::new("sess-1"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_tracing_agent_load_session_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let _response = agent
            .load_session(LoadSessionRequest::new("sess-1", "/tmp"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_tracing_agent_set_session_mode_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let _response = agent
            .set_session_mode(SetSessionModeRequest::new("sess-1", "plan"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_tracing_agent_ext_method_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        let _response = agent.ext_method(make_ext_request()).await.unwrap();
    }

    #[tokio::test]
    async fn test_tracing_agent_ext_notification_delegates() {
        let mock = Arc::new(MockAgent);
        let agent = TracingAgent::new(mock, "test");

        agent
            .ext_notification(make_ext_notification())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_tracing_agent_initialize_without_agent_info() {
        struct NoInfoAgent;

        #[async_trait::async_trait(?Send)]
        impl Agent for NoInfoAgent {
            async fn initialize(
                &self,
                _r: InitializeRequest,
            ) -> agent_client_protocol::Result<InitializeResponse> {
                Ok(InitializeResponse::new(
                    agent_client_protocol::ProtocolVersion::LATEST,
                ))
            }
            async fn authenticate(
                &self,
                _r: AuthenticateRequest,
            ) -> agent_client_protocol::Result<AuthenticateResponse> {
                Ok(AuthenticateResponse::new())
            }
            async fn new_session(
                &self,
                _r: NewSessionRequest,
            ) -> agent_client_protocol::Result<NewSessionResponse> {
                Ok(NewSessionResponse::new("s1"))
            }
            async fn prompt(
                &self,
                _r: PromptRequest,
            ) -> agent_client_protocol::Result<PromptResponse> {
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
            async fn cancel(&self, _r: CancelNotification) -> agent_client_protocol::Result<()> {
                Ok(())
            }
            async fn load_session(
                &self,
                _r: LoadSessionRequest,
            ) -> agent_client_protocol::Result<LoadSessionResponse> {
                Ok(LoadSessionResponse::new())
            }
            async fn set_session_mode(
                &self,
                _r: SetSessionModeRequest,
            ) -> agent_client_protocol::Result<SetSessionModeResponse> {
                Ok(SetSessionModeResponse::new())
            }
            async fn ext_method(
                &self,
                _r: ExtRequest,
            ) -> agent_client_protocol::Result<ExtResponse> {
                let raw = RawValue::from_string("null".to_string()).unwrap();
                Ok(ExtResponse::new(Arc::from(raw)))
            }
            async fn ext_notification(
                &self,
                _n: ExtNotification,
            ) -> agent_client_protocol::Result<()> {
                Ok(())
            }
        }

        let agent = TracingAgent::new(Arc::new(NoInfoAgent), "test");
        let response = agent
            .initialize(InitializeRequest::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
            .await
            .unwrap();
        assert!(response.agent_info.is_none());
    }

    // -- trace_notifications tests --

    #[tokio::test]
    async fn test_trace_notifications_forwards_messages() {
        let (tx, rx) = broadcast::channel(16);
        let mut forwarded_rx = trace_notifications("test-agent".to_string(), rx);

        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hello"),
            ))),
        );

        tx.send(notification).unwrap();

        let received =
            tokio::time::timeout(tokio::time::Duration::from_millis(100), forwarded_rx.recv())
                .await;

        assert!(received.is_ok());
        let notif = received.unwrap().unwrap();
        assert_eq!(notif.session_id.to_string(), "sess-1");
    }

    #[tokio::test]
    async fn test_trace_notifications_handles_channel_close() {
        let (tx, rx) = broadcast::channel(16);
        let _forwarded_rx = trace_notifications("test-agent".to_string(), rx);

        drop(tx);

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_trace_notifications_handles_lag() {
        let (tx, rx) = broadcast::channel(1);
        let _forwarded_rx = trace_notifications("test-agent".to_string(), rx);

        for i in 0..5 {
            let notification = SessionNotification::new(
                SessionId::from("sess-1"),
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(format!("msg {}", i)),
                ))),
            );
            let _ = tx.send(notification);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        drop(tx);
    }

    #[tokio::test]
    async fn test_trace_notifications_flushes_buffers_on_close() {
        let (tx, rx) = broadcast::channel(16);
        let _forwarded_rx = trace_notifications("test-agent".to_string(), rx);

        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("partial chunk"),
            ))),
        );
        tx.send(notification).unwrap();

        drop(tx);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
