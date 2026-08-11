//! The indexing pass: what it writes, what it reports, and when it stops.

use model_embedding::mock::MockEmbedder;
use model_embedding::TextEmbedder;

use crate::mcp::tools::code_context::indexing::{
    download_observer_for, index_discovered_files_with_embedder,
};

use super::support::{
    count_embedded_chunks, count_total_chunks, make_tiny_indexable_project, read_embedded_flag,
};

/// With a working embedder, every chunk row has a non-NULL embedding blob
/// and every fully-embedded file has `embedded=1`.
#[tokio::test]
async fn test_indexer_writes_embedding_blob_for_every_chunk() {
    let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

    // Always-succeeding mock embedder with a small fixed dimension.
    let embedder: std::sync::Arc<dyn TextEmbedder> = std::sync::Arc::new(MockEmbedder::new(8));

    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        Some(embedder),
        swissarmyhammer_code_context::noop_reporter(),
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let total = count_total_chunks(&shared_db);
    let embedded = count_embedded_chunks(&shared_db);
    assert!(total > 0, "expected >0 chunks after indexing, got {total}");
    assert_eq!(
        embedded, total,
        "every chunk should have a non-NULL embedding blob"
    );

    // Files should be marked embedded=1.
    for relative in ["src/main.rs", "src/lib.rs"] {
        let flag = read_embedded_flag(&shared_db, relative);
        assert_eq!(
            flag,
            Some(1),
            "expected {relative} to have embedded=1, got {flag:?}"
        );
    }
}

/// Embeddings written by the indexer are binary-compatible with the
/// `deserialize_embedding` helper used by `search_code`.
#[tokio::test]
async fn test_indexer_embedding_blob_roundtrips_through_deserialize() {
    let (_tmp, root, shared_db) = make_tiny_indexable_project().await;
    let dim = 8;
    let embedder: std::sync::Arc<dyn TextEmbedder> = std::sync::Arc::new(MockEmbedder::new(dim));

    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        Some(embedder),
        swissarmyhammer_code_context::noop_reporter(),
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    // Read one row's blob and convert it back to a Vec<f32>.
    let blob: Vec<u8> = {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT embedding FROM ts_chunks WHERE embedding IS NOT NULL LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        blob.len(),
        dim * 4,
        "blob length should be dim*4 bytes (little-endian f32)"
    );

    // Round-trip through the same little-endian f32 layout used by
    // search_code::deserialize_embedding.
    let parsed: Vec<f32> = blob
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(parsed.len(), dim);
    // MockEmbedder returns vec![0.1; dim]
    for v in parsed {
        assert!((v - 0.1).abs() < 1e-6, "expected 0.1 vector, got {v}");
    }
}

