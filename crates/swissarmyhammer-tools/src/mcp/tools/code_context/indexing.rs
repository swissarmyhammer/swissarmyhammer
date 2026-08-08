//! The tree-sitter indexing pass, and the embedding pass that rides along with
//! it.
//!
//! This is the ONE production indexer: the MCP server bootstrap, the file
//! watcher, and the synchronous `rebuild index` op all drive
//! [`index_discovered_files_async`]. It chunks each dirty file with the real
//! tree-sitter chunker, writes the chunks with their symbol paths, and embeds
//! them when an embedder is available.

use std::path::Path;

/// Trigger incremental tree-sitter indexing on dirty files.
///
/// Constructs the default embedding model (qwen-embedding) once for the run
/// and delegates to [`index_discovered_files_with_embedder`]. If the embedder
/// fails to construct or load, indexing still runs but chunk embeddings are
/// skipped — files keep `embedded=0` so the next pass can retry them.
///
/// Uses the leader's single shared write connection for all DB operations.
/// The mutex is locked only for each DB call — file I/O and parsing happen
/// without holding the lock so other writers (LSP worker, watcher) can interleave.
///
/// Exposed `pub` so end-to-end integration tests (notably
/// `tests/integration/semantic_search_e2e.rs`) can drive the real production
/// indexer over a temp workspace. The function is otherwise only called from
/// within this crate (the MCP server bootstrap and the file watcher).
///
/// The `shutdown` flag is the leader's step-down signal: when set, the indexer
/// stops at the top of its per-file loop so a preempted old leader stops writing
/// the shared DB promptly. One-shot/non-leader callers pass a fresh never-set
/// flag (`new_shutdown_flag()`).
///
/// Returns an [`IndexRunStats`] summarising the run. Callers that drive the
/// indexer purely for side effects (the bootstrap pass, the file watcher)
/// may ignore the value; the synchronous `rebuild index` MCP op uses it to
/// build its response payload.
pub async fn index_discovered_files_async(
    workspace_root: &Path,
    db: swissarmyhammer_code_context::SharedDb,
    reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
    shutdown: swissarmyhammer_code_context::ShutdownFlag,
) -> swissarmyhammer_code_context::IndexRunStats {
    let embedder = build_default_embedder(&reporter).await;
    index_discovered_files_with_embedder(workspace_root, db, embedder, reporter, shutdown).await
}

/// Build a download observer that forwards each model-download
/// [`DownloadEvent`](swissarmyhammer_embedding::DownloadEvent) to `reporter` as an
/// [`IndexProgress::DownloadingModel`](swissarmyhammer_code_context::IndexProgress::DownloadingModel)
/// event, so a first-run index's embedding-model download surfaces as
/// `notifications/progress` before the first `Discovering` event instead of
/// minutes of silence.
///
/// The full untruncated filename and the running/total byte counts pass through
/// verbatim; the reporter's wire mapping keeps `progress` monotonic (a download
/// advances neither the file nor the batch counter).
pub(super) fn download_observer_for(
    reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
) -> swissarmyhammer_embedding::DownloadObserver {
    use swissarmyhammer_code_context::IndexProgress;
    std::sync::Arc::new(move |event: swissarmyhammer_embedding::DownloadEvent| {
        reporter.report(IndexProgress::DownloadingModel {
            file: event.file().to_string(),
            downloaded_bytes: event.downloaded_bytes(),
            total_bytes: event.total_bytes(),
        });
    })
}

