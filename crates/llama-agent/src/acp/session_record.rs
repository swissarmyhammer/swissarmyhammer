//! Conversion between llama-agent [`Message`]s and the agent-neutral ACP
//! [`SessionRecord`] / [`SessionUpdate`] representation.
//!
//! This module is the seam between llama-agent's internal conversation model
//! and the shared `agent-client-protocol-extras` session-persistence layer.
//! It provides:
//!
//! - [`message_to_session_update`] — one llama [`Message`] to one ACP
//!   [`SessionUpdate`] (forward direction, used by `session/load` replay).
//! - [`messages_to_session_updates`] — a whole conversation forward.
//! - [`session_updates_to_messages`] — the reverse direction, reconstructing
//!   llama [`Message`]s from a persisted update stream.
//! - [`session_record_from`] — build a [`SessionRecord`] from a live
//!   [`Session`], ready to hand to [`SessionStore`](agent_client_protocol_extras::SessionStore).
//!
//! # Tool-call round-tripping
//!
//! A llama [`Message`] with [`MessageRole::Tool`] carries a tool *result*: its
//! `content` is the result text, and `tool_call_id` / `tool_name` identify the
//! call it answers. The earlier inline conversion in `load_session` collapsed
//! these into an [`AgentMessageChunk`](SessionUpdate::AgentMessageChunk) — the
//! result text was indistinguishable from agent prose and the call identity was
//! lost. Here a `Tool` message maps to a complete
//! [`ToolCall`](agent_client_protocol::schema::ToolCall) update
//! ([`SessionUpdate::ToolCall`]) with the call id, a title derived from the
//! tool name, a [`Completed`](agent_client_protocol::schema::ToolCallStatus::Completed)
//! status, and the result as tool-call content. That is a proper ACP tool-call
//! update and it round-trips: [`session_updates_to_messages`] turns a
//! [`SessionUpdate::ToolCall`] back into a `Tool` [`Message`] with the same id,
//! name, and content.
//!
//! # Compaction
//!
//! [`Session::messages`] is *already* the post-compaction conversation: when a
//! session is compacted, the old turns are replaced in place by a single
//! summary [`System`](MessageRole::System) message (see
//! [`Session::compact`](crate::types::Session::compact)). The persisted
//! `updates` stream is therefore built from `session.messages` as it stands —
//! it always reflects the current, coherent conversation a client should see on
//! `session/load`. `compaction_history` is bookkeeping metadata about *past*
//! compaction operations; it is not replayable conversation content and is
//! deliberately not projected into `updates`.

use std::str::FromStr;
use std::time::SystemTime;

use agent_client_protocol::schema::{
    ContentBlock, ContentChunk, SessionUpdate, TextContent, ToolCall, ToolCallContent,
    ToolCallStatus,
};
use agent_client_protocol_extras::SessionRecord;

use crate::acp::visible_text::{FilterSegment, VisibleTextFilter};
use crate::types::ids::ToolCallId;
use crate::types::{Message, MessageRole, Session};

/// Convert a single llama [`Message`] into one or more ACP [`SessionUpdate`]s.
///
/// Returns an empty vector for [`MessageRole::System`] messages: system
/// prompts and compaction summaries are agent-internal scaffolding and are
/// not part of the conversation a client replays on `session/load`.
///
/// # Mapping
///
/// * [`MessageRole::User`] → one [`SessionUpdate::UserMessageChunk`]
/// * [`MessageRole::Assistant`] → an ordered list of
///   [`SessionUpdate::AgentMessageChunk`] and [`SessionUpdate::AgentThoughtChunk`]
///   matching the original raw text's `<think>` / visible interleaving. The
///   raw content is run through [`VisibleTextFilter`] — the same splitter
///   used during live streaming — so a session reload reconstructs the
///   structured reasoning vs. text exactly as it was broadcast. Without this
///   step the entire raw content was emitted as one `AgentMessageChunk` and
///   the FE saw `<think>` markup mixed into the visible message body: the
///   "thinking text was lost on the MCP side" bug.
/// * [`MessageRole::Tool`] → one [`SessionUpdate::ToolCall`] (a complete tool
///   call carrying the result; see the module docs for the round-trip
///   contract)
/// * [`MessageRole::System`] → empty
pub fn message_to_session_updates(message: &Message) -> Vec<SessionUpdate> {
    match message.role {
        MessageRole::User => vec![SessionUpdate::UserMessageChunk(text_chunk(
            &message.content,
        ))],
        MessageRole::Assistant => assistant_content_to_updates(&message.content),
        MessageRole::Tool => vec![SessionUpdate::ToolCall(tool_call_from_message(message))],
        MessageRole::System => Vec::new(),
    }
}

