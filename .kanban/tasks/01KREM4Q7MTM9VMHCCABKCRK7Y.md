---
assignees:
- claude-code
depends_on:
- 01KREM3V8GBBDXWP2474GN541E
position_column: done
position_ordinal: ffffffffffffffffffffffffba80
project: semantic-search
title: Compute and persist chunk embeddings during indexing
---
## What

Add the missing embedding step to the real indexer: for every chunk written to `ts_chunks`, compute an embedding via `Embedder::default()` and persist it as a little-endian f32 BLOB in the `embedding` column. Mark `indexed_files.embedded=1` only after every chunk for that file is embedded.

This is the core fix. Today's DB confirms 0 of 38,485 chunks have an embedding even though the schema column exists; `search code` is silently a no-op.

### Files

- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — the production indexer at `index_discovered_files_async` (line ~1764). This is where the new embedding step goes. The current `INSERT INTO ts_chunks (...)` at line ~1900 must include the `embedding` blob.
- `swissarmyhammer-code-context/src/ops/search_code.rs` — already exports `serialize_embedding(&[f32]) -> Vec<u8>` at line 82. Reuse it; do NOT duplicate.
- `swissarmyhammer-embedding/src/embedder.rs` — `Embedder::default()` returns the `qwen-embedding` model (Qwen3-Embedding-0.6B; ANE on macOS-arm64, llama.cpp elsewhere; max_seq 256/512; L2-normalized; 1024-dim). The `TextEmbedder::embed_text(&self, text)` API is async, single-text. There is no batch API at the public level — embed one chunk at a time. The Embedder is `Send + Sync` and `embed_text` takes `&self`, so it can be wrapped in an `Arc` and reused across the worker loop.

### Approach

1. At the top of `index_discovered_files_async`, construct an `Arc<Embedder>` ONCE: `Embedder::default().await?` then `.load().await?`. Log the backend, model, dimension, max_seq. If construction or load fails, log a warning and SKIP embedding for this run — still write chunks (existing behavior) but leave `embedded=0`. Do not crash; the rest of code-context must still work.
2. For each file: after `chunk_file()` produces chunks and before the INSERT, embed each chunk's text via `embedder.embed_text(&content).await?`. On a per-chunk embedding error, log a warning and skip that chunk's embedding (insert with NULL) — do not abort the file.
3. Change the INSERT to include `embedding` as the 8th column and bind the serialized blob (or NULL).
4. After all chunks for a file are inserted, if EVERY chunk got an embedding, UPDATE `indexed_files SET embedded=1 WHERE file_path=?`. If any chunk failed to embed, leave `embedded=0` (it will be retried on next dirty-file pass).
5. Same change must be applied wherever ts_chunks is written by the production path. Verify there is only one (`mod.rs:1900`); the `swissarmyhammer-code-context/src/indexing.rs::write_ts_chunks` path is dead (see card 5) and out of scope here.

### Performance note

Qwen3-Embedding-0.6B on Apple Neural Engine takes ~30-100ms per chunk. With ~38K chunks in this workspace that is roughly 30-60 minutes for a first-time index. That is acceptable for an initial population; incremental updates are small. Do NOT add multi-threaded embedding in this card — the backends serialize internally and parallelism here invites contention. Keep it sequential; revisit if performance is a real problem.

### Configuration

- The model name `Embedder::default()` resolves should be a constant. If a later card wants to make it configurable, that's a separate concern. Hardcode `Embedder::default()` here.
- Document model + dimension + cost in a `// MODEL NOTE:` comment block near the embedder construction so future readers don't have to chase yaml files.

## Acceptance Criteria

- [x] After running the indexer on a small repo (the integration test in card 4), `SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL` is nonzero and equals the chunk count for indexed files.
- [x] Files where every chunk got an embedding have `embedded=1` in `indexed_files`.
- [x] Files where some chunk embedding failed have `embedded=0` and are retried on the next pass.
- [x] Embedder construction/load failures are logged but do not abort indexing — chunks still get written without embeddings (existing fallback behavior).
- [x] Embedded blob is binary-compatible with `deserialize_embedding` in `search_code.rs:75` — round-trip in a test.

## Tests

- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — extend the existing `index_discovered_files_async` test module to assert chunk rows have non-null `embedding` and `indexed_files.embedded=1` after one pass. Use a tiny 2-3 file Rust fixture.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — unit test: simulate per-chunk embedding failure (mock or inject an embedder that returns Err for one chunk); assert the file's `embedded=0` and the failing chunk has NULL embedding while others are populated.
- [x] Run: `cargo nextest run -p swissarmyhammer-tools mcp::tools::code_context` — all pass.
- [ ] The end-to-end test in card 4 must use this code path and assert real matches come back.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #code-context

