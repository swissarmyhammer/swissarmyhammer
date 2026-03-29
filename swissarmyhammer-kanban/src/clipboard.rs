//! Clipboard provider trait and helpers for system clipboard access.
//!
//! Defines a `ClipboardProvider` trait that abstracts over the system clipboard,
//! a structured JSON format for clipboard payloads, and an in-memory implementation
//! for use in tests.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Trait for reading and writing text to the system clipboard.
///
/// Implementations must be `Send + Sync` so they can be shared across threads
/// and stored as `Arc<dyn ClipboardProvider>` in command context extensions.
#[async_trait]
pub trait ClipboardProvider: Send + Sync {
    /// Write a text string to the system clipboard.
    async fn write_text(&self, text: &str) -> Result<(), String>;

    /// Read the current text contents from the system clipboard.
    ///
    /// Returns `Ok(None)` when the clipboard is empty or contains non-text data.
    async fn read_text(&self) -> Result<Option<String>, String>;
}

/// Top-level clipboard payload envelope.
///
/// The `swissarmyhammer_clipboard` wrapper field acts as a type marker so we can
/// distinguish our clipboard data from arbitrary text the user may have copied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardPayload {
    /// The inner clipboard content, wrapped in a recognizable envelope.
    pub swissarmyhammer_clipboard: ClipboardContent,
}

/// The structured content placed on the clipboard by copy/cut operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardContent {
    /// The entity type (e.g. "task", "tag").
    pub entity_type: String,
    /// The entity ID (ULID).
    pub entity_id: String,
    /// The clipboard mode: "copy" or "cut".
    pub mode: String,
    /// Snapshot of the entity's fields at copy time.
    pub fields: serde_json::Value,
}

/// Serialize entity data into the clipboard JSON format.
///
/// Returns a JSON string wrapped in the `swissarmyhammer_clipboard` envelope
/// so it can be recognized on paste.
pub fn serialize_to_clipboard(
    entity_type: &str,
    entity_id: &str,
    mode: &str,
    fields: serde_json::Value,
) -> String {
    let payload = ClipboardPayload {
        swissarmyhammer_clipboard: ClipboardContent {
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            mode: mode.to_string(),
            fields,
        },
    };
    // serde_json::to_string on a well-formed struct never fails.
    serde_json::to_string(&payload).expect("clipboard payload serialization should not fail")
}

/// Attempt to deserialize clipboard text into a `ClipboardContent`.
///
/// Returns `None` if the text is not valid JSON or does not contain the
/// `swissarmyhammer_clipboard` envelope marker.
pub fn deserialize_from_clipboard(text: &str) -> Option<ClipboardContent> {
    let payload: ClipboardPayload = serde_json::from_str(text).ok()?;
    Some(payload.swissarmyhammer_clipboard)
}

/// Newtype wrapper for `Arc<dyn ClipboardProvider>` so it can be stored as a
/// sized `CommandContext` extension via `set_extension` / `require_extension`.
///
/// # Example
/// ```ignore
/// let provider = ClipboardProviderExt(Arc::new(InMemoryClipboard::new()));
/// ctx.set_extension(Arc::new(provider));
/// // Later:
/// let ext = ctx.require_extension::<ClipboardProviderExt>()?;
/// ext.0.write_text("hello").await?;
/// ```
#[derive(Clone)]
pub struct ClipboardProviderExt(pub Arc<dyn ClipboardProvider>);

/// In-memory clipboard for use in tests.
///
/// Stores a single `Option<String>` behind `Arc<Mutex<...>>` so it can be
/// cloned and shared across async test contexts.
#[derive(Debug, Clone)]
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
}

impl Default for InMemoryClipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ClipboardProvider for InMemoryClipboard {
    async fn write_text(&self, text: &str) -> Result<(), String> {
        let mut guard = self
            .contents
            .lock()
            .map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        *guard = Some(text.to_string());
        Ok(())
    }

    async fn read_text(&self) -> Result<Option<String>, String> {
        let guard = self
            .contents
            .lock()
            .map_err(|e| format!("clipboard lock poisoned: {e}"))?;
        Ok(guard.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_round_trip() {
        let fields = serde_json::json!({
            "title": "Fix the bug",
            "description": "Something is broken",
            "tags": ["bug", "urgent"]
        });
        let json_str = serialize_to_clipboard("task", "01ABC", "copy", fields.clone());

        let content = deserialize_from_clipboard(&json_str).expect("should deserialize");
        assert_eq!(content.entity_type, "task");
        assert_eq!(content.entity_id, "01ABC");
        assert_eq!(content.mode, "copy");
        assert_eq!(content.fields, fields);
    }

    #[test]
    fn deserialize_returns_none_for_garbage() {
        assert!(deserialize_from_clipboard("not json at all").is_none());
    }

    #[test]
    fn deserialize_returns_none_for_wrong_shape() {
        // Valid JSON but missing the envelope key.
        assert!(deserialize_from_clipboard(r#"{"foo": "bar"}"#).is_none());
    }

    #[test]
    fn deserialize_returns_none_for_empty_string() {
        assert!(deserialize_from_clipboard("").is_none());
    }

    #[test]
    fn serialize_produces_envelope_key() {
        let json_str =
            serialize_to_clipboard("tag", "01TAG", "cut", serde_json::json!({"name": "bug"}));
        assert!(json_str.contains("swissarmyhammer_clipboard"));
    }

    #[tokio::test]
    async fn in_memory_clipboard_write_and_read() {
        let clip = InMemoryClipboard::new();

        // Initially empty.
        let result = clip.read_text().await.unwrap();
        assert!(result.is_none());

        // Write and read back.
        clip.write_text("hello clipboard").await.unwrap();
        let result = clip.read_text().await.unwrap();
        assert_eq!(result.as_deref(), Some("hello clipboard"));
    }

    #[tokio::test]
    async fn in_memory_clipboard_overwrite() {
        let clip = InMemoryClipboard::new();
        clip.write_text("first").await.unwrap();
        clip.write_text("second").await.unwrap();
        let result = clip.read_text().await.unwrap();
        assert_eq!(result.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn in_memory_clipboard_round_trip_with_structured_data() {
        let clip = InMemoryClipboard::new();
        let fields = serde_json::json!({"title": "My Task"});
        let payload = serialize_to_clipboard("task", "01XYZ", "copy", fields.clone());

        clip.write_text(&payload).await.unwrap();
        let read_back = clip.read_text().await.unwrap().expect("should have text");
        let content = deserialize_from_clipboard(&read_back).expect("should parse");
        assert_eq!(content.entity_type, "task");
        assert_eq!(content.entity_id, "01XYZ");
        assert_eq!(content.mode, "copy");
        assert_eq!(content.fields, fields);
    }
}
