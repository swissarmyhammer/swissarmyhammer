---
assignees:
- claude-code
depends_on:
- 01KQ5FJ0VXEQZVKHZBN49Q5GFS
- 01KQ5QM5PJHK3V5PW3F4K63J4K
position_column: todo
position_ordinal: 7f8280
project: single-changelog
title: 'single-changelog: delete dead log/changelog APIs and orphaned on-disk data'
---
#single-changelog #refactor #tech-debt #cleanup

## Goal

Final teardown card for the unified-storage initiative. After this card lands, `grep -rE 'append.*log|changelog|jsonl' --include='*.rs'` returns only references to the live `swissarmyhammer-store::Changelog` machinery. No dead public APIs, no orphaned types, no stale on-disk data.

Audit was performed 2026-04-26 — every item below was verified to have **zero production callers** (only test-only call sites, or no callers at all).

Depends on the entity writer-off card (so `append_changelog` no longer has production callers) and the views migration card (so `ViewsChangelog` no longer has production callers). Independent of the projecting reader card.

## What

### Source code to delete

#### `swissarmyhammer-entity/src/changelog.rs`

- Delete `pub async fn append_changelog(path, entry)` (production callers removed in `01KQ5FJ0VXEQZVKHZBN49Q5GFS`; only test fixtures remain).
- Migrate the test fixtures that used `append_changelog` to write legacy entity-format lines onto disk for back-compat reads (`changelog.rs` test module, `cache.rs:1525, 2107, 2174, 2336`, `context.rs:3437, 3440`). Replacement: write directly via `tokio::fs::write` with hand-formatted JSON lines. Tests stay focused on the read-side projection.

#### `swissarmyhammer-kanban` dead log infrastructure