/// Construct the default embedder and load it.
///
/// Returns `None` and logs a warning on construction or load failure. The
/// indexer treats this as a soft fallback — it still writes chunks (without
/// embeddings), leaving `indexed_files.embedded=0` so a future pass can
/// retry once the model is available.
///
/// MODEL NOTE: The default model is `qwen-embedding` (Qwen3-Embedding-0.6B),
/// a 1024-dim L2-normalized embedder. On macOS-arm64 it runs on the Apple
/// Neural Engine; elsewhere it falls back to llama.cpp. Max sequence is
/// 256 (ANE) or 512 (llama). The embedder is `Send + Sync`, and a single
/// shared `Arc<dyn TextEmbedder>` is reused across all chunks in an
/// indexing pass — see swissarmyhammer-embedding/src/embedder.rs.
///
/// Performance: per-chunk embedding on ANE is ~30-100ms, so a fresh full
/// index is minutes-to-tens-of-minutes for large workspaces. We embed
/// sequentially because the backends serialize internally; adding worker
/// parallelism here invites contention without throughput gains.
/// Returns whether the named environment variable is set to a truthy value.
///
/// Truthy means `1`, `true`, `yes`, or `on` (case-insensitive). Any other
/// value — including unset, empty, or `0`/`false` — is false. Used for opt-in
/// boolean toggles like `SAH_DISABLE_EMBEDDING`.
fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(super) async fn build_default_embedder(
    reporter: &std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
) -> Option<std::sync::Arc<dyn model_embedding::TextEmbedder>> {
    use model_embedding::TextEmbedder as _;

    // Escape hatch: skip chunk embeddings entirely when `SAH_DISABLE_EMBEDDING`
    // is set to a truthy value. This selects the same `None`-embedder path the
    // indexer already takes on model-load failure — chunks are written without
    // an `embedding` blob and files keep `embedded=0`. It exists for two real
    // use cases: CI/headless indexing where the multi-GB model is unwanted, and
    // tests that exercise the indexing/progress contract without paying a cold
    // model load (which on a clean machine downloads gigabytes from HuggingFace
    // and otherwise dominates the run). Semantic `search code` is unavailable
    // for chunks indexed in this mode until a later pass embeds them.
    if env_flag_enabled("SAH_DISABLE_EMBEDDING") {
        tracing::info!(
            "code-context: SAH_DISABLE_EMBEDDING set — skipping chunk embeddings this pass"
        );
        return None;
    }

    // Attach a download observer so a first-run model download streams
    // DownloadingModel progress through `reporter` — before the first
    // Discovering event — instead of minutes of silence. Use
    // `with_download_observer` (not `default`) because on the ANE backend the
    // model resolves and downloads during construction, so an observer attached
    // afterwards could never see it.
    let embedder = match swissarmyhammer_embedding::Embedder::with_download_observer(
        swissarmyhammer_embedding::DEFAULT_MODEL_NAME,
        download_observer_for(std::sync::Arc::clone(reporter)),
    )
    .await
    {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "code-context: failed to construct default embedder — chunk embeddings will be skipped this pass"
            );
            return None;
        }
    };
    if let Err(err) = embedder.load().await {
        tracing::warn!(
            error = %err,
            "code-context: failed to load embedding model — chunk embeddings will be skipped this pass"
        );
        return None;
    }
    tracing::info!(
        backend = embedder.backend_name(),
        model = embedder.model_name(),
        dimension = ?embedder.embedding_dimension(),
        max_sequence_length = embedder.max_sequence_length(),
        "code-context: loaded chunk embedder"
    );
    Some(std::sync::Arc::new(embedder) as std::sync::Arc<dyn model_embedding::TextEmbedder>)
}

