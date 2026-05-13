---
assignees:
- claude-code
depends_on:
- 01KREM3V8GBBDXWP2474GN541E
position_column: done
position_ordinal: ffffffffffffffffffffffffbb80
project: semantic-search
title: Drop readiness gate from search code; add progress message to result
---
## What

`search code` currently returns the bare string `"Index not ready — X/Y files indexed (Z% complete). Please retry shortly."` instead of any matches, because `check_ts_readiness` in `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1244` bails before the query runs. Drop the gate from `search code` only (do NOT touch the other nine call sites). Always run the search against whatever embeddings exist. Extend `SearchCodeResult` with an optional progress message so callers know the index is still building and can decide to retry.

This is strategy 2(a). The reason the gate fires on every fresh CLI run: `workspace.open()` calls `startup_cleanup()` which re-discovers files and resets some to `ts_indexed=0` before the worker catches up. The DB outside the running process shows 1590/1590; the running process briefly sees 1547/1590.

### Files

- `swissarmyhammer-code-context/src/ops/search_code.rs` — extend `SearchCodeResult` with a new field. Approach: add `progress: Option<IndexingProgress>` where `IndexingProgress` is a small struct `{ embedded_files: u64, total_files: u64, embedded_percent: f64, message: String }`. The struct should be `Debug, Clone, Serialize`. `progress` is `Some(...)` when `embedded_files < total_files`, otherwise `None`. Compute it from `indexed_files` inside `search_code` (or a helper) so callers always see a consistent view.
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs::execute_search_code` (line ~1474) — remove the `check_ts_readiness` call (line ~1529). The op now always returns a `SearchCodeResult` via `json_result(&result)`.
- Do NOT remove `check_ts_readiness` or `check_blocking_status` — nine other ops still use them. Only delete the call from `execute_search_code`.

### Approach

1. Add an `IndexingProgress` struct in `swissarmyhammer-code-context/src/ops/search_code.rs` (same module as `SearchCodeResult`). Public, `Debug, Clone, Serialize`. Fields: `embedded_files: u64`, `total_files: u64`, `embedded_percent: f64`, `message: String`.
2. Add `pub progress: Option<IndexingProgress>` to `SearchCodeResult` (also `Serialize`).
3. In `search_code()` after the existing query, run one extra count query: `SELECT COUNT(*), SUM(embedded) FROM indexed_files`. If they differ, populate `progress` with a human-readable message like: `"Embedding still in progress: 1547/1590 files (97%). Results may be incomplete — retry shortly for full coverage."`.
4. In `execute_search_code`, delete the `if let Some(progress) = check_ts_readiness(&ws)? { return Ok(progress); }` block. Run the search unconditionally.
5. The `SearchCodeResult` already serializes via `json_result`, so the new field flows out automatically. Verify the CLI output mode shows the progress message clearly when present (the CLI prints `RawContent::Text` items in `code-context-cli/src/commands/ops.rs:430`; the JSON pretty-print path covers the structured field).

## Acceptance Criteria

- [x] Calling `search code` when `embedded_files < total_files` returns a populated `SearchCodeResult` (matches may be empty if nothing is embedded yet) with `progress: Some(...)` set, NOT the old `"Index not ready"` string.
- [x] Calling `search code` when `embedded_files == total_files` returns `progress: None`.
- [x] The other nine ops that use `check_ts_readiness` still gate as before (unchanged behavior).
- [x] CLI `code-context search code --query "..."` no longer prints the "Index not ready" string when the DB has any embedded chunks.
- [x] Follower processes that open against a legacy on-disk schema (no `embedded` column) successfully migrate the column so subsequent queries against `embedded` do not fail.

## Tests

- [x] `swissarmyhammer-code-context/src/ops/search_code.rs` — unit test: DB with 5 files of which 3 have `embedded=1`. Insert an embedded chunk in one of the embedded files. Call `search_code`. Assert: matches.len() == 1 AND `progress.is_some()` AND `progress.embedded_files == 3` AND `progress.total_files == 5`.
- [x] `swissarmyhammer-code-context/src/ops/search_code.rs` — unit test: DB fully embedded. Assert `progress.is_none()`.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — unit test: call `execute_search_code` against a workspace where files exist but no embeddings yet; assert the result is a normal `SearchCodeResult` (with progress populated, matches empty), NOT a "not ready" placeholder.
- [x] `swissarmyhammer-code-context/src/workspace.rs::test_follower_migrates_legacy_schema_on_open` — locks in the leader/follower migration gap fix: drops the `embedded` column under a live leader to simulate an old-code leader, opens a follower, and verifies the column is restored on disk and the follower's read-only connection can query it.
- [x] `swissarmyhammer-code-context/src/workspace.rs::test_migrate_legacy_schema_is_idempotent` — running the migration twice leaves a single `embedded` column.
- [x] `swissarmyhammer-code-context/src/workspace.rs::test_migrate_legacy_schema_swallows_open_failure` — best-effort behaviour when the write open fails (bogus path).
- [x] Run: `cargo nextest run -p swissarmyhammer-code-context -p swissarmyhammer-tools` — all pass (1964/1964 as of fix).

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #code-context

## Implementation Notes

- The dispatch-layer test was made testable without loading a real embedding model by factoring out an inner `search_code_with_query_embedding(args, context, query_embedding)` helper that takes a caller-supplied embedding vector. `execute_search_code` constructs the real embedder and then delegates. This keeps the test fast and deterministic while still proving the gate is gone end-to-end at the dispatch boundary.
- The `IndexingProgress` is exported from the `swissarmyhammer-code-context` crate root alongside the other `search_code` types.
- **Follow-up bug fix (from review feedback):** the prior card added `migrate_indexed_files_add_embedded` inside `create_schema`, which only runs on the leader path. Followers opening against a legacy on-disk schema would then fail any query that touched the new `embedded` column. Fix: factored the migration into a crate-public `db::migrate_indexed_files` helper, and added a `migrate_legacy_schema_if_writable(db_path)` step to `open_as_follower`. The follower briefly opens a read-write connection to the same DB, runs the idempotent ALTER (no-op when the column is present), and closes it before opening its long-lived read-only connection. Failures (read-only FS, locked file) are logged at WARN and swallowed so follower startup stays robust.

## Review Findings (2026-05-12 20:30)

Verified against the actual code:

- `IndexingProgress` struct present in `swissarmyhammer-code-context/src/ops/search_code.rs:59-70`, derives `Debug, Clone, Serialize`, fields match the contract.
- `progress: Option<IndexingProgress>` field present on `SearchCodeResult` (line 83).
- `compute_indexing_progress(conn)` helper at `search_code.rs:168-196` — single `COUNT(*), COALESCE(SUM(embedded), 0)` round-trip, correct empty-table and fully-embedded short-circuits.
- `search_code_with_query_embedding` factored out at `mod.rs:1520-1561` — clean async/sync seam for testability.
- `check_ts_readiness` removed from `execute_search_code` only; remaining 8 op call sites still gate (get_symbol, search_symbol, list_symbols, grep_code, find_duplicates, query_ast, get_callgraph, get_blastradius).
- 4 new progress unit tests in `search_code.rs` pass (partially-embedded, fully-embedded, no-files, none-embedded).
- Dispatch-layer test `test_search_code_returns_result_with_progress_when_not_embedded` at `mod.rs:3761` proves the gate is gone end-to-end.
- Follower migration: `db::migrate_indexed_files` is crate-public at `db.rs:81`; `migrate_legacy_schema_if_writable` at `workspace.rs:284` opens a short-lived RW connection, runs the migration, swallows failures with WARN logs.
- 3 migration tests at `workspace.rs:794, 847, 859` — drop-column-under-live-leader simulation, idempotency, and bogus-path resilience all pass.
- `cargo nextest run -p swissarmyhammer-code-context --lib`: 719/719 pass.
- `cargo clippy -p swissarmyhammer-code-context -p swissarmyhammer-tools --lib --tests -- -D warnings`: clean.

### Nits

- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1481` — Doc comment on `execute_search_code` says "still used by the nine other ops" but only **8** ops actually still call `check_ts_readiness` after removal from `search_code` (get_symbol, search_symbol, list_symbols, grep_code, find_duplicates, query_ast, get_callgraph, get_blastradius). The same off-by-one phrasing appears in the task description's "Files" and "Acceptance Criteria" sections. Suggested fix: change "nine other ops" to "eight other ops" in the doc comment. Pure documentation accuracy; no behavior impact.