/// Single-update form for backwards-compatible callers that only want the
/// "primary" update for a message. Loses the multi-segment split for
/// assistant messages — prefer [`message_to_session_updates`] for replayable
/// stream projection.
///
/// Kept for the public API surface; not used by the persistence path.
pub fn message_to_session_update(message: &Message) -> Option<SessionUpdate> {
    match message.role {
        MessageRole::User => Some(SessionUpdate::UserMessageChunk(text_chunk(
            &message.content,
        ))),
        MessageRole::Assistant => Some(SessionUpdate::AgentMessageChunk(text_chunk(
            &message.content,
        ))),
        MessageRole::Tool => Some(SessionUpdate::ToolCall(tool_call_from_message(message))),
        MessageRole::System => None,
    }
}

/// Split a persisted assistant `content` string into the ordered
/// [`SessionUpdate`] stream the FE expects: `<think>` runs become
/// [`SessionUpdate::AgentThoughtChunk`], visible runs become
/// [`SessionUpdate::AgentMessageChunk`], and `<tool_call>` markup is dropped
/// (the actual tool call is replayed from the following `Tool` message).
///
/// Uses the same [`VisibleTextFilter`] that the live streaming path uses, so
/// the projected stream a `session/load` replays is byte-equivalent to what
/// the client originally saw broadcast.
fn assistant_content_to_updates(content: &str) -> Vec<SessionUpdate> {
    let mut filter = VisibleTextFilter::default();
    let mut segments = filter.push(content);
    segments.extend(filter.finish());
    segments
        .into_iter()
        .map(|seg| match seg {
            FilterSegment::Visible(text) => SessionUpdate::AgentMessageChunk(text_chunk(&text)),
            FilterSegment::Thought(text) => SessionUpdate::AgentThoughtChunk(text_chunk(&text)),
        })
        .collect()
}

/// Convert a whole conversation into the ordered ACP [`SessionUpdate`] stream.
///
/// Order is preserved. [`MessageRole::System`] messages are dropped, and
/// each assistant message expands into the ordered visible/thought stream
/// produced by [`assistant_content_to_updates`] so reasoning survives a
/// `session/load`.
pub fn messages_to_session_updates(messages: &[Message]) -> Vec<SessionUpdate> {
    messages
        .iter()
        .flat_map(message_to_session_updates)
        .collect()
}

/// Reconstruct llama [`Message`]s from a persisted ACP [`SessionUpdate`] stream.
///
/// This is the inverse of [`messages_to_session_updates`] for the update
/// variants this agent emits:
///
/// * [`SessionUpdate::UserMessageChunk`] → [`MessageRole::User`]
/// * [`SessionUpdate::AgentMessageChunk`] → [`MessageRole::Assistant`]
/// * [`SessionUpdate::ToolCall`] → [`MessageRole::Tool`], restoring the call id,
///   tool name, and result content.
///
/// Other [`SessionUpdate`] variants (plans, mode changes, thought chunks, …)
/// carry no llama [`Message`] equivalent and are skipped. Messages are stamped
/// with the current time: the original per-message timestamps are not part of
/// the persisted update stream.
pub fn session_updates_to_messages(updates: &[SessionUpdate]) -> Vec<Message> {
    // Stateful pass: `AgentThoughtChunk` and consecutive `AgentMessageChunk`s
    // are folded back into one [`MessageRole::Assistant`] message whose
    // `content` reconstructs the original `<think>…</think>` markup. Without
    // this, a session that persisted a split visible/thought stream (the new
    // `assistant_content_to_updates` projection) would lose the thinking on
    // BE resume — the inverse of the bug fixed in the forward direction.
    let mut out: Vec<Message> = Vec::with_capacity(updates.len());
    let mut buffered: Option<AssistantBuilder> = None;

    for update in updates {
        match update {
            SessionUpdate::AgentThoughtChunk(chunk) => {
                let text = content_block_text(&chunk.content);
                buffered.get_or_insert_with(AssistantBuilder::default).push_thought(&text);
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                let text = content_block_text(&chunk.content);
                buffered.get_or_insert_with(AssistantBuilder::default).push_visible(&text);
            }
            other => {
                if let Some(builder) = buffered.take() {
                    out.push(builder.finish());
                }
                if let Some(msg) = session_update_to_message(other) {
                    out.push(msg);
                }
            }
        }
    }
    if let Some(builder) = buffered.take() {
        out.push(builder.finish());
    }
    out
}

