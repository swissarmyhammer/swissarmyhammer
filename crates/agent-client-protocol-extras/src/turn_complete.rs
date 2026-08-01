//! End-of-turn marker for a session's `session/update` notification stream.
//!
//! A client that reassembles a turn's reply from streamed
//! `SessionUpdate::AgentMessageChunk` notifications needs to know when the
//! stream for that turn is finished. The `session/prompt` **response** does not
//! answer that: notifications travel their own broadcast path (backend channel
//! → tracing hop → pool notifier → per-turn collector), so chunks are still in
//! flight when the response lands. Waiting a fixed wall-clock window for them
//! silently truncates the reply whenever the machine is loaded.
//!
//! The marker closes that gap. The agent emits it on the SAME channel as the
//! chunks, as the last act of the turn. Every hop in the chain is FIFO, so the
//! marker arrives at the collector *after* every chunk of that turn — the
//! collector stops on a real end-of-stream signal instead of a timer.
//!
//! The marker rides ACP's extensibility `_meta` map (like [`MAX_TOKENS_META_KEY`]
//! and [`PIN_ON_SAVE_META_KEY`]) under [`TURN_COMPLETE_META_KEY`], carried by an
//! otherwise-empty `SessionUpdate::SessionInfoUpdate` — an update kind that
//! declares nothing when every field is absent, so a client that does not know
//! the key ignores it.
//!
//! # Examples
//!
//! An agent marks its turn complete. Emit this on the same channel as the
//! turn's chunks, after the last one:
//!
//! ```
//! use agent_client_protocol::schema::SessionId;
//! use agent_client_protocol_extras::turn_complete_notification;
//!
//! let session = SessionId::new("sess-1");
//! let marker = turn_complete_notification(session.clone());
//!
//! assert_eq!(marker.session_id, session);
//! ```
//!
//! A client draining that channel recognizes it. An ordinary chunk is not the
//! marker, so the collector keeps reassembling until the marker arrives:
//!
//! ```
//! use agent_client_protocol::schema::{
//!     ContentBlock, ContentChunk, SessionId, SessionNotification, SessionUpdate, TextContent,
//! };
//! use agent_client_protocol_extras::{is_turn_complete, turn_complete_notification};
//!
//! let session = SessionId::new("sess-1");
//! let chunk = SessionNotification::new(
//!     session.clone(),
//!     SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
//!         TextContent::new("hello"),
//!     ))),
//! );
//!
//! assert!(!is_turn_complete(&chunk));
//! assert!(is_turn_complete(&turn_complete_notification(session)));
//! ```
//!
//! [`MAX_TOKENS_META_KEY`]: crate::MAX_TOKENS_META_KEY
//! [`PIN_ON_SAVE_META_KEY`]: crate::PIN_ON_SAVE_META_KEY

use agent_client_protocol::schema::{
    SessionId, SessionInfoUpdate, SessionNotification, SessionUpdate,
};

/// `SessionNotification` `_meta` key marking the end of a turn's notification
/// stream.
///
/// A cross-crate wire contract: the claude and llama agents emit it, the
/// per-turn notification collectors consume it. The value is the boolean
/// `true`; absent or non-`true` means an ordinary notification.
pub const TURN_COMPLETE_META_KEY: &str = "turn_complete";

/// Build the end-of-turn marker notification for `session_id`.
///
/// Emit it on the same channel as the turn's streamed chunks, after the last
/// one, so a collector that drains that channel sees it last.
#[must_use]
pub fn turn_complete_notification(session_id: impl Into<SessionId>) -> SessionNotification {
    let mut meta = serde_json::Map::new();
    meta.insert(TURN_COMPLETE_META_KEY.to_string(), serde_json::json!(true));
    SessionNotification::new(
        session_id,
        SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new()),
    )
    .meta(meta)
}

/// Whether `notification` is the end-of-turn marker.
#[must_use]
pub fn is_turn_complete(notification: &SessionNotification) -> bool {
    notification
        .meta
        .as_ref()
        .and_then(|meta| meta.get(TURN_COMPLETE_META_KEY))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{ContentBlock, ContentChunk, TextContent};

    #[test]
    fn the_marker_is_recognized_for_its_session() {
        let marker = turn_complete_notification(SessionId::new("sess-1"));
        assert!(is_turn_complete(&marker));
        assert_eq!(marker.session_id, SessionId::new("sess-1"));
    }

    #[test]
    fn an_ordinary_chunk_is_not_the_marker() {
        let chunk = SessionNotification::new(
            SessionId::new("sess-1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hello"),
            ))),
        );
        assert!(!is_turn_complete(&chunk));
    }

    #[test]
    fn a_notification_carrying_other_meta_is_not_the_marker() {
        let mut meta = serde_json::Map::new();
        meta.insert("something_else".to_string(), serde_json::json!(true));
        let notification = SessionNotification::new(
            SessionId::new("sess-1"),
            SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new()),
        )
        .meta(meta);
        assert!(!is_turn_complete(&notification));
    }

    #[test]
    fn the_marker_survives_a_json_round_trip() {
        let marker = turn_complete_notification(SessionId::new("sess-1"));
        let json = serde_json::to_string(&marker).expect("marker serializes");
        let decoded: SessionNotification = serde_json::from_str(&json).expect("marker decodes");
        assert!(is_turn_complete(&decoded));
    }
}
