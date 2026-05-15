//! Structured progress events for the code-context indexer.
//!
//! The indexer emits typed [`IndexProgress`] events through a small
//! [`ProgressReporter`] trait. Consumers (MCP notifications, CLI TUI, tests)
//! pick their own rendering strategy — the indexer never formats a progress
//! string itself.
//!
//! ## Why this trait is synchronous
//!
//! Progress events are emitted from inside indexing tasks that hold tight
//! loops over files and embedding batches, so the reporter cannot use
//! `async fn report`. Reporter implementations that need to perform async
//! work (sending JSON-RPC notifications, redrawing TUIs from a tokio task)
//! should buffer events internally via an `mpsc` channel and run a drain
//! task elsewhere.
//!
//! ## Best-effort totals
//!
//! [`IndexProgress::Chunking`] and [`IndexProgress::Embedding`] carry
//! `total` and `batches` counts respectively. These are best-effort — they
//! report `0` while the value is not yet known (for example before discovery
//! has finished counting files) and are populated once the count becomes
//! available. Consumers must therefore tolerate a `0` denominator.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Final tally returned by an indexing run.
///
/// Mirrors the payload of [`IndexProgress::Done`] but is returned synchronously
/// to the caller so it does not need to install a custom [`ProgressReporter`]
/// just to read the run summary. Callers that drive the indexer purely for
/// its side effects (the MCP bootstrap pass, the file watcher) may ignore the
/// returned value.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexRunStats {
    /// Files processed in this run.
    pub files: u64,
    /// Chunks written in this run.
    pub chunks: u64,
    /// Wall-clock duration of the run.
    pub elapsed: Duration,
}

/// A single progress event emitted by the indexer.
///
/// Each variant is a closed enum with named fields so consumers can pattern
/// match without parsing strings.
#[derive(Debug, Clone, PartialEq)]
pub enum IndexProgress {
    /// File discovery is in progress. `found` is the running count of files
    /// added to the dirty set so far. The first event is typically
    /// `Discovering { found: 0 }` (before discovery starts); a second
    /// `Discovering { found: N }` event is emitted once discovery finishes
    /// so consumers know the total without waiting for the first
    /// [`IndexProgress::Chunking`] event.
    Discovering {
        /// Files discovered so far.
        found: u64,
    },
    /// A single file has been tree-sitter chunked. `done` is the number of
    /// files chunked so far (monotonically non-decreasing within a run);
    /// `total` is the number of files the run intends to process. `total`
    /// is `0` if discovery has not produced a final count yet.
    Chunking {
        /// The file that was just chunked.
        file: PathBuf,
        /// Files chunked so far (1-based once the first file completes).
        done: u64,
        /// Total files to chunk in this run, or `0` if not yet known.
        total: u64,
    },
    /// A batch of chunks has been embedded. `batch` is the 1-based batch
    /// index; `batches` is the planned total number of batches (`0` if not
    /// yet known); `chunks_in_batch` is the number of chunks in the batch
    /// that was just completed.
    Embedding {
        /// 1-based index of the batch that was just embedded.
        batch: u64,
        /// Total batches expected, or `0` if not yet known.
        batches: u64,
        /// Number of chunks in the batch that was just embedded.
        chunks_in_batch: u64,
    },
    /// The indexing run has finished. Final tallies and wall-clock duration.
    Done {
        /// Files processed in this run.
        files: u64,
        /// Chunks written in this run.
        chunks: u64,
        /// Wall-clock duration of the run.
        elapsed: Duration,
    },
}

/// Sink for indexer progress events.
///
/// Implementations must be cheap to call from a hot loop — the indexer
/// emits one event per file chunked and one per embedding batch.
/// Implementations that need to do async work should forward events to an
/// `mpsc` channel and drain them in a separate task.
pub trait ProgressReporter: Send + Sync {
    /// Record a single progress event.
    ///
    /// Implementations should not block; they should not return errors.
    /// Dropped events are an acceptable failure mode (progress is advisory).
    fn report(&self, event: IndexProgress);
}

/// A reporter that discards every event.
///
/// This is the default for callers that do not surface progress (the MCP
/// bootstrap pass, the file watcher, internal tests). It exists as a named
/// type so call sites can write `Arc::new(NoopReporter)` rather than
/// constructing a closure.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn report(&self, _event: IndexProgress) {}
}

/// Convenience constructor: a shared [`NoopReporter`] wrapped in `Arc`.
///
/// Returns `Arc<dyn ProgressReporter>` so the value can be passed directly
/// to indexer functions whose signature is `Arc<dyn ProgressReporter>`.
pub fn noop_reporter() -> Arc<dyn ProgressReporter> {
    Arc::new(NoopReporter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A `ProgressReporter` that records every event into a `Mutex<Vec<_>>`
    /// so tests can assert on the recorded sequence.
    struct VecReporter {
        events: Mutex<Vec<IndexProgress>>,
    }

    impl VecReporter {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }

        fn take(&self) -> Vec<IndexProgress> {
            std::mem::take(&mut self.events.lock().unwrap())
        }
    }

    impl ProgressReporter for VecReporter {
        fn report(&self, event: IndexProgress) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn noop_reporter_swallows_events() {
        let reporter = NoopReporter;
        // Just verify it compiles and runs — there is no observable side effect.
        reporter.report(IndexProgress::Discovering { found: 0 });
        reporter.report(IndexProgress::Done {
            files: 0,
            chunks: 0,
            elapsed: Duration::from_secs(0),
        });
    }

    #[test]
    fn noop_reporter_helper_returns_dyn_arc() {
        let reporter: Arc<dyn ProgressReporter> = noop_reporter();
        reporter.report(IndexProgress::Discovering { found: 42 });
    }

    #[test]
    fn vec_reporter_records_events_in_order() {
        let reporter = VecReporter::new();
        reporter.report(IndexProgress::Discovering { found: 0 });
        reporter.report(IndexProgress::Discovering { found: 3 });
        reporter.report(IndexProgress::Done {
            files: 3,
            chunks: 7,
            elapsed: Duration::from_millis(10),
        });
        let events = reporter.take();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], IndexProgress::Discovering { found: 0 }));
        assert!(matches!(events[1], IndexProgress::Discovering { found: 3 }));
        assert!(matches!(
            events[2],
            IndexProgress::Done {
                files: 3,
                chunks: 7,
                ..
            }
        ));
    }
}