/// Accumulator for the visible/thought stream of one assistant turn. Each
/// `push_*` call appends to a single growing `content` buffer in source
/// order, re-wrapping thought runs in `<think>…</think>` so the result is
/// indistinguishable from the original raw model output. `finish` produces
/// the canonical [`MessageRole::Assistant`] message.
#[derive(Default)]
struct AssistantBuilder {
    content: String,
}

impl AssistantBuilder {
    fn push_visible(&mut self, text: &str) {
        self.content.push_str(text);
    }

    fn push_thought(&mut self, text: &str) {
        self.content.push_str("<think>");
        self.content.push_str(text);
        self.content.push_str("</think>");
    }

    fn finish(self) -> Message {
        plain_message(MessageRole::Assistant, self.content)
    }
}

/// Build an agent-neutral [`SessionRecord`] from a live [`Session`].
///
/// Captures the session id, working directory, a last-activity timestamp
/// derived from [`Session::updated_at`], the stored
/// [`title`](Session::title), and the conversation projected onto
/// [`SessionRecord::updates`]. The MCP server configuration is intentionally
/// left empty: the record's `updates` stream is the replayable conversation,
/// and llama's [`MCPServerConfig`](crate::types::mcp::MCPServerConfig) has no
/// lossless mapping onto ACP's `McpServer`.
///
/// The title is *not* derived here — it is generated once, after the first
/// meaningful exchange (see [`first_user_message_text`] and
/// [`has_first_exchange`]), and stored on the session. This keeps persistence
/// a pure projection of session state.
///
/// # Parameters
///
/// * `acp_session_id` - The opaque ACP session id to key the record on.
/// * `session` - The live llama session whose conversation is being persisted.
pub fn session_record_from(acp_session_id: &str, session: &Session) -> SessionRecord {
    let updated_at = system_time_to_rfc3339(session.updated_at);
    let mut record = SessionRecord::new(acp_session_id, session.cwd.clone(), updated_at);
    record.title = session.title.clone();
    record.updates = messages_to_session_updates(&session.messages);
    record
}

/// Text of the earliest non-empty user message in a conversation.
///
/// Returns `None` when the conversation has no user-message text yet. This is
/// the heuristic title source and the model-prompt seed for llama-agent's
/// title generation.
pub fn first_user_message_text(messages: &[Message]) -> Option<String> {
    messages.iter().find_map(|message| {
        if message.role != MessageRole::User {
            return None;
        }
        let trimmed = message.content.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// Whether a conversation has had its first meaningful exchange — at least one
/// user message *and* at least one assistant response.
///
/// This is the shared trigger condition for session-title generation (see the
/// contract in [`agent_client_protocol_extras::session_title`]).
pub fn has_first_exchange(messages: &[Message]) -> bool {
    let has_user = messages.iter().any(|m| m.role == MessageRole::User);
    let has_assistant = messages.iter().any(|m| m.role == MessageRole::Assistant);
    has_user && has_assistant
}

/// Render a [`SystemTime`] as an RFC 3339 / ISO 8601 timestamp string.
///
/// Times before the Unix epoch (which should not occur for session activity)
/// clamp to the epoch.
fn system_time_to_rfc3339(time: SystemTime) -> String {
    let unix_secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    chrono::DateTime::from_timestamp(unix_secs, 0)
        .unwrap_or_default()
        .to_rfc3339()
}

/// Wrap plain text in a [`ContentChunk`] carrying a single text [`ContentBlock`].
fn text_chunk(text: &str) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text.to_string())))
}

