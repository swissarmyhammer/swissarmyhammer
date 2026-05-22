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

use crate::types::ids::ToolCallId;
use crate::types::{Message, MessageRole, Session};

/// Convert a single llama [`Message`] into an ACP [`SessionUpdate`].
///
/// Returns `None` for [`MessageRole::System`] messages: system prompts and
/// compaction summaries are agent-internal scaffolding and are not part of the
/// conversation a client replays on `session/load`.
///
/// # Mapping
///
/// * [`MessageRole::User`] → [`SessionUpdate::UserMessageChunk`]
/// * [`MessageRole::Assistant`] → [`SessionUpdate::AgentMessageChunk`]
/// * [`MessageRole::Tool`] → [`SessionUpdate::ToolCall`] (a complete tool call
///   carrying the result; see the module docs for the round-trip contract)
/// * [`MessageRole::System`] → `None`
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

/// Convert a whole conversation into the ordered ACP [`SessionUpdate`] stream.
///
/// Order is preserved. [`MessageRole::System`] messages are dropped (see
/// [`message_to_session_update`]).
pub fn messages_to_session_updates(messages: &[Message]) -> Vec<SessionUpdate> {
    messages
        .iter()
        .filter_map(message_to_session_update)
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
    updates
        .iter()
        .filter_map(session_update_to_message)
        .collect()
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