/// When the embedder fails on a specific chunk, that chunk row has NULL
/// embedding, other chunks succeed, and the file's `embedded` flag stays
/// at 0. The file is not re-driven by this function until something else
/// flips `ts_indexed` back to 0; the successfully embedded chunks remain
/// searchable in the meantime.
#[tokio::test]
async fn test_indexer_partial_embedding_failure_leaves_file_unembedded() {
    let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

    // Fail on the very first embed_text call. With two tiny files, at
    // least one file will end up with a partially-failed chunk.
    let mock = std::sync::Arc::new(MockEmbedder::with_failures(8, vec![0]));
    let embedder: std::sync::Arc<dyn TextEmbedder> = mock.clone();

    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        Some(embedder),
        swissarmyhammer_code_context::noop_reporter(),
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let total = count_total_chunks(&shared_db);
    let embedded = count_embedded_chunks(&shared_db);
    assert!(total > 0, "expected chunks to be written even on failure");
    assert!(
        embedded < total,
        "expected at least one chunk to have NULL embedding (embedded={embedded}, total={total})"
    );
    assert!(
        embedded > 0,
        "expected the other chunks to still succeed (embedded={embedded})"
    );

    // At least one file must be left with embedded=0 because one of its
    // chunks failed to embed.
    let conn_flags: Vec<(String, i64)> = {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn
            .prepare("SELECT file_path, embedded FROM indexed_files ORDER BY file_path")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert!(
        conn_flags.iter().any(|(_, flag)| *flag == 0),
        "expected at least one file with embedded=0, got: {conn_flags:?}"
    );

    // The mock should have been called at least once per chunk.
    assert!(
        mock.call_count() >= total as usize,
        "embedder should have been driven for every chunk (call_count={}, total={total})",
        mock.call_count()
    );
}

/// When no embedder is provided (e.g. construction or load failed), chunks
/// are still written without embeddings (preserved fallback behavior),
/// and `embedded` stays at 0. As with partial failure, the file is not
/// re-driven by this function — a future invocation with a working
/// embedder is only triggered once `ts_indexed` is flipped back to 0.
#[tokio::test]
async fn test_indexer_no_embedder_still_writes_chunks_without_embeddings() {
    let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        None,
        swissarmyhammer_code_context::noop_reporter(),
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let total = count_total_chunks(&shared_db);
    let embedded = count_embedded_chunks(&shared_db);
    assert!(total > 0, "expected chunks to be written without embedder");
    assert_eq!(
        embedded, 0,
        "no chunks should have embeddings when embedder is absent"
    );

    for relative in ["src/main.rs", "src/lib.rs"] {
        let flag = read_embedded_flag(&shared_db, relative);
        assert_eq!(
            flag,
            Some(0),
            "expected {relative} to have embedded=0 when no embedder, got {flag:?}"
        );
    }
}

// -----------------------------------------------------------------------
// Progress reporter tests for `index_discovered_files_with_embedder`
//
// These tests use a `VecReporter` that records every `IndexProgress`
// event the indexer emits. They run the indexer end-to-end on the same
// tiny two-file workspace fixture used by the embedding tests above so
// we exercise the real chunk + embed code paths, not a stub.
// -----------------------------------------------------------------------

/// A `ProgressReporter` that records every event into a `Mutex<Vec<_>>`
/// so tests can assert on the recorded sequence.
struct VecReporter {
    events: std::sync::Mutex<Vec<swissarmyhammer_code_context::IndexProgress>>,
}

impl VecReporter {
    fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn snapshot(&self) -> Vec<swissarmyhammer_code_context::IndexProgress> {
        self.events.lock().unwrap().clone()
    }
}

impl swissarmyhammer_code_context::ProgressReporter for VecReporter {
    fn report(&self, event: swissarmyhammer_code_context::IndexProgress) {
        self.events.lock().unwrap().push(event);
    }
}

/// The download observer the rebuild-index path attaches to the embedder
/// must forward each synthetic `DownloadEvent` to the reporter as an
/// `IndexProgress::DownloadingModel` carrying the full, untruncated filename
/// and the exact byte counts — no network, no real model, just the mapping
/// seam. This is the unit the rebuild-index path relies on to stream
/// model-download progress before the first `Discovering` event.
#[tokio::test]
async fn download_observer_forwards_events_as_downloading_model_progress() {
    use swissarmyhammer_code_context::IndexProgress;

    let reporter = std::sync::Arc::new(VecReporter::new());
    let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
        reporter.clone();
    let observer = download_observer_for(dyn_reporter);

    // Synthetic download events, exactly as the model loader emits them —
    // a first chunk and the final full-size event. No download happens.
    let file = "models/qwen3-embedding-0.6b/model.safetensors";
    observer(swissarmyhammer_embedding::DownloadEvent::new(
        file,
        0,
        620_000_000,
    ));
    observer(swissarmyhammer_embedding::DownloadEvent::new(
        file,
        620_000_000,
        620_000_000,
    ));

    let events = reporter.snapshot();
    assert_eq!(events.len(), 2, "each DownloadEvent maps to one event");
    match &events[0] {
        IndexProgress::DownloadingModel {
            file: f,
            downloaded_bytes,
            total_bytes,
        } => {
            assert_eq!(f, file, "the full untruncated filename is forwarded");
            assert_eq!(*downloaded_bytes, 0);
            assert_eq!(*total_bytes, 620_000_000);
        }
        other => panic!("expected DownloadingModel, got {other:?}"),
    }
    assert!(
        matches!(
            &events[1],
            IndexProgress::DownloadingModel { downloaded_bytes, total_bytes, .. }
                if *downloaded_bytes == 620_000_000 && *total_bytes == 620_000_000
        ),
        "the final event reports the complete download, got {:?}",
        events[1]
    );
}

/// The end-to-end event sequence must:
/// - Open with a `Discovering` event (the pre-discovery zero-count signal)
/// - Follow with a second `Discovering` carrying the final file count
/// - Emit at least one `Chunking` and at least one `Embedding` event per file
/// - Close with exactly one `Done` event
///
/// We use a tiny two-file Rust workspace so the assertions stay readable.
#[tokio::test]
async fn test_indexer_emits_progress_event_sequence() {
    use swissarmyhammer_code_context::IndexProgress;

    let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

    let reporter = std::sync::Arc::new(VecReporter::new());
    let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
        reporter.clone();

    // Always-succeeding mock embedder so we get a non-empty
    // `chunks_in_batch` value to assert on.
    let embedder: std::sync::Arc<dyn TextEmbedder> = std::sync::Arc::new(MockEmbedder::new(8));

    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        Some(embedder),
        dyn_reporter,
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let events = reporter.snapshot();
    assert!(
        events.len() >= 5,
        "expected at least 5 events (Discovering x2, Chunking, Embedding, Done) for a \
         two-file workspace, got {}: {events:?}",
        events.len()
    );

    // First event is the pre-discovery `Discovering { found: 0 }` signal.
    assert!(
        matches!(events[0], IndexProgress::Discovering { found: 0 }),
        "first event must be Discovering {{ found: 0 }}, got {:?}",
        events[0]
    );

    // Second event is the post-discovery `Discovering { found: N }` signal.
    let discovered_total = match events[1] {
        IndexProgress::Discovering { found } => found,
        ref other => panic!("second event must be a Discovering total, got {other:?}"),
    };
    assert_eq!(
        discovered_total, 2,
        "two-file workspace should discover 2 files, got {discovered_total}"
    );

    // Final event is exactly one `Done`. It must report the same number
    // of files we discovered, and `chunks` must match `count_total_chunks`.
    let last = events.last().expect("non-empty events");
    let total_chunks_in_db = count_total_chunks(&shared_db) as u64;
    match last {
        IndexProgress::Done {
            files,
            chunks,
            elapsed,
        } => {
            assert_eq!(
                *files, discovered_total,
                "Done.files must match discovered count"
            );
            assert_eq!(
                *chunks, total_chunks_in_db,
                "Done.chunks must match the row count in ts_chunks"
            );
            assert!(
                elapsed.as_nanos() > 0,
                "Done.elapsed should be non-zero for a real indexing pass"
            );
        }
        other => panic!("last event must be Done, got {other:?}"),
    }
    // No event after Done.
    let done_count = events
        .iter()
        .filter(|e| matches!(e, IndexProgress::Done { .. }))
        .count();
    assert_eq!(
        done_count, 1,
        "expected exactly one Done event, got {done_count}"
    );

    // Middle events: every `Chunking` event's `done` is monotonically
    // non-decreasing and bounded by `total`. Collect them in order and
    // check.
    let chunking: Vec<(u64, u64)> = events
        .iter()
        .filter_map(|e| match e {
            IndexProgress::Chunking { done, total, .. } => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(
        chunking.len(),
        discovered_total as usize,
        "expected one Chunking event per discovered file"
    );
    let mut prev_done = 0u64;
    for (done, total) in &chunking {
        assert!(
            *done > prev_done,
            "Chunking.done must be strictly increasing — got {done} after {prev_done}"
        );
        assert_eq!(
            *total, discovered_total,
            "Chunking.total must equal the discovered file count"
        );
        assert!(
            *done <= *total,
            "Chunking.done ({done}) must not exceed total ({total})"
        );
        prev_done = *done;
    }
    assert_eq!(
        prev_done, discovered_total,
        "the last Chunking event must report done == total"
    );

    // Embedding events: 1-based batch index, monotonically increasing,
    // each one's chunks_in_batch is the number of chunks for that file.
    let embedding: Vec<(u64, u64, u64)> = events
        .iter()
        .filter_map(|e| match e {
            IndexProgress::Embedding {
                batch,
                batches,
                chunks_in_batch,
            } => Some((*batch, *batches, *chunks_in_batch)),
            _ => None,
        })
        .collect();
    assert_eq!(
        embedding.len(),
        discovered_total as usize,
        "expected one Embedding event per discovered file"
    );
    for (idx, (batch, batches, _)) in embedding.iter().enumerate() {
        assert_eq!(
            *batch,
            (idx + 1) as u64,
            "Embedding.batch must be 1-based and sequential"
        );
        assert_eq!(
            *batches, discovered_total,
            "Embedding.batches must equal the planned batch total (one batch per file)"
        );
    }
}

/// When the dirty-file SQL query fails (e.g. the `indexed_files` table is
/// missing or the connection is otherwise broken), the indexer must still
/// emit a complete lifecycle: pre-discovery `Discovering(0)`, the
/// post-discovery `Discovering(0)` symmetry signal, then the terminal
/// `Done(0, 0, _)`. Without the second `Discovering`, consumers that key
/// off "second Discovering means discovery completed" would never see the
/// signal on this path — so we assert exactly three events here, mirroring
/// the empty-workspace lifecycle.
#[tokio::test]
async fn test_indexer_db_query_failure_still_emits_framing_events() {
    use swissarmyhammer_code_context::{CodeContextWorkspace, IndexProgress};

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let ws = CodeContextWorkspace::open(&root).expect("workspace open");
    let shared_db = ws.shared_db().expect("leader has shared db");

    // Force the dirty-file query to fail by dropping the table it reads.
    // Subsequent `SELECT file_path FROM indexed_files WHERE ts_indexed = 0`
    // returns `Err(rusqlite::Error::SqliteFailure(..))` ("no such table"),
    // which exercises the early-return error branch in
    // `index_discovered_files_with_embedder`.
    {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute("DROP TABLE indexed_files", [])
            .expect("drop indexed_files");
    }

    let reporter = std::sync::Arc::new(VecReporter::new());
    let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
        reporter.clone();
    index_discovered_files_with_embedder(
        &root,
        shared_db,
        None,
        dyn_reporter,
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let events = reporter.snapshot();
    assert_eq!(
        events.len(),
        3,
        "DB-query-failure path must emit exactly Discovering(0), Discovering(0), Done — \
         got {events:?}"
    );
    assert!(
        matches!(events[0], IndexProgress::Discovering { found: 0 }),
        "first event must be pre-discovery Discovering(0), got {:?}",
        events[0]
    );
    assert!(
        matches!(events[1], IndexProgress::Discovering { found: 0 }),
        "second event must be post-discovery Discovering(0) — without it, consumers \
         that key off 'second Discovering means discovery completed' will never see \
         the signal on this error path. Got {:?}",
        events[1]
    );
    match &events[2] {
        IndexProgress::Done {
            files: 0,
            chunks: 0,
            ..
        } => {}
        other => panic!("final event must be Done(0, 0, _), got {other:?}"),
    }
}

/// When the dirty-file set is empty (no files to index), the indexer
/// must still emit the open/close framing events so consumers see a
/// clean lifecycle: `Discovering(0)`, `Discovering(0)`, `Done(0, 0, _)`.
#[tokio::test]
async fn test_indexer_empty_workspace_still_emits_framing_events() {
    use swissarmyhammer_code_context::{CodeContextWorkspace, IndexProgress};

    // Empty temp dir — no source files, so `indexed_files` will be empty
    // after `startup_cleanup` and the dirty-file query returns nothing.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let ws = CodeContextWorkspace::open(&root).expect("workspace open");
    let shared_db = ws.shared_db().expect("leader has shared db");

    let reporter = std::sync::Arc::new(VecReporter::new());
    let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
        reporter.clone();
    index_discovered_files_with_embedder(
        &root,
        shared_db,
        None,
        dyn_reporter,
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let events = reporter.snapshot();
    assert_eq!(
        events.len(),
        3,
        "empty workspace must emit exactly Discovering(0), Discovering(0), Done — got {events:?}"
    );
    assert!(matches!(events[0], IndexProgress::Discovering { found: 0 }));
    assert!(matches!(events[1], IndexProgress::Discovering { found: 0 }));
    match &events[2] {
        IndexProgress::Done {
            files: 0,
            chunks: 0,
            ..
        } => {}
        other => panic!("final event must be Done(0, 0, _), got {other:?}"),
    }
}

/// Single-writer invariant: when the leader's `ShutdownFlag` is already set
/// before the indexing pass starts (the preemption / step-down case), the
/// tree-sitter indexer must stop at the top of the per-file loop and NOT
/// write the whole dirty set — otherwise a preempted old leader keeps
/// writing the shared on-disk DB while the new leader is also writing it.
///
/// We seed several dirty files, set the flag, run the indexer with
/// `embedder = None`, and assert it left every file un-indexed
/// (`ts_indexed = 0`). A pre-set flag is the deterministic worst case: the
/// check at the top of the loop fires on the first iteration.
#[tokio::test]
async fn test_indexer_stops_mid_pass_when_shutdown_flag_set() {
    use swissarmyhammer_code_context::CodeContextWorkspace;

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join("src")).unwrap();
    // Seed several dirty source files so "stops early" is observable.
    let files = ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"];
    for name in files {
        std::fs::write(
            root.join("src").join(name),
            format!("pub fn {}() {{}}\n", name.trim_end_matches(".rs")),
        )
        .unwrap();
    }

    let ws = CodeContextWorkspace::open(&root).expect("workspace open");
    let shared_db = ws.shared_db().expect("leader has shared db");

    // Sanity: startup_cleanup left all files dirty (ts_indexed = 0).
    let dirty_before: i64 = {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 0",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        dirty_before,
        files.len() as i64,
        "all seeded files should be dirty before indexing"
    );

    // Preempt before the pass: set the flag so the loop-top check fires on
    // the first iteration.
    let shutdown = swissarmyhammer_code_context::new_shutdown_flag();
    shutdown.store(true, std::sync::atomic::Ordering::Relaxed);

    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        None,
        swissarmyhammer_code_context::noop_reporter(),
        shutdown,
    )
    .await;

    let still_dirty: i64 = {
        let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 0",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        still_dirty,
        files.len() as i64,
        "a flag set before the pass should stop the indexer on the first \
         iteration, leaving every file un-indexed (got {still_dirty} still dirty)"
    );
}