/// Build a complete [`ToolCall`] update from a [`MessageRole::Tool`] message.
///
/// The tool call is reported as already [`Completed`](ToolCallStatus::Completed)
/// with the message content as its sole [`ToolCallContent`]. A missing
/// `tool_call_id` falls back to a freshly generated id; a missing `tool_name`
/// falls back to a generic title.
fn tool_call_from_message(message: &Message) -> ToolCall {
    let call_id = message
        .tool_call_id
        .as_ref()
        .map(ToolCallId::to_string)
        .unwrap_or_else(|| ToolCallId::new().to_string());
    let title = message
        .tool_name
        .clone()
        .unwrap_or_else(|| "Tool call".to_string());

    ToolCall::new(call_id, title)
        .status(ToolCallStatus::Completed)
        .content(vec![ToolCallContent::from(ContentBlock::Text(
            TextContent::new(message.content.clone()),
        ))])
}

/// Reverse of [`message_to_session_update`] for a single update.
///
/// Returns `None` for [`SessionUpdate`] variants that have no llama [`Message`]
/// equivalent.
///
/// [`SessionUpdate::AgentThoughtChunk`] folds into the prior assistant
/// message rather than producing a fresh one — see
/// [`session_updates_to_messages`] for the coalescing logic. This helper
/// alone cannot do that (it has no prior context), so it returns `None` for
/// thought chunks and callers must use the stream-aware variant.
fn session_update_to_message(update: &SessionUpdate) -> Option<Message> {
    match update {
        SessionUpdate::UserMessageChunk(chunk) => Some(plain_message(
            MessageRole::User,
            content_block_text(&chunk.content),
        )),
        SessionUpdate::AgentMessageChunk(chunk) => Some(plain_message(
            MessageRole::Assistant,
            content_block_text(&chunk.content),
        )),
        SessionUpdate::ToolCall(tool_call) => Some(message_from_tool_call(tool_call)),
        _ => None,
    }
}

