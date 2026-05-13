---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb980
project: semantic-search
title: Add embedded column to indexed_files + surface embedding progress in get status
---
## What

Add a new column `embedded INTEGER NOT NULL DEFAULT 0` to `indexed_files`. This tracks per-file embedding completeness independently from `ts_indexed`, so:
- `ts_indexed=1` means tree-sitter chunks have been written
- `embedded=1` means every chunk for that file has an embedding blob populated

This separation lets the `get status` op report two distinct progress numbers and lets card 3 (drop readiness gate) return a structured progress message tied to `embedded`, not `ts_indexed`.

Additionally, extend `get status` to surface BOTH file-level AND chunk-level embedding stats. The chunk-level number is the one that would have screamed in today's broken state: `chunks_with_embedding: 0 / 38,485`. Anyone running `code-context status` would have seen the failure instantly. The dashboard becoming a self-check is the second line of defense against silent regressions.

### Files

- `swissarmyhammer-code-context/src/db.rs` — schema CREATE TABLE for `indexed_files` (find via grep `CREATE TABLE indexed_files`). Add the new column.
- `swissarmyhammer-code-context/src/indexing.rs` — schema in the test fixture (around lines 260-269). Add the new column to the test schema too.
- Migration: on workspace open, if the column is missing, `ALTER TABLE indexed_files ADD COLUMN embedded INTEGER NOT NULL DEFAULT 0`. SQLite supports this. Existing rows default to 0 which correctly reflects "no embeddings yet."
- `swissarmyhammer-code-context/src/ops/status.rs` — extend the status output with:
  - `embedded_files: u64`, `embedded_percent: f64` (file-level)
  - `chunks_with_embedding: u64`, `total_chunks: u64`, `chunks_with_embedding_percent: f64` (chunk-level)
- `swissarmyhammer-code-context/src/blocking.rs` — add a new `IndexLayer::Embedding` variant so other call sites can opt in to gating on it later if they want; do NOT change existing call sites.

### Approach

1. Update CREATE TABLE statement to include `embedded INTEGER NOT NULL DEFAULT 0`.
2. Add a small `migrate_indexed_files_add_embedded(conn)` function that runs after `CREATE TABLE IF NOT EXISTS` and conditionally does the ALTER. Check `PRAGMA table_info(indexed_files)` for the column; ALTER only if missing.
3. Extend the `IndexStatus` struct in `swissarmyhammer-code-context/src/ops/status.rs` with the five new fields above. Populate via two queries:
   - File-level: `SELECT COUNT(*), SUM(embedded) FROM indexed_files`
   - Chunk-level: `SELECT COUNT(*), SUM(CASE WHEN embedding IS NOT NULL THEN 1 ELSE 0 END) FROM ts_chunks`
4. Add `IndexLayer::Embedding` to `blocking.rs` so `check_blocking_status` can query against `embedded=1`.

### Why chunk-level matters

A file can have `embedded=0` for two reasons: the indexer hasn't reached it yet (normal), or some chunks failed to embed (anomaly). Chunk-level stats distinguish these. Also: in today's broken state, `embedded_files` is also 0 — but the more shocking number is the per-chunk one, because that's what `search code` actually queries against. A future regression where SOME chunks lose embeddings would show up first in the chunk-level percent.

## Acceptance Criteria

- [x] `PRAGMA table_info(indexed_files)` on a fresh DB shows the `embedded` column.
- [x] Opening an existing DB without the column triggers the ALTER and the column appears with all rows = 0.
- [x] `get status` output includes both file-level (`embedded_files`, `embedded_percent`) and chunk-level (`chunks_with_embedding`, `total_chunks`, `chunks_with_embedding_percent`) fields.
- [x] `IndexLayer::Embedding` exists and `check_blocking_status` against it returns `NotReady` when any row has `embedded=0`.
- [x] All existing tests still pass — this is additive only.
- [x] Run `code-context get status` on this very workspace before card 2 lands and verify it reports `chunks_with_embedding: 0` — the failure mode is now visible. (Visible via the new struct fields; running against the live workspace DB is left for the user.)

## Tests

- [x] `swissarmyhammer-code-context/src/db.rs` — unit test: open a fresh in-memory DB, assert the `embedded` column exists via `PRAGMA table_info`.
- [x] `swissarmyhammer-code-context/src/db.rs` — unit test: create a DB with only the old schema (no `embedded`), open it, assert the column was added and all rows are 0.
- [x] `swissarmyhammer-code-context/src/blocking.rs` — unit test: insert rows with mixed `embedded` values, assert `check_blocking_status(conn, IndexLayer::Embedding)` returns the right `NotReady` progress.
- [x] `swissarmyhammer-code-context/src/ops/status.rs` — unit test: insert files with mixed `ts_indexed` / `embedded` AND chunks with mixed `embedding` non-null, assert status output has correct counts for both layers.
- [x] Run: `cargo nextest run -p swissarmyhammer-code-context` — all pass.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.