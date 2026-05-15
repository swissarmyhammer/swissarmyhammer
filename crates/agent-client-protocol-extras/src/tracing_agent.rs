//! TracingAgent - middleware that logs every ACP message flowing through.
//!
//! In ACP 0.10, `TracingAgent` was a wrapper that implemented the now-removed
//! `Agent` trait and logged each method call. ACP 0.11 replaces the trait
//! with a Role/Builder/handler model, so the wrapper is reshaped as a
//! middleware [`ConnectTo<Client>`] component:
//!
//! ```text
//!     Client  <----[real channel]---->  TracingAgent  <----[duplex channel]---->  inner Agent
//!                                       (logs both directions)
//! ```
//!
//! `TracingAgent` accepts any inner component that implements
//! `ConnectTo<Client>` (i.e. anything that "is an agent" in the new model)
//! and forwards every JSON-RPC message in both directions, emitting
//! `tracing::info!` for each one.
//!
//! In addition, [`trace_notifications`] keeps its 0.10 shape: it is a
//! broadcast-channel notification logger that buffers `AgentMessageChunk`
//! updates and flushes them as a single line per session — this remains the
//! preferred way for downstream code to log human-readable session output.

use agent_client_protocol::schema::{ContentBlock, SessionNotification, SessionUpdate};
use agent_client_protocol::{Channel, Client, ConnectTo, Result as AcpResult};
use std::collections::HashMap;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// TracingAgent middleware
// ---------------------------------------------------------------------------

/// Middleware that logs every message flowing between a client and an inner agent.
///
/// `TracingAgent` is generic over its inner component `A: ConnectTo<Client>`,
/// so it composes with any agent built via `Agent.builder()` or any other
/// component that exposes the `ConnectTo<Client>` interface.
///
/// # Example
///
/// ```ignore
/// use agent_client_protocol::Agent;
/// use agent_client_protocol_extras::TracingAgent;
///
/// let inner = Agent
///     .builder()
///     .name("my-agent")
///     // ... handlers ...
///     ;
/// let traced = TracingAgent::new(inner, "my-agent");
/// // `traced` itself is `ConnectTo<Client>` and can be `connect_to`'d
/// // to a client transport (stdio, ByteStreams, etc.).
/// ```
pub struct TracingAgent<A> {
    inner: A,
    agent_name: String,
}

impl<A> TracingAgent<A> {
    /// Create a new `TracingAgent` wrapping the given inner component.
    ///
    /// # Arguments
    /// * `inner` - any `ConnectTo<Client>` component (typically an `Agent` builder
    ///   or another middleware)
    /// * `agent_name` - human-readable name used as a tag in every log line
    pub fn new(inner: A, agent_name: impl Into<String>) -> Self {
        Self {
            inner,
            agent_name: agent_name.into(),
        }
    }

    /// Return the agent name used as a logging tag.
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// Borrow the wrapped inner component.
    pub fn inner(&self) -> &A {
        &self.inner
    }

    /// Consume the wrapper and return the inner component.
    pub fn into_inner(self) -> A {
        self.inner
    }
}