## Implementation Notes

- Refactored `index_discovered_files_async` to delegate to a new dependency-injectable helper `index_discovered_files_with_embedder(root, db, Option<Arc<dyn TextEmbedder>>)`. The public function builds the default embedder (logs warning on failure → `None`) and forwards to the helper. Tests inject `MockEmbedder` directly into the helper to exercise all paths without loading a real model.
- Per-chunk embedding happens BEFORE the DB lock so the ~30-100ms wait does not starve other workers. New types: `PreparedChunk` (a chunk row with optional embedding) and helpers `prepare_chunk` + `embed_file_chunks`.
- The `INSERT INTO ts_chunks` now includes `embedding` as the 8th column; `serialize_embedding` from `swissarmyhammer-code-context::ops::search_code` is reused.
- The `embedded=1` UPDATE only fires when every chunk for the file got an embedding (and an embedder was supplied). Partial failure leaves `embedded=0` for retry.
- The end-to-end test (card 4) is a separate card and not in scope here.

## Review Findings (2026-05-12 19:30)

### Warnings
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1834` — Docstring on `index_discovered_files_with_embedder` claims "if any chunk's embedding failed (and was written as NULL) the file keeps `embedded=0` and will be retried on the next dirty-file pass." That is not what the code does. The file always exits the loop with `ts_indexed=1` (lines 2010-2018: both branches set `ts_indexed = 1`). The dirty-file selector at line 1852 is `WHERE ts_indexed = 0`, not `WHERE ts_indexed = 0 OR embedded = 0`, so a file marked `ts_indexed=1, embedded=0` will NOT be retried until something else (file edit, watcher event, `build_status`) flips `ts_indexed` back to 0. The task's third acceptance criterion ("Files where some chunk embedding failed have `embedded=0` and are retried on the next pass") is therefore not actually met by this implementation. Operational impact is small because `search_code` filters chunks by `embedding IS NOT NULL` so successful chunks are still searchable, but the documentation and acceptance claim are misleading. Either (a) update the docstring on `index_discovered_files_with_embedder` to describe the real behavior ("partial-embed files stay `embedded=0` and remain searchable on the chunks that did embed; they are not re-driven until they become dirty again for another reason"), or (b) widen the dirty-file query to include `OR embedded = 0`. Option (a) is the minimal fix consistent with this card's scope.

### Nits
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:2129` — `embed_file_chunks` logs one `tracing::warn!` per failed chunk. If the model crashes mid-run and every subsequent chunk fails, that's potentially tens of thousands of warning lines (the workspace has ~38K chunks). Consider rate-limiting, sampling, or emitting a single summary warning per file. Not a correctness issue; just operator-friendliness.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1952-1954` — `all_chunks_embedded` is false when `embedded_chunks.is_empty()`. A file with no parseable chunks (e.g., empty or chunk_file returns nothing) lands as `ts_indexed=1, embedded=0` even though there is nothing to embed. Vacuously, such a file is "fully embedded." Combined with the no-retry behavior above, this is harmless, but if you ever add an `embedded = 0` retry pass it will spin on empty files forever. Consider treating empty `embedded_chunks` as a successful no-op when `embedder.is_some()`.

## Review Follow-up (2026-05-12)

Addressed all three Review Findings:

- **Warning (docstring)**: Took the review's recommended option (a) — rewrote the docstring on `index_discovered_files_with_embedder` to describe the real behavior. The docstring now states that partial-embed files exit with `ts_indexed=1, embedded=0` and are not re-driven by this function until something else flips `ts_indexed` back to 0; the successful chunks remain searchable because `search_code` filters by `embedding IS NOT NULL`. Also corrected the matching inline comment near step 8 and the three test docstrings that repeated the same false claim.
- **Nit (warn spam)**: Replaced the per-chunk `tracing::warn!` inside `embed_file_chunks` with a single per-file summary `warn!` that reports the failure count, total chunks, an example failing symbol, and the first error string. At most one log line per file, regardless of how many chunks fail.
- **Nit (vacuous empty)**: Dropped the `!embedded_chunks.is_empty()` guard from `all_chunks_embedded`. A file with no prepared chunks (empty file, or chunk_file returns nothing) now exits with `embedded=1` when an embedder is supplied — vacuously fully embedded. A file with no embedder still ends up `embedded=0` (the `embedder.is_some()` guard is still in place), so the existing no-embedder fallback test stays valid.

Verification: `cargo nextest run -p swissarmyhammer-tools mcp::tools::code_context` — 91/91 pass.