/// Trigger incremental tree-sitter indexing on dirty files with a supplied
/// embedder.
///
/// This is the dependency-injectable form of [`index_discovered_files_async`].
/// Tests pass a mock embedder; production passes the model resolved by
/// `Embedder::default()`.
///
/// When `embedder` is `Some`, every chunk text is embedded and the resulting
/// little-endian f32 blob is written to the `embedding` column. A file is
/// flagged `embedded=1` only when every one of its chunks got an embedding
/// (a file with no chunks is vacuously fully embedded). If any chunk's
/// embedding failed it is written with a NULL `embedding` blob and the file
/// keeps `embedded=0`.
///
/// Important: the dirty-file selector is `WHERE ts_indexed = 0`, so a file
/// that exits this function with `ts_indexed=1, embedded=0` is NOT re-driven
/// on subsequent calls until something else (a file edit picked up by the
/// watcher, `rebuild_index`, etc.) flips `ts_indexed` back to 0. The
/// successfully embedded chunks remain searchable in the meantime — the
/// search path filters by `embedding IS NOT NULL`.
///
/// When `embedder` is `None` the indexer behaves as it did before chunk
/// embeddings existed: chunks are written without an embedding blob and
/// `embedded` stays at 0.
///
/// The `shutdown` flag is checked at the top of the per-file loop: when set
/// (the leader has stepped down / been preempted), the pass stops early and
/// returns the partial [`IndexRunStats`] so the old leader stops writing the
/// shared DB before the new leader becomes the sole writer.
pub(crate) async fn index_discovered_files_with_embedder(
    workspace_root: &Path,
    db: swissarmyhammer_code_context::SharedDb,
    embedder: Option<std::sync::Arc<dyn model_embedding::TextEmbedder>>,
    reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
    shutdown: swissarmyhammer_code_context::ShutdownFlag,
) -> swissarmyhammer_code_context::IndexRunStats {
    use std::sync::Arc;
    use swissarmyhammer_code_context::{IndexProgress, IndexRunStats};
    use swissarmyhammer_treesitter::{chunk::chunk_file, LanguageRegistry, ParsedFile};

    let run_start = std::time::Instant::now();

    // Emit a `Discovering { found: 0 }` event before discovery starts so
    // consumers can show a "discovering files…" indicator immediately. The
    // dirty-file query below is "discovery" for this incremental indexer —
    // it pulls the set of files that need indexing from `indexed_files`.
    reporter.report(IndexProgress::Discovering { found: 0 });

    // Query all dirty files from the DB (populated by startup_cleanup)
    let dirty_files: Vec<String> = {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        let result: Result<Vec<String>, rusqlite::Error> = (|| {
            let mut stmt =
                conn.prepare("SELECT file_path FROM indexed_files WHERE ts_indexed = 0")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })();
        match result {
            Ok(files) => files,
            Err(e) => {
                tracing::warn!("code-context: failed to query dirty files: {}", e);
                // Emit the post-discovery `Discovering { found: 0 }` event
                // even on this error path so the event lifecycle stays
                // symmetric: every run emits two `Discovering` events
                // (pre- and post-discovery) before the terminal `Done`.
                // Consumers that key off "the second Discovering means
                // discovery completed" need this signal on every path.
                reporter.report(IndexProgress::Discovering { found: 0 });
                let elapsed = run_start.elapsed();
                reporter.report(IndexProgress::Done {
                    files: 0,
                    chunks: 0,
                    elapsed,
                });
                return IndexRunStats {
                    files: 0,
                    chunks: 0,
                    elapsed,
                };
            }
        }
    };

    // Emit the final discovery count now that we know the total file set.
    reporter.report(IndexProgress::Discovering {
        found: dirty_files.len() as u64,
    });

    if dirty_files.is_empty() {
        tracing::info!("code-context: no dirty files to index");
        let elapsed = run_start.elapsed();
        reporter.report(IndexProgress::Done {
            files: 0,
            chunks: 0,
            elapsed,
        });
        return IndexRunStats {
            files: 0,
            chunks: 0,
            elapsed,
        };
    }

    tracing::info!(
        "code-context: indexing {} dirty files incrementally",
        dirty_files.len()
    );

    let lang_registry = LanguageRegistry::global();
    let total = dirty_files.len();
    let mut indexed = 0u64;
    let mut total_chunks = 0u64;
    // 1-based batch counter for `Embedding` events. Each file's chunks are
    // treated as one batch (the indexer embeds chunk-by-chunk inside
    // `embed_file_chunks`, but from the consumer's point of view the
    // file-level grouping is the meaningful batch boundary).
    let mut batch_index: u64 = 0;
    let total_batches: u64 = total as u64;

    for relative_path in &dirty_files {
        // Single-writer invariant: stop the pass promptly once the leader has
        // stepped down. A preempted old leader must not keep writing the shared
        // on-disk DB through its remaining dirty set while the new leader writes
        // it too (the SharedDb mutex is per-process and does not serialize two
        // processes). Mirrors the per-iteration check in `run_lsp_indexing_loop`.
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!(
                "code-context: tree-sitter indexer stopping mid-pass (stepped down) — \
                 {} of {} dirty files indexed",
                indexed,
                total
            );
            break;
        }

        let file_path = workspace_root.join(relative_path);

        // 1. Detect language (no DB needed)
        let lang_config = match lang_registry.detect_language(&file_path) {
            Some(config) => config,
            None => {
                let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
                indexed += 1;
                continue;
            }
        };

        // 2. Read and parse file (no DB needed)
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => {
                let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
                indexed += 1;
                continue;
            }
        };

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&lang_config.language()).is_err() {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());
            let _ = conn.execute(
                "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                rusqlite::params![relative_path],
            );
            indexed += 1;
            continue;
        }

        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => {
                let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
                indexed += 1;
                continue;
            }
        };

        let content_hash: [u8; 16] = md5::compute(content.as_bytes()).into();

        let parsed_file = Arc::new(ParsedFile::new(
            file_path.clone(),
            content,
            tree,
            content_hash,
        ));

        // 3. Extract semantic chunks (no DB needed)
        let chunks = chunk_file(parsed_file.clone());
        // The `done` value here is post-increment so the first file reports
        // `done: 1`. `indexed` is incremented at the bottom of the loop
        // body, so it currently holds the count of files completed before
        // this one — add 1 to get the 1-based "files chunked so far" value
        // a consumer expects.
        reporter.report(IndexProgress::Chunking {
            file: file_path.clone(),
            done: indexed + 1,
            total: total as u64,
        });

        // 4. Embed chunks BEFORE acquiring the DB lock. embed_text is async
        //    and may take 30-100ms per chunk on ANE; holding the connection
        //    mutex across that wait would starve other workers.
        let embedded_chunks =
            embed_file_chunks(&chunks, &parsed_file, embedder.as_deref(), relative_path).await;
        batch_index += 1;
        reporter.report(IndexProgress::Embedding {
            batch: batch_index,
            batches: total_batches,
            chunks_in_batch: embedded_chunks.len() as u64,
        });
        // A file is "fully embedded" when an embedder was supplied and every
        // prepared chunk has a Some(embedding). A file with zero chunks (e.g.
        // an empty file or one chunk_file rejected) is vacuously fully
        // embedded — there is nothing to embed, so we should not pretend the
        // file is in a partial-failure state.
        let all_chunks_embedded =
            embedder.is_some() && embedded_chunks.iter().all(|c| c.embedding.is_some());

        // 5. Lock DB once for the entire write batch for this file
        {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());

            // Clear old chunks
            let _ = conn.execute(
                "DELETE FROM ts_chunks WHERE file_path = ?",
                rusqlite::params![relative_path],
            );

            // Write new chunks
            let mut chunks_written = 0u64;
            for chunk in &embedded_chunks {
                let blob = chunk
                    .embedding
                    .as_deref()
                    .map(swissarmyhammer_code_context::serialize_embedding);
                if conn.execute(
                    "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path, embedding)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        relative_path,
                        chunk.start_byte,
                        chunk.end_byte,
                        chunk.start_line,
                        chunk.end_line,
                        chunk.text,
                        &chunk.symbol_path,
                        blob,
                    ],
                ).is_ok() {
                    chunks_written += 1;
                }
            }

            // 6. Extract symbols from chunks
            let _ = swissarmyhammer_code_context::ensure_ts_symbols(&conn, relative_path);

            // 7. Generate and write call edges
            let source_text = parsed_file.source.as_str();
            let language = lang_config.language();
            if let Ok(edges) = swissarmyhammer_code_context::generate_ts_call_edges(
                &conn,
                relative_path,
                source_text,
                language,
            ) {
                let _ = swissarmyhammer_code_context::write_ts_edges(&conn, relative_path, &edges);
            }

            // 8. Mark file as ts_indexed. Mark embedded=1 only when every
            //    chunk for the file got an embedding (or there were no chunks
            //    to embed); partial failure leaves embedded=0. The file is
            //    not re-driven by this function until ts_indexed is flipped
            //    back to 0 by something else — see the function docstring.
            if all_chunks_embedded {
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1, embedded = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
            }

            total_chunks += chunks_written;
        }

        indexed += 1;

        if indexed.is_multiple_of(100) {
            tracing::info!(
                "code-context: indexed {}/{} files ({} chunks so far)",
                indexed,
                total,
                total_chunks
            );
        }

        // Yield to let other async tasks run
        tokio::task::yield_now().await;
    }

    // Summary
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    let chunk_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
        .unwrap_or(0);
    let embedded_chunk_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let symbol_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM lsp_symbols", [], |r| r.get(0))
        .unwrap_or(0);
    let edge_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))
        .unwrap_or(0);
    tracing::info!(
        "code-context: indexing complete — {}/{} files, {} chunks ({} embedded), {} symbols, {} call edges",
        indexed,
        total,
        chunk_count,
        embedded_chunk_count,
        symbol_count,
        edge_count
    );
    // Drop the DB lock before emitting the final event so consumer
    // reporters that touch the database (e.g. status snapshots) cannot
    // deadlock against our own connection guard.
    drop(conn);
    let elapsed = run_start.elapsed();
    reporter.report(IndexProgress::Done {
        files: indexed,
        chunks: total_chunks,
        elapsed,
    });
    IndexRunStats {
        files: indexed,
        chunks: total_chunks,
        elapsed,
    }
}

