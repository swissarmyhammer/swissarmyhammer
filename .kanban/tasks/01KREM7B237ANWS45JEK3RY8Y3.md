---
assignees:
- claude-code
depends_on:
- 01KREM6B7X01T8WXDA258DS2K9
position_column: done
position_ordinal: ffffffffffffffffffffffffbf80
project: semantic-search
title: Delete dead indexer in swissarmyhammer-code-context/src/indexing.rs
---
## What

`swissarmyhammer-code-context/src/indexing.rs` contains a complete, tested indexer (`spawn_indexing_worker`, `run_indexing_worker`, `persist_batch_results`, `write_ts_chunks`, `parse_and_extract_chunks`) that is NEVER called from production code. Production indexing goes through `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs::index_discovered_files_async` instead, which uses the real `chunk_file()` tree-sitter chunker and writes `symbol_path`.

DB evidence: chunks in the live `.code-context/index.db` have symbol paths like `cache.rs::TINY_MODEL_REPO` — only the `mod.rs` path produces those. The `indexing.rs` path produces whole-file blobs with no symbol_path.

Call graph (verified this session):
- `write_ts_chunks` (indexing.rs:224) — production callers: only `persist_batch_results`
- `persist_batch_results` (indexing.rs:142) — production callers: only `run_indexing_worker`
- `run_indexing_worker` (indexing.rs:73) — production callers: NONE (only `spawn_indexing_worker`)
- `spawn_indexing_worker` (indexing.rs:52) — production callers: NONE; called only from `swissarmyhammer-code-context/tests/workspace_e2e_test.rs` (test code) and tests inside indexing.rs itself.

The `workspace_e2e_test.rs` tests exist to test this dead worker. They are testing an orphan code path; once the worker is gone they have nothing to assert.

### Files

- `swissarmyhammer-code-context/src/indexing.rs` — delete the entire file or shrink it dramatically. Keep `IndexingConfig` and `SharedDb` only if they're used elsewhere (verify). Everything tied to the worker (spawn, run, persist, write_ts_chunks, parse_and_extract_chunks, and all their tests) goes.
- `swissarmyhammer-code-context/src/lib.rs` — remove `pub mod indexing;` and any re-exports.
- `swissarmyhammer-code-context/tests/workspace_e2e_test.rs` — these tests spin up `spawn_indexing_worker` against a temp repo and assert the resulting DB state. Three options:
  1. **Preferred:** rewrite them to drive `index_discovered_files_async` from `swissarmyhammer-tools` instead, so the e2e coverage moves to the real path. This will require adding `swissarmyhammer-tools` as a dev-dependency of `swissarmyhammer-code-context`, which may cause a cycle. Check the dep graph; if cyclic, move the tests to `swissarmyhammer-tools/tests/`.
  2. Delete the tests entirely (acceptable — card 4's end-to-end test covers the real path).
  3. Keep them but they exercise a worker that doesn't exist anymore — NOT acceptable.

   Pick option 1 if dep direction allows, otherwise option 2.
- `swissarmyhammer-code-context/Cargo.toml` — drop any deps used only by the deleted module (rayon? threading helpers?). Verify with `cargo machete` or by inspection.
- `MEMORY.md` user memory at `/Users/wballard/.claude/projects/-Users-wballard-github-swissarmyhammer-swissarmyhammer/memory/MEMORY.md` references the old indexing path with claims like "auto-population works end-to-end" attributed to `indexing.rs::run_indexing_worker`. Update or delete this note as part of the cleanup, since the memory is wrong about which path is live.

### Approach

1. Run `cargo machete` (or read deps manually) to identify deps only used by `indexing.rs`.
2. Delete `swissarmyhammer-code-context/src/indexing.rs`. Remove the `pub mod indexing;` line from `lib.rs`.
3. Compile. Fix any breakages by removing dead imports.
4. Decide on the `workspace_e2e_test.rs` tests per the options above. Either rewrite (preferred) or delete.
5. Update `MEMORY.md` to remove the false claim that `indexing.rs` is the live indexer.

### Verification before deletion

- [ ] Grep the entire workspace for `spawn_indexing_worker` and `run_indexing_worker`. Only matches outside `swissarmyhammer-code-context/src/indexing.rs` should be: the tests in that same file, `workspace_e2e_test.rs`, and `.kanban/` activity logs (history, ignorable). If anything else shows up, do NOT delete — investigate.
- [ ] Run `cargo check --workspace` after deletion. Anything that breaks is something the dead code was holding up.

## Acceptance Criteria

- [ ] `swissarmyhammer-code-context/src/indexing.rs` is deleted (or trimmed to the bare types still used externally).
- [ ] `cargo check --workspace` clean.
- [ ] `cargo nextest run --workspace` clean.
- [ ] `workspace_e2e_test.rs` either deleted or rewritten against `index_discovered_files_async`.
- [ ] No remaining references to `spawn_indexing_worker` outside `.kanban/` history.
- [ ] `MEMORY.md` updated.

## Tests

- [ ] `cargo nextest run --workspace` — all tests pass.
- [ ] `cargo check --workspace` — clean.
- [ ] If `workspace_e2e_test.rs` is rewritten, the new tests still produce real DB state assertions against the real indexer.

## Workflow

- Use `/tdd` is not applicable for a deletion — focus on verification first, then delete, then re-run the full suite. #code-context