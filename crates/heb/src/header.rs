//! Event header and category types for HEB events.

use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Generator;

thread_local! {
    static ULID_GEN: RefCell<Generator> = const { RefCell::new(Generator::new()) };
}

/// Generate a monotonic ULID. Same-millisecond calls are guaranteed to sort correctly.
fn next_ulid() -> String {
    ULID_GEN.with(|gen| {
        gen.borrow_mut()
            .generate()
            .expect("ULID overflow (>2^80 in same millisecond)")
            .to_string()
    })
}

/// Coarse category for topic-based ZMQ filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// Hook lifecycle (pre_tool_use, post_tool_use, etc.)
    Hook,
    /// Session start/end
    Session,
    /// Agent spawned/completed
    Agent,
    /// Kanban mutations
    Card,
    /// Health, errors
    System,
}

impl EventCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Hook => "hook",
            Self::Session => "session",
            Self::Agent => "agent",
            Self::Card => "card",
            Self::System => "system",
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

impl fmt::Display for EventCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Event header — metadata envelope for every HEB event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHeader {
    /// ULID — globally unique, lexicographically sortable by creation time.
    pub id: String,
    /// When the event was created
    pub timestamp: DateTime<Utc>,
    /// Originating Claude Code session ID
    pub session_id: String,
    /// Working directory of the session that produced this event
    pub cwd: PathBuf,
    /// Coarse category for topic-based ZMQ filtering
    pub category: EventCategory,
    /// Fine-grained event type (e.g. "pre_tool_use", "post_tool_use")
    pub event_type: String,
    /// What produced this event (e.g. "avp-hook", "agent:xyz")
    pub source: String,
}

impl EventHeader {
    /// Create a new header with the current timestamp.
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        category: EventCategory,
        event_type: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: next_ulid(),
            timestamp: Utc::now(),
            session_id: session_id.into(),
            cwd: cwd.into(),
            category,
            event_type: event_type.into(),
            source: source.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = EventHeader::new(
            "sess-1",
            "/workspace",
            EventCategory::Hook,
            "pre_tool_use",
            "avp-hook",
        );
        let json = serde_json::to_string(&header).unwrap();
        let restored: EventHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_id, "sess-1");
        assert_eq!(restored.category, EventCategory::Hook);
        assert_eq!(restored.event_type, "pre_tool_use");
    }

    #[test]
    fn test_category_as_str() {
        assert_eq!(EventCategory::Hook.as_str(), "hook");
        assert_eq!(EventCategory::Session.as_str(), "session");
        assert_eq!(EventCategory::Agent.as_str(), "agent");
        assert_eq!(EventCategory::Card.as_str(), "card");
        assert_eq!(EventCategory::System.as_str(), "system");
    }

    #[test]
    fn test_category_as_bytes() {
        assert_eq!(EventCategory::Hook.as_bytes(), b"hook");
        assert_eq!(EventCategory::Session.as_bytes(), b"session");
        assert_eq!(EventCategory::Agent.as_bytes(), b"agent");
        assert_eq!(EventCategory::Card.as_bytes(), b"card");
        assert_eq!(EventCategory::System.as_bytes(), b"system");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(format!("{}", EventCategory::Hook), "hook");
        assert_eq!(format!("{}", EventCategory::Session), "session");
        assert_eq!(format!("{}", EventCategory::Agent), "agent");
        assert_eq!(format!("{}", EventCategory::Card), "card");
        assert_eq!(format!("{}", EventCategory::System), "system");
    }

    #[test]
    fn test_category_serde_roundtrip() {
        let categories = vec![
            EventCategory::Hook,
            EventCategory::Session,
            EventCategory::Agent,
            EventCategory::Card,
            EventCategory::System,
        ];
        for cat in &categories {
            let json = serde_json::to_string(cat).unwrap();
            let restored: EventCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, cat);
        }
    }

    #[test]
    fn test_header_fields_populated() {
        let header = EventHeader::new(
            "sess-2",
            "/tmp/test",
            EventCategory::System,
            "error",
            "test-source",
        );
        assert!(!header.id.is_empty(), "ULID should be non-empty");
        assert_eq!(header.session_id, "sess-2");
        assert_eq!(header.cwd, PathBuf::from("/tmp/test"));
        assert_eq!(header.category, EventCategory::System);
        assert_eq!(header.event_type, "error");
        assert_eq!(header.source, "test-source");
    }

    #[test]
    fn test_ulid_monotonic() {
        let h1 = EventHeader::new("s", "/", EventCategory::Hook, "a", "src");
        let h2 = EventHeader::new("s", "/", EventCategory::Hook, "b", "src");
        assert!(
            h2.id > h1.id,
            "Sequential ULIDs should be monotonically increasing"
        );
    }
}