/// A chunk row prepared for insertion into `ts_chunks`, with an optional
/// pre-computed embedding vector.
struct PreparedChunk {
    start_byte: i32,
    end_byte: i32,
    start_line: i32,
    end_line: i32,
    text: String,
    symbol_path: String,
    /// `Some` when the chunk text was successfully embedded; `None` when
    /// embedding was unavailable (no embedder) or returned an error.
    embedding: Option<Vec<f32>>,
}

/// Convert a chunk into [`PreparedChunk`] form (no embedding), or return
/// `None` if the chunk doesn't have parseable byte ranges.
fn prepare_chunk(
    chunk: &swissarmyhammer_treesitter::chunk::SemanticChunk,
    parsed_file: &swissarmyhammer_treesitter::ParsedFile,
) -> Option<PreparedChunk> {
    use swissarmyhammer_treesitter::ChunkSource;
    let text = chunk.source.content()?.to_string();
    let (start_byte, end_byte) = match &chunk.source {
        ChunkSource::Parsed {
            start_byte,
            end_byte,
            ..
        } => (*start_byte, *end_byte),
        _ => return None,
    };
    let start_line = parsed_file.source[..start_byte].matches('\n').count() as i32;
    let end_line = parsed_file.source[..end_byte].matches('\n').count() as i32;
    Some(PreparedChunk {
        start_byte: start_byte as i32,
        end_byte: end_byte as i32,
        start_line,
        end_line,
        text,
        symbol_path: chunk.symbol_path(),
        embedding: None,
    })
}