/// Reconstruct a [`MessageRole::Tool`] [`Message`] from a persisted [`ToolCall`].
///
/// The result content is recovered by concatenating the text of every
/// [`ToolCallContent::Content`] text block; non-text content (diffs, terminals)
/// has no llama representation and is dropped.
///
/// The llama [`ToolCallId`] is internally a ULID, whereas ACP tool-call ids are
/// opaque strings. A tool call produced by this agent always carries a ULID id,
/// so it parses cleanly; an id that is not a ULID (e.g. from a foreign record)
/// falls back to a fresh [`ToolCallId`] rather than failing the restore — the
/// result content and tool name are preserved regardless.
fn message_from_tool_call(tool_call: &ToolCall) -> Message {
    let content = tool_call
        .content
        .iter()
        .filter_map(|item| match item {
            ToolCallContent::Content(content) => Some(content_block_text(&content.content)),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");

    let tool_call_id = ToolCallId::from_str(&tool_call.tool_call_id.0).unwrap_or_else(|_| {
        tracing::debug!(
            "Tool call id {} is not a ULID; assigning a fresh id on restore",
            tool_call.tool_call_id.0
        );
        ToolCallId::new()
    });

    Message {
        role: MessageRole::Tool,
        content,
        tool_call_id: Some(tool_call_id),
        tool_name: Some(tool_call.title.clone()),
        timestamp: SystemTime::now(),
    }
}

/// Build a text-only [`Message`] with the given role, stamped with the current
/// time.
fn plain_message(role: MessageRole, content: String) -> Message {
    Message {
        role,
        content,
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    }
}

/// Extract the text of a [`ContentBlock`], or the empty string for non-text
/// blocks (images, audio, resources) that have no llama text representation.
fn content_block_text(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ids::ToolCallId;

    /// Build a text [`Message`] with the given role.
    fn msg(role: MessageRole, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }
    }

    /// Build a [`MessageRole::Tool`] result message.
    fn tool_msg(content: &str, id: ToolCallId, name: &str) -> Message {
        Message {
            role: MessageRole::Tool,
            content: content.to_string(),
            tool_call_id: Some(id),
            tool_name: Some(name.to_string()),
            timestamp: SystemTime::now(),
        }
    }

    /// A user message becomes a `UserMessageChunk` carrying its text.
    #[test]
    fn user_message_maps_to_user_chunk() {
        let update = message_to_session_update(&msg(MessageRole::User, "hello")).unwrap();
        match update {
            SessionUpdate::UserMessageChunk(chunk) => {
                assert_eq!(content_block_text(&chunk.content), "hello");
            }
            other => panic!("expected UserMessageChunk, got {other:?}"),
        }
    }

    /// An assistant message becomes an `AgentMessageChunk`.
    #[test]
    fn assistant_message_maps_to_agent_chunk() {
        let update = message_to_session_update(&msg(MessageRole::Assistant, "hi there")).unwrap();
        assert!(matches!(update, SessionUpdate::AgentMessageChunk(_)));
    }

    /// The user-reported bug: an assistant message with `<think>` markup must
    /// project to a SEPARATE thought update plus a separate visible update,
    /// in source order. Before this fix the entire raw content (markup
    /// included) was crammed into one `AgentMessageChunk` and the FE saw
    /// `<think>` tags as part of the assistant's visible reply — "the
    /// thinking text was lost on the MCP side".
    #[test]
    fn assistant_message_with_think_splits_into_thought_then_visible() {
        let raw = "<think>let me check</think>Here you go.";
        let updates = messages_to_session_updates(&[msg(MessageRole::Assistant, raw)]);
        assert_eq!(updates.len(), 2, "must emit two updates, got {updates:?}");
        match &updates[0] {
            SessionUpdate::AgentThoughtChunk(chunk) => {
                assert_eq!(content_block_text(&chunk.content), "let me check");
            }
            other => panic!("expected AgentThoughtChunk first, got {other:?}"),
        }
        match &updates[1] {
            SessionUpdate::AgentMessageChunk(chunk) => {
                assert_eq!(content_block_text(&chunk.content), "Here you go.");
            }
            other => panic!("expected AgentMessageChunk second, got {other:?}"),
        }
    }

    /// `<tool_call>` markup persisted in the assistant's raw content must be
    /// stripped from the projected updates — the structured ToolCall comes
    /// from the following Tool-role message. Leaking the raw markup into an
    /// AgentMessageChunk would render the tool-call JSON as visible message
    /// text in the UI, which is the exact pre-filter bug.
    #[test]
    fn assistant_message_drops_tool_call_markup_from_visible_stream() {
        let raw = r#"I'll look that up.<tool_call>{"name":"x"}</tool_call>"#;
        let updates = messages_to_session_updates(&[msg(MessageRole::Assistant, raw)]);
        // Only the leading visible run survives; the tool_call body is dropped.
        assert_eq!(updates.len(), 1, "tool_call markup must be stripped");
        match &updates[0] {
            SessionUpdate::AgentMessageChunk(chunk) => {
                let text = content_block_text(&chunk.content);
                assert_eq!(text, "I'll look that up.");
                assert!(!text.contains("<tool_call>"));
                assert!(!text.contains("\"name\""));
            }
            other => panic!("expected AgentMessageChunk, got {other:?}"),
        }
    }

    /// Thought-before-visible-before-thought (multiple interleaved spans in
    /// one assistant message) must produce alternating updates in source
    /// order, so the FE renders reasoning in the spot it actually appeared.
    #[test]
    fn assistant_message_preserves_thought_visible_interleave_on_load() {
        let raw = "<think>a</think>X<think>b</think>Y";
        let updates = messages_to_session_updates(&[msg(MessageRole::Assistant, raw)]);
        let kinds: Vec<&str> = updates
            .iter()
            .map(|u| match u {
                SessionUpdate::AgentThoughtChunk(_) => "T",
                SessionUpdate::AgentMessageChunk(_) => "V",
                _ => "?",
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["T", "V", "T", "V"],
            "must emit T V T V in order"
        );
    }

    /// On BE resume, the split update stream must coalesce back into a
    /// single assistant Message whose content reconstructs the original
    /// `<think>…</think>` markup — the inverse of the forward split. Without
    /// this round-trip the next turn's prompt would lose the prior turn's
    /// thinking entirely.
    #[test]
    fn assistant_thought_and_visible_round_trip_through_session_updates() {
        let original = "<think>plan</think>Done.";
        let updates = messages_to_session_updates(&[msg(MessageRole::Assistant, original)]);
        let restored = session_updates_to_messages(&updates);
        assert_eq!(
            restored.len(),
            1,
            "two updates must coalesce into one assistant message"
        );
        assert_eq!(restored[0].role, MessageRole::Assistant);
        assert_eq!(restored[0].content, original);
    }

    /// A persisted assistant message with no `<think>` markup must still
    /// project to a single `AgentMessageChunk` and round-trip cleanly — the
    /// new code path must not regress the common no-reasoning case.
    #[test]
    fn assistant_message_without_think_is_a_single_chunk() {
        let raw = "just an answer";
        let updates = messages_to_session_updates(&[msg(MessageRole::Assistant, raw)]);
        assert_eq!(updates.len(), 1);
        assert!(matches!(updates[0], SessionUpdate::AgentMessageChunk(_)));

        let restored = session_updates_to_messages(&updates);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].content, raw);
    }

    /// System messages (prompts, compaction summaries) are not replayed.
    #[test]
    fn system_message_is_skipped() {
        assert!(message_to_session_update(&msg(MessageRole::System, "be nice")).is_none());
    }

    /// A tool message becomes a complete, completed `ToolCall` update — not an
    /// agent text chunk — preserving the call id, tool name, and result.
    #[test]
    fn tool_message_maps_to_tool_call_update() {
        let id = ToolCallId::new();
        let update =
            message_to_session_update(&tool_msg(r#"{"ok":true}"#, id, "get_weather")).unwrap();
        match update {
            SessionUpdate::ToolCall(call) => {
                assert_eq!(call.tool_call_id.0.as_ref(), id.to_string());
                assert_eq!(call.title, "get_weather");
                assert_eq!(call.status, ToolCallStatus::Completed);
                assert_eq!(call.content.len(), 1);
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }

    /// A full conversation round-trips through updates and back without losing
    /// user / assistant / tool content — including the tool call's identity.
    #[test]
    fn conversation_round_trips_loss_free() {
        let tool_id = ToolCallId::new();
        let original = vec![
            msg(MessageRole::User, "what is the weather?"),
            msg(MessageRole::Assistant, "let me check"),
            tool_msg(r#"{"temp":72}"#, tool_id, "get_weather"),
            msg(MessageRole::Assistant, "it is 72 degrees"),
        ];

        let updates = messages_to_session_updates(&original);
        assert_eq!(updates.len(), 4);

        let restored = session_updates_to_messages(&updates);
        assert_eq!(restored.len(), 4);

        assert_eq!(restored[0].role, MessageRole::User);
        assert_eq!(restored[0].content, "what is the weather?");

        assert_eq!(restored[1].role, MessageRole::Assistant);
        assert_eq!(restored[1].content, "let me check");

        assert_eq!(restored[2].role, MessageRole::Tool);
        assert_eq!(restored[2].content, r#"{"temp":72}"#);
        assert_eq!(
            restored[2].tool_call_id.as_ref().map(ToolCallId::to_string),
            Some(tool_id.to_string())
        );
        assert_eq!(restored[2].tool_name.as_deref(), Some("get_weather"));

        assert_eq!(restored[3].role, MessageRole::Assistant);
        assert_eq!(restored[3].content, "it is 72 degrees");
    }

    /// System messages drop out of the update stream, so a round trip omits
    /// them — the replayed conversation contains only client-visible turns.
    #[test]
    fn system_messages_are_omitted_from_round_trip() {
        let original = vec![
            msg(MessageRole::System, "system prompt"),
            msg(MessageRole::User, "hi"),
        ];
        let restored = session_updates_to_messages(&messages_to_session_updates(&original));
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].role, MessageRole::User);
    }

    /// `first_user_message_text` returns the earliest non-empty user message,
    /// ignoring system scaffolding.
    #[test]
    fn first_user_message_text_skips_system_messages() {
        let messages = vec![
            msg(MessageRole::System, "ignored"),
            msg(MessageRole::User, "Implement the feature"),
            msg(MessageRole::User, "a follow-up"),
        ];
        assert_eq!(
            first_user_message_text(&messages).as_deref(),
            Some("Implement the feature")
        );
    }

    /// With no user message yet, there is no title source.
    #[test]
    fn first_user_message_text_is_none_without_user_message() {
        let messages = vec![msg(MessageRole::System, "system only")];
        assert!(first_user_message_text(&messages).is_none());
    }

    /// `has_first_exchange` is true only once both a user message and an
    /// assistant response are present.
    #[test]
    fn has_first_exchange_requires_user_and_assistant() {
        assert!(!has_first_exchange(&[]));
        assert!(!has_first_exchange(&[msg(MessageRole::User, "hi")]));
        assert!(has_first_exchange(&[
            msg(MessageRole::User, "hi"),
            msg(MessageRole::Assistant, "hello"),
        ]));
    }
}
