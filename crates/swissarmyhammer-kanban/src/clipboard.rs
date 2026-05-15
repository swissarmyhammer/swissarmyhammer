//! Clipboard infrastructure for entity copy/cut/paste operations.
//!
//! Provides:
//! - `ClipboardProvider` trait for system clipboard I/O
//! - `InMemoryClipboard` for testing
//! - `ClipboardPayload` JSON wrapper for safe deserialization
//! - Serialization/deserialization helpers

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Trait for reading/writing text to the system clipboard.
///
/// Implementations wrap platform-specific clipboard APIs (e.g., Tauri plugin).
/// The `InMemoryClipboard` implementation is available for unit tests.
#[async_trait]
pub trait ClipboardProvider: Send + Sync {
    /// Write text to the system clipboard.
    async fn write_text(&self, text: &str) -> Result<(), String>;

    /// Read text from the system clipboard. Returns `None` if clipboard is empty.
    async fn read_text(&self) -> Result<Option<String>, String>;
}

/// Newtype wrapper for `Arc<dyn ClipboardProvider>` so it can be stored as a
/// CommandContext extension (requires a concrete type for `TypeId` keying).
pub struct ClipboardProviderExt(pub Arc<dyn ClipboardProvider>);

/// In-memory clipboard implementation for unit and integration tests.
///
/// Thread-safe via `Mutex`. Does not interact with the OS clipboard.
#[derive(Debug, Clone, Default)]
pub struct InMemoryClipboard {
    contents: Arc<Mutex<Option<String>>>,
}

impl InMemoryClipboard {
    /// Create a new empty in-memory clipboard.
    pub fn new() -> Self {
        Self {
            contents: Arc::new(Mutex::new(None)),
        }
    }

    /// Peek at the current clipboard contents without consuming them.
    pub fn peek(&self) -> Option<String> {
        self.contents.lock().unwrap().clone()
    }
}

#[async_trait]
impl ClipboardProvider for InMemoryClipboard {
    async fn write_text(&self, text: &str) -> Result<(), String> {
        *self.contents.lock().unwrap() = Some(text.to_string());
        Ok(())
    }

    async fn read_text(&self) -> Result<Option<String>, String> {
        Ok(self.contents.lock().unwrap().clone())
    }
}

/// JSON payload written to the clipboard.
///
/// The `swissarmyhammer_clipboard` key acts as a type marker so we can
/// distinguish our clipboard data from arbitrary text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardPayload {
    /// The marker/wrapper object containing entity data.
    pub swissarmyhammer_clipboard: ClipboardData,
}

/// Inner clipboard data with entity type, original ID, mode, and field snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardData {
    /// Entity type (e.g., "task").
    pub entity_type: String,
    /// Original entity ID (for reference; paste always creates a new ID).
    pub entity_id: String,
    /// "copy" or "cut" — informational, does not affect paste behavior.
    pub mode: String,
    /// Snapshot of all entity fields at copy/cut time.
    pub fields: Value,
}

/// Serialize an entity's fields into clipboard JSON format.
///
/// Returns the JSON string ready to write to the clipboard.
pub fn serialize_to_clipboard(
    entity_type: &str,
    entity_id: &str,
    mode: &str,
    fields: Value,
) -> String {
    let payload = ClipboardPayload {
        swissarmyhammer_clipboard: ClipboardData {
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            mode: mode.to_string(),
            fields,
        },
    };
    serde_json::to_string(&payload).expect("clipboard payload serialization should not fail")
}

/// Attempt to deserialize clipboard text as a `ClipboardPayload`.
///
/// Returns `None` if the text is not valid JSON or does not contain
/// the `swissarmyhammer_clipboard` marker.
pub fn deserialize_from_clipboard(text: &str) -> Option<ClipboardPayload> {
    serde_json::from_str::<ClipboardPayload>(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let fields = serde_json::json!({
            "title": "Test task",
            "body": "A description with #bug tag",
            "assignees": ["alice"],
        });

        let json = serialize_to_clipboard("task", "01ABC", "copy", fields.clone());
        let parsed = deserialize_from_clipboard(&json).expect("should deserialize");

        assert_eq!(parsed.swissarmyhammer_clipboard.entity_type, "task");
        assert_eq!(parsed.swissarmyhammer_clipboard.entity_id, "01ABC");
        assert_eq!(parsed.swissarmyhammer_clipboard.mode, "copy");
        assert_eq!(parsed.swissarmyhammer_clipboard.fields, fields);
    }

    #[test]
    fn deserialize_returns_none_for_invalid_json() {
        assert!(deserialize_from_clipboard("not json").is_none());
    }

    #[test]
    fn deserialize_returns_none_for_wrong_structure() {
        assert!(deserialize_from_clipboard(r#"{"other": "data"}"#).is_none());
    }

    #[tokio::test]
    async fn in_memory_clipboard_write_read() {
        let clipboard = InMemoryClipboard::new();
        clipboard.write_text("hello").await.unwrap();
        let text = clipboard.read_text().await.unwrap();
        assert_eq!(text, Some("hello".to_string()));
    }

    #[tokio::test]
    async fn in_memory_clipboard_starts_empty() {
        let clipboard = InMemoryClipboard::new();
        let text = clipboard.read_text().await.unwrap();
        assert_eq!(text, None);
    }
}