/// Prepare every chunk for insertion, embedding each one if an embedder was
/// provided. Per-chunk embedding errors leave that chunk's `embedding` as
/// `None`; the rest of the file continues. At most one summary warning is
/// emitted per file (with the failure count and an example symbol + error)
/// so that a model crash mid-run does not produce one log line per chunk
/// across tens of thousands of chunks.
async fn embed_file_chunks(
    chunks: &[swissarmyhammer_treesitter::chunk::SemanticChunk],
    parsed_file: &swissarmyhammer_treesitter::ParsedFile,
    embedder: Option<&dyn model_embedding::TextEmbedder>,
    relative_path: &str,
) -> Vec<PreparedChunk> {
    let mut prepared = Vec::with_capacity(chunks.len());
    let mut failed_count: usize = 0;
    let mut first_failure: Option<(String, String)> = None;
    for chunk in chunks {
        let Some(mut pc) = prepare_chunk(chunk, parsed_file) else {
            continue;
        };
        if let Some(emb) = embedder {
            match emb.embed_text(&pc.text).await {
                Ok(result) => pc.embedding = Some(result.embedding().to_vec()),
                Err(err) => {
                    failed_count += 1;
                    if first_failure.is_none() {
                        first_failure = Some((pc.symbol_path.clone(), err.to_string()));
                    }
                }
            }
        }
        prepared.push(pc);
    }
    if failed_count > 0 {
        let (symbol, err) = first_failure.unwrap_or_default();
        tracing::warn!(
            file = %relative_path,
            failed_chunks = failed_count,
            total_chunks = prepared.len(),
            first_failed_symbol = %symbol,
            first_error = %err,
            "code-context: chunk embedding failed for one or more chunks — those chunks were inserted with NULL embedding"
        );
    }
    prepared
}
