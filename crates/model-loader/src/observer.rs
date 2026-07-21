//! Download progress observation.
//!
//! A [`DownloadObserver`] is an optional callback threaded through every
//! download entry point in this crate. When attached, it receives a
//! [`DownloadEvent`] at download start (0 bytes of the total reported by the
//! hub), after every received chunk, and a final event guaranteed to reach
//! `downloaded_bytes == total_bytes`. Passing `None` everywhere keeps the
//! pre-observer behavior byte-identical.

use std::sync::Arc;

/// A snapshot of download progress for a single file.
///
/// Events are emitted in order with monotonically non-decreasing
/// `downloaded_bytes`; the final event of a download always satisfies
/// `downloaded_bytes == total_bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadEvent {
    file: String,
    downloaded_bytes: u64,
    total_bytes: u64,
}

impl DownloadEvent {
    /// Create a new progress event.
    ///
    /// * `file` — the full, untruncated filename being downloaded
    /// * `downloaded_bytes` — bytes received so far
    /// * `total_bytes` — total size of the file as reported by the hub
    pub fn new(file: &str, downloaded_bytes: u64, total_bytes: u64) -> Self {
        Self {
            file: file.to_string(),
            downloaded_bytes,
            total_bytes,
        }
    }

    /// The full, untruncated filename being downloaded.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Bytes received so far.
    pub fn downloaded_bytes(&self) -> u64 {
        self.downloaded_bytes
    }

    /// Total size of the file in bytes as reported by the hub.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

/// Callback invoked with [`DownloadEvent`]s while a file downloads.
///
/// The callback must be cheap and non-blocking: it runs inline on the
/// download path (potentially from concurrent chunk tasks, serialized by the
/// emitter). Wrap it in an `Arc` so it can be shared across retries and
/// parallel chunk downloads.
pub type DownloadObserver = Arc<dyn Fn(DownloadEvent) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_event_exposes_constructor_values() {
        let event = DownloadEvent::new("some-very-long-model-filename.gguf", 42, 100);
        assert_eq!(event.file(), "some-very-long-model-filename.gguf");
        assert_eq!(event.downloaded_bytes(), 42);
        assert_eq!(event.total_bytes(), 100);
    }

    #[test]
    fn download_observer_is_invocable_through_the_alias() {
        let called = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let sink = Arc::clone(&called);
        let observer: DownloadObserver = Arc::new(move |event: DownloadEvent| {
            sink.store(
                event.downloaded_bytes(),
                std::sync::atomic::Ordering::SeqCst,
            );
        });
        observer(DownloadEvent::new("model.gguf", 7, 10));
        assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst), 7);
    }
}