- `swissarmyhammer-kanban/src/context.rs` — delete `pub async fn append_task_log`, `append_tag_log`, `append_actor_log`, `append_column_log`, `append_board_log`, the private `append_log` helper, and the `task_log_path` / `tag_log_path` / `actor_log_path` / `column_log_path` / `board_log_path` accessor family. Audit confirmed every call site is in `#[tokio::test]` blocks of the same file (5,221+ test-only references in 5 test functions: `test_append_task_log`, `test_append_tag_log`, `test_append_actor_log`, `test_append_column_log`, `test_append_board_log`, plus their `_writes_jsonl` siblings). Delete the test functions too — they're testing nothing reachable.
- `swissarmyhammer-kanban/src/types/log.rs` — delete the file and remove `LogEntry` from `swissarmyhammer-kanban/src/types/mod.rs` re-exports. Re-import `LogEntry` from `swissarmyhammer-operations` only where still needed (the operations processor's signature plumbing, see below).
- `swissarmyhammer-kanban/src/processor.rs::write_log` — delete the no-op method. The `OperationProcessor::write_log` trait method goes away with it (next bullet).

#### `swissarmyhammer-operations` dead log trait

- `swissarmyhammer-operations/src/processor.rs` — delete `OperationProcessor::write_log` from the trait. Both production impls are no-ops (`swissarmyhammer-kanban::KanbanOperationProcessor::write_log` says "Per-entity logging is handled by EntityContext/StoreHandle; there is no global activity log, so this is intentionally a no-op." `swissarmyhammer-js::JsOperationProcessor::write_log` says "No logging for in-memory JS state operations.").
- `swissarmyhammer-operations/src/log.rs::LogEntry` — delete the file unless `swissarmyhammer-operations::Execute` / `ExecutionResult` still need it. Audit at implementation time. If `ExecutionResult::split` returns `(Result, Option<LogEntry>)`, then either: (a) drop the `Option<LogEntry>` from `ExecutionResult` and have `Execute` impls return only the result; (b) keep `LogEntry` as a public type but with no writers. Option (a) is the right shape — `Execute` impls don't need to construct unused log entries. Search `LogEntry::new` after the trait removal — if any caller still constructs one, audit them.
- `swissarmyhammer-js/src/processor.rs::write_log` — delete the no-op method along with the trait removal.

### On-disk data to delete

- **`.kanban/activity/`** — 9.7 MB, 5,221 entries, last write 2026-04-08, no current writer (verified by grep + processor.rs no-op). Delete the directory.
- **`.kanban/swimlanes/`** — pre-rename data from `swimlane → project` migration. No live writer in workspace. The only reader is `llama-agent/src/acp/plan.rs:183` which reads `task["position"]["swimlane"]` for backwards-compat; that field no longer exists in any current task entity. Delete the directory.
- **`.kanban/board.jsonl`** — file doesn't exist on disk in this checkout, but the dead `append_board_log` API in `swissarmyhammer-kanban::context` could create it. Once that API is deleted, no path can create the file. Sanity-check: if the file exists in any workspace, document a one-time delete.
- **`.kanban/views.jsonl`** — orphaned by the views-migration card. Delete.

The deletions are local to a workspace's `.kanban/` directory. Document the cleanup in the PR description so other clones know to delete the same paths.

## Acceptance

- [ ] `grep -rn 'append_changelog\b' --include='*.rs'` returns matches only inside `#[deprecated]`-tagged functions or in `swissarmyhammer-store` (the unrelated `Changelog::append`).
- [ ] `grep -rn 'append_task_log\|append_tag_log\|append_actor_log\|append_column_log\|append_board_log\|append_log\b' --include='*.rs'` returns nothing.
- [ ] `grep -rn 'task_log_path\|tag_log_path\|actor_log_path\|column_log_path\|board_log_path' --include='*.rs'` returns nothing.
- [ ] `grep -rn 'OperationProcessor.*write_log\|fn write_log\b' --include='*.rs'` returns nothing.
- [ ] `swissarmyhammer-kanban/src/types/log.rs` is deleted (or empty / re-exporting nothing relevant).
- [ ] If `LogEntry` was deleted: `grep -rn 'LogEntry' --include='*.rs'` returns nothing.
- [ ] `.kanban/activity/`, `.kanban/swimlanes/`, `.kanban/views.jsonl` are deleted from this workspace; `.gitignore` updated if needed (probably not — these were git-tracked, deletion is a normal commit).
- [ ] `cargo nextest run --workspace` green.
- [ ] `cargo build --workspace` is clean (no new warnings; `#[deprecated]` warnings from prior cards no longer fire because the deprecated items are gone).

## Tests

This is a deletion-heavy card. The acceptance is "everything else still works." Specific guards:

- [ ] Re-run the delete/undo-delete roundtrip tests from `01KQ5FJ0VXEQZVKHZBN49Q5GFS` and `01KQ5QM5PJHK3V5PW3F4K63J4K` — they must still pass after the deletions.
- [ ] Verify the kanban app still boots and reads its `.kanban/` workspace (where `activity/`, `swimlanes/`, `views.jsonl` are gone). Add a sanity test in `kanban-app` that opens a workspace missing those paths and asserts no error logs.
- [ ] If `LogEntry` removal triggers `ExecutionResult::split` reshape: every `Execute` impl in `swissarmyhammer-kanban` and `swissarmyhammer-js` keeps producing valid results.
- [ ] `cargo nextest run --workspace` green.

## Workflow

Sequential deletions, each followed by a `cargo build` to surface what else needs to go:

1. Delete the entity-layer `append_changelog` definition; migrate its test fixtures.
2. Delete the kanban-layer `append_*_log` family + `LogEntry` import + `LogEntry` type file + tests for them.
3. Delete `OperationProcessor::write_log` trait method, then its no-op impls in `swissarmyhammer-kanban` and `swissarmyhammer-js`.
4. Decide on `LogEntry`'s fate based on what's left referencing it; delete or keep accordingly.
5. Delete the on-disk data directories.
6. Run the full workspace test suite.

## Scope

- depends_on: `01KQ5FJ0VXEQZVKHZBN49Q5GFS` (writer-off — without it, `append_changelog` deletion breaks production), `01KQ5QM5PJHK3V5PW3F4K63J4K` (views migration — without it, `views.jsonl` deletion breaks running view edits).
- Blocks: nothing. After this lands, the `single-changelog` initiative is `done`: unified storage with diff and undo, one writer per file, one undo stack, one diff/projection mechanism.
