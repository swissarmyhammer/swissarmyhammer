---
assignees:
- claude-code
depends_on:
- 01KREM4Q7MTM9VMHCCABKCRK7Y
- 01KREM5G5V6CRVHR492W1SKGGQ
position_column: done
position_ordinal: ffffffffffffffffffffffffbc80
project: semantic-search
title: 'End-to-end integration test: index real repo, search for matches'
---
## What

Add the end-to-end test that would have caught both bugs in this project (the missing-embeddings bug and the over-strict gate). Currently every `search_code` test raw-SQL-injects pre-computed embedding blobs via `insert_chunk_with_embedding`, completely bypassing the indexer. That is why production indexing has shipped for months with 0 embedding rows in the DB and no test caught it.

The new test must drive the REAL indexer end-to-end: temp dir → write a few small Rust files → call `index_discovered_files_async` → assert `embedding IS NOT NULL` rows exist → call `execute_search_code` via the MCP tool path → assert matches come back for a semantically related query.

### This is the project's reference pattern

This card establishes the **required test pattern** for every MCP tool that advertises a search, lookup, or analysis capability:

> **Real indexer → real query → assert real result.**
> Fixture-only tests that raw-SQL-insert pre-computed data DO NOT count as coverage of an advertised capability — they prove only that the consuming function compiles and its math is correct.

Card A (audit) sweeps for other ops missing this pattern and creates follow-up cards. This card writes the canonical example they will be modeled on.

### Files

- New test file: `swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs` (or `swissarmyhammer-code-context/tests/semantic_search_e2e.rs` if it fits better — pick the location that lets you call `index_discovered_files_async` directly. It's `pub(crate)` in the tools crate, so an integration test in the same crate is fine. Otherwise expose it `pub` for testing — judgment call by the implementer based on what is cleanest).
- Existing fixture pattern: `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:2733` already documents that internal tests live alongside `index_discovered_files_async`. The end-to-end test can live there or in `tests/` — choose `tests/` so it is a real integration test that doesn't see `pub(crate)` internals.

### Approach

1. Create a `tempfile::TempDir`. Inside it, write 3 small Rust files with distinct semantic meaning, e.g.:
   - `auth.rs` containing a function that validates user credentials
   - `parser.rs` containing JSON parsing helpers
   - `math.rs` containing arithmetic utilities
2. Initialize a `CodeContextWorkspace` rooted at the temp dir. This triggers `startup_cleanup()` which discovers the three files and marks them dirty.
3. Call `index_discovered_files_async(&temp_dir, db).await` — this is the production indexer. After it returns:
   - Assert `SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL` > 0.
   - Assert all three files in `indexed_files` have `ts_indexed=1` AND `embedded=1`.
4. Call `execute_search_code` via the MCP tool path with a semantic query like `"verify user identity"` and `top_k=3`.
   - Assert the result deserializes to a `SearchCodeResult` (not a "not ready" text).
   - Assert `result.matches.len() > 0`.
   - Assert the top match's `file_path` is `auth.rs` (this proves embeddings are working as semantic signal, not just exact-text fallback).
   - Assert `result.progress.is_none()` (fully embedded).
5. Use `serial_test::serial(cwd)` if the test needs to chdir. Use `CurrentDirGuard` per project test-isolation conventions.

### Document the pattern

Add a top-of-file comment in the new test file that names the pattern explicitly:

```
//! End-to-end semantic search test — the reference pattern for code-context op coverage.
//!
//! Every MCP tool that advertises a capability (search, lookup, analysis) needs at
//! least one test in this style: drive the real production indexer over real files,
//! then call the user-facing op and assert on the result. Fixture-only tests that
//! raw-SQL-insert pre-computed data prove math, not features.
//!
//! See card A for the audit that ensures other ops have equivalent coverage.
```

### Caveats

- Loading `Embedder::default()` downloads the Qwen embedding model (~600MB GGUF or CoreML .mlpackage). Production CI may already cache this — confirm by checking other tests in `swissarmyhammer-embedding/tests/` and `swissarmyhammer-tools/tests/integration/file_size_limits.rs` that already use `Embedder`. If the test would download in CI, mark it appropriately (`#[ignore]` with a comment, or behind a feature flag). The default should be: ON in CI, locally requires cached model. Match the pattern used by existing embedder tests.
- Sequential embedding of ~10 chunks should complete in seconds; this is small enough not to need batching.

## Acceptance Criteria

- [ ] New test file exists and uses the real production indexer (not the fixture-inject pattern).
- [ ] Test passes: writes files, indexes, asserts embeddings exist, runs a semantic query, asserts the right file is the top match.
- [ ] Top-of-file comment names and documents the "real indexer → real query → real result" pattern, with a reference to card A.
- [ ] If this test is removed from the suite, it would not pass without card 2 (real embedding writes) AND card 3 (gate removed). Verify this by reading the asserts — the test must fail today against trunk.
- [ ] Test uses absolute path/temp_dir hygiene (no leftover state).
- [ ] Test does not depend on hard-coded sleep delays — uses real awaits.

## Tests

- [ ] `cargo nextest run -p swissarmyhammer-tools --test integration semantic_search_e2e` (or wherever the test lives) — passes.
- [ ] Run with `--no-fail-fast` and confirm no flakes across 5 runs.
- [ ] Manually re-run search_code's existing fixture-based tests — they should still pass; this card is additive.

## Workflow

- Use `/tdd` — write the failing assertions first against trunk (where embeddings don't exist), then verify they pass after card 2 + card 3 land.