impl<A> ConnectTo<Client> for TracingAgent<A>
where
    A: ConnectTo<Client> + Send + 'static,
{
    /// Wire the client transport to the inner agent through a logging tee.
    ///
    /// Creates an internal duplex channel between us and the inner component,
    /// then runs three concurrent loops: copy-and-log client→inner, copy-and-log
    /// inner→client, and the inner component's own future.
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> AcpResult<()> {
        let agent_name = self.agent_name;

        // Internal pipe between us and the inner agent
        let (to_inner, inner_side) = Channel::duplex();

        // Drive the inner agent on its end of the duplex channel
        let inner_future = self.inner.connect_to(inner_side);

        // Drive the real client transport — we expose ourselves as the agent
        // it talks to. Construct a channel pair and let the client's transport
        // drive the other side.
        let (client_channel, client_future) = client.into_channel_and_future();

        // Wire up two copy-loops with logging between client_channel and to_inner.
        let log_client_to_inner = log_and_copy_messages(
            client_channel.rx,
            to_inner.tx,
            agent_name.clone(),
            Direction::FromClient,
        );
        let log_inner_to_client = log_and_copy_messages(
            to_inner.rx,
            client_channel.tx,
            agent_name,
            Direction::FromAgent,
        );

        match futures::try_join!(
            inner_future,
            client_future,
            log_client_to_inner,
            log_inner_to_client,
        ) {
            Ok(((), (), (), ())) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

/// Direction tag used to label log lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    /// Message coming from the client to the inner agent.
    FromClient,
    /// Message coming from the inner agent to the client.
    FromAgent,
}

impl Direction {
    fn label(self) -> &'static str {
        match self {
            Direction::FromClient => "client→agent",
            Direction::FromAgent => "agent→client",
        }
    }
}

/// Forward every message from `rx` to `tx`, emitting one `tracing::info!` per message.
///
/// This is the per-direction copy loop used by [`TracingAgent::connect_to`].
async fn log_and_copy_messages(
    mut rx: futures::channel::mpsc::UnboundedReceiver<
        AcpResult<agent_client_protocol::jsonrpcmsg::Message>,
    >,
    tx: futures::channel::mpsc::UnboundedSender<
        AcpResult<agent_client_protocol::jsonrpcmsg::Message>,
    >,
    agent_name: String,
    direction: Direction,
) -> AcpResult<()> {
    use futures::StreamExt;

    while let Some(msg) = rx.next().await {
        log_message(&agent_name, direction, &msg);
        tx.unbounded_send(msg)
            .map_err(|e| agent_client_protocol::util::internal_error(e.to_string()))?;
    }
    Ok(())
}

/// Emit a `tracing::info!` for a single JSON-RPC message.
///
/// Pulls the method name out of requests and notifications. Responses are
/// logged as `response (id=...)`.
fn log_message(
    agent_name: &str,
    direction: Direction,
    msg: &AcpResult<agent_client_protocol::jsonrpcmsg::Message>,
) {
    match msg {
        Ok(agent_client_protocol::jsonrpcmsg::Message::Request(req)) => {
            // jsonrpcmsg::Request covers both requests-with-id and notifications-without-id
            if req.id.is_some() {
                tracing::info!(
                    "[{}] {}: request method={}",
                    agent_name,
                    direction.label(),
                    req.method
                );
            } else {
                tracing::info!(
                    "[{}] {}: notification method={}",
                    agent_name,
                    direction.label(),
                    req.method
                );
            }
        }
        Ok(agent_client_protocol::jsonrpcmsg::Message::Response(resp)) => {
            tracing::info!(
                "[{}] {}: response id={:?}",
                agent_name,
                direction.label(),
                resp.id
            );
        }
        Err(err) => {
            tracing::warn!(
                "[{}] {}: transport error: {}",
                agent_name,
                direction.label(),
                err
            );
        }
    }
}

// ---------------------------------------------------------------------------
// trace_notifications: notification-channel logger (unchanged from 0.10)
// ---------------------------------------------------------------------------

/// Extract text content from ACP `ContentBlock`s for logging.
fn extract_block_text(block: &ContentBlock) -> Option<&str> {
    if let ContentBlock::Text(text) = block {
        Some(text.text.as_str())
    } else {
        None
    }
}

/// Buffers for accumulating message chunks per session.
///
/// Chunks are stored with their content-block index and source so that the
/// final flush can reconstruct the assistant message in order.
struct ChunkBuffer {
    /// (content_block_index, is_stream_event, text) tuples in arrival order.
    /// `is_stream_event=true` means the chunk came from the agent's
    /// real-time `stream_event` source; `false` means it came from a
    /// duplicate full-message source. Stream chunks are preferred when both
    /// exist for the same content-block index.
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

    /// Append a chunk with its content-block index and stream-event flag.
    fn append(&mut self, index: u64, is_stream_event: bool, text: &str) {
        self.chunks.push((index, is_stream_event, text.to_string()));
    }

    /// Concatenate all buffered chunks for this session and emit a single
    /// `tracing::info!` line, then clear the buffer.
    fn flush(&mut self, agent_name: &str) {
        if self.chunks.is_empty() {
            return;
        }

        let mut stream_chunks: HashMap<u64, String> = HashMap::new();
        let mut other_chunks: HashMap<u64, String> = HashMap::new();

        for (index, is_stream_event, text) in &self.chunks {
            if text.is_empty() {
                continue;
            }
            if *is_stream_event {
                stream_chunks.entry(*index).or_default().push_str(text);
            } else {
                other_chunks.entry(*index).or_default().push_str(text);
            }
        }

        // Prefer stream-event chunks when both are present for an index.
        let mut final_chunks: HashMap<u64, String> = HashMap::new();
        let all_indices: std::collections::HashSet<u64> = stream_chunks
            .keys()
            .chain(other_chunks.keys())
            .copied()
            .collect();
        for index in all_indices {
            if let Some(stream_text) = stream_chunks.get(&index) {
                final_chunks.insert(index, stream_text.clone());
            } else if let Some(other_text) = other_chunks.get(&index) {
                final_chunks.insert(index, other_text.clone());
            }
        }

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

/// Log a single notification, with chunk-buffering support.
///
/// Returns `true` if this notification was an `AgentMessageChunk` and was
/// buffered for later flush, `false` if it was logged immediately.
fn log_notification(
    agent_name: &str,
    notification: &SessionNotification,
    buffers: &mut HashMap<String, ChunkBuffer>,
) -> bool {
    let session_id = &notification.session_id;
    let session_key = session_id.to_string();

    match &notification.update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            if let Some(text) = extract_block_text(&chunk.content) {
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
                    text.len()
                );
                buffers
                    .entry(session_key)
                    .or_insert_with(|| ChunkBuffer::new(session_id.to_string()))
                    .append(content_block_index, is_stream_event, text);
            } else {
                tracing::debug!(
                    "[{}] session={}, AgentMessageChunk (non-text)",
                    agent_name,
                    session_id
                );
            }
            true
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            if let Some(buffer) = buffers.get_mut(&session_key) {
                buffer.flush(agent_name);
            }
            if let Some(text) = extract_block_text(&chunk.content) {
                tracing::info!(
                    "[{}] session={}, AgentThoughtChunk ({} chars): {}",
                    agent_name,
                    session_id,
                    text.len(),
                    text
                );
            }
            false
        }
        SessionUpdate::ToolCall(tool_call) => {
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

/// Spawn a task that logs every notification flowing through `receiver` and
/// re-broadcasts them on a fresh channel returned to the caller.
///
/// `AgentMessageChunk` notifications are buffered per session and emitted as a
/// single `tracing::info!` line when a non-chunk notification arrives or when
/// the channel closes.
///
/// # Arguments
/// * `agent_name` - tag prepended to every log line
/// * `receiver` - source channel; this function takes ownership of it
///
/// # Returns
/// A new `broadcast::Receiver` that downstream consumers should use; the
/// original receiver is consumed by the logging task.
pub fn trace_notifications(
    agent_name: String,
    receiver: broadcast::Receiver<SessionNotification>,
) -> broadcast::Receiver<SessionNotification> {
    let (tx, rx) = broadcast::channel(256);

    let mut recv = receiver;
    tokio::spawn(async move {
        let mut buffers: HashMap<String, ChunkBuffer> = HashMap::new();

        loop {
            match recv.recv().await {
                Ok(notification) => {
                    log_notification(&agent_name, &notification, &mut buffers);
                    let _ = tx.send(notification);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("[{}] notification receiver lagged by {}", agent_name, n);
                }
                Err(broadcast::error::RecvError::Closed) => {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{
        AvailableCommandsUpdate, ContentChunk, CurrentModeUpdate, Plan, SessionId, TextContent,
        ToolCall, ToolCallUpdate, ToolCallUpdateFields,
    };

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

    // -- TracingAgent constructor / accessors --

    #[test]
    fn test_tracing_agent_new_and_accessors() {
        struct DummyInner;
        let agent = TracingAgent::new(DummyInner, "my-agent");
        assert_eq!(agent.agent_name(), "my-agent");
        let _: &DummyInner = agent.inner();
    }

    #[test]
    fn test_tracing_agent_into_inner_returns_wrapped_value() {
        struct DummyInner(u32);
        let agent = TracingAgent::new(DummyInner(42), "x");
        let inner = agent.into_inner();
        assert_eq!(inner.0, 42);
    }

    // -- Direction labels --

    #[test]
    fn test_direction_labels() {
        assert_eq!(Direction::FromClient.label(), "client→agent");
        assert_eq!(Direction::FromAgent.label(), "agent→client");
    }

    // -- trace_notifications behaviour --

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
