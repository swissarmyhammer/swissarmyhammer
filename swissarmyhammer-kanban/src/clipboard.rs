//! Clipboard abstraction for copy/cut/paste operations.
//!
//! Provides a trait `ClipboardProvider` for reading/writing text to a clipboard,
//! an `InMemoryClipboard` for tests, and a `ClipboardProviderExt` newtype for
//! injecting the provider into `CommandContext` via the extension system.

use std::sync::{Arc, RwLock};

/// Trait abstracting clipboard read/write.
///
/// Implementations may wrap the system clipboard (via a Tauri or OS API) or an
/// in-memory buffer for testing.
pub trait ClipboardProvider: Send + Sync + std::fmt::Debug {
    /// Write text to the clipboard.
    fn write_text(&self, text: &str);

    /// Read text from the clipboard. Returns `None` if empty or unavailable.
    fn read_text(&self) -> Option<String>;
}

/// In-memory clipboard for testing. Thread-safe via internal `RwLock`.
#[derive(Debug)]
pub struct InMemoryClipboard {
    content: RwLock<Option<String>>,
}

impl InMemoryClipboard {
    /// Create a new empty in-memory clipboard.
    pub fn new() -> Self {
        Self {
            content: RwLock::new(None),
        }
    }
}

impl ClipboardProvider for InMemoryClipboard {
    fn write_text(&self, text: &str) {
        let mut guard = self.content.write().expect("clipboard lock poisoned");
        *guard = Some(text.to_string());
    }

    fn read_text(&self) -> Option<String> {
        let guard = self.content.read().expect("clipboard lock poisoned");
        guard.clone()
    }
}

/// Extension newtype for injecting a `ClipboardProvider` into `CommandContext`.
///
/// Usage:
/// ```ignore
/// let clipboard = Arc::new(InMemoryClipboard::new());
/// let ext = ClipboardProviderExt(clipboard as Arc<dyn ClipboardProvider>);
/// ctx.set_extension(Arc::new(ext));
/// ```
#[derive(Debug)]
pub struct ClipboardProviderExt(pub Arc<dyn ClipboardProvider>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_clipboard_roundtrip() {
        let cb = InMemoryClipboard::new();
        assert!(cb.read_text().is_none());
        cb.write_text("hello");
        assert_eq!(cb.read_text().as_deref(), Some("hello"));
    }

    #[test]
    fn in_memory_clipboard_overwrite() {
        let cb = InMemoryClipboard::new();
        cb.write_text("first");
        cb.write_text("second");
        assert_eq!(cb.read_text().as_deref(), Some("second"));
    }
}
