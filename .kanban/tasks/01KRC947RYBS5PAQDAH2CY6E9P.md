---
assignees: []
position_column: done
position_ordinal: '7e80'
project: single-changelog
title: 'single-changelog: post-merge audit for residual duplicate change-logging'
---
Post-mortem audit after commit a24c0d116 ("refactor(single-changelog): unify changelog storage on store-layer text patches") landed on branch `log`. Verifies whether the unification is complete or any duplicate change-logging machinery remains.

**Methodology**: grepped the workspace for `append`, `write_all`, `OpenOptions.*append`, `jsonl`, `Changelog`, `LogEntry`, `UndoStack`, `UndoEntry`, `ChangeEntry`, `forward_patch`, `reverse_patch`, `history`, `audit`, `activity`, `snapshot`. Cross-checked each hit. Examined `kanban-app/src/{watcher,state}.rs` for parallel event routing. Examined every `pub async fn write_*` in entity/views/perspectives/kanban for paths bypassing `StoreHandle`. Checked on-disk shape under `.kanban/`.

## Review Findings

The unification is essentially complete. **No real duplicate writers, parallel undo stacks, or parallel changelog formats remain.** The findings below are minor residuals / observations, not duplicate machinery:

### 1. Multiple writers to same on-disk log file — NONE FOUND
- `swissarmyhammer-store/src/changelog.rs:109,250,347,374` — canonical `Changelog::append` (the only production writer).
- `swissarmyhammer-entity/src/changelog.rs:600`, `swissarmyhammer-entity/src/context.rs:3575`, `swissarmyhammer-entity/src/cache.rs:1523` — three `write_legacy_changelog_line` helpers, all `#[cfg(test)]` only, used to seed legacy-format JSONL fixtures so the projecting reader can be exercised. Each has a doc comment explicitly calling out the test-only intent. Not duplicate writers.
- `avp-cli/src/main.rs:212`, `llama-agent/src/acp/raw_message_manager.rs:38`, `claude-agent/src/agent_raw_messages.rs:65`, `claude-agent/src/claude.rs:783`, `swissarmyhammer-treesitter/src/db.rs:432`, `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1053`, `swissarmyhammer-tools/src/mcp/tools/shell/state.rs:157,235` — unrelated raw-message/shell/DB logs in other domains. Out of scope.

### 2. Parallel changelog types/formats — NONE FOUND
- `swissarmyhammer-store/src/changelog.rs:39` `ChangelogEntry` — canonical (text patches via diffy).
- `swissarmyhammer-entity/src/changelog.rs:79` `ChangeEntry` — kept by design as the **read-only projected shape**. No writer in the entity crate; the only `pub` functions on this module are `diff_entities`, `reverse_changes`, `apply_changes`, `read_changelog`, `read_changelog_for`, `read_changelog_with_fallback`. Confirmed clean.
- `swissarmyhammer-merge/src/yaml.rs:57` `ChangelogEntry` — private struct, read-only parser used during git merge conflict resolution on the legacy JSONL shape. No writer. Cleared.
- `swissarmyhammer-tools/src/mcp/tools/git/diff/mod.rs:113` `ChangeEntry` — unrelated; git semantic-diff output struct. Cleared.

### 3. Parallel undo stacks — NONE FOUND
Only `swissarmyhammer-store::stack::UndoStack` exists. The expected orphan `swissarmyhammer-entity/src/undo_stack.rs` is deleted. Only one `undo_stack.yaml` writer (`swissarmyhammer-store/src/context.rs:80,127,179`). The `swissarmyhammer-kanban/src/board/init.rs:290` reference adds it to `.gitignore` only — not a writer.

### 4. Parallel snapshot-on-write — NONE FOUND
The "snapshot" hits in `swissarmyhammer-views/src/context.rs:190` and `swissarmyhammer-perspectives/src/context.rs:94` are in-memory `let old = self.get_by_id(...).cloned()` for diff computation only. No persisted snapshots outside the StoreHandle path.

### 5. Parallel event emission — OBSERVATION (not a bug)
Each domain has exactly one broadcast channel feeding the bridge:
- `EntityCache::event_sender` (entity events)
- `PerspectiveContext::event_sender` (perspective events)
- `ViewsContext::event_sender` (view events)
- `kanban-app/src/state.rs:388-397` subscribes to all three; `kanban-app/src/watcher.rs:670-734` is the single bridge that fans them out to Tauri.

However, `StoreHandle` *also* records `pending_events` (`handle.rs:31`) drained via `flush_changes()` / `StoreContext::flush_all()`. Grepping production callers: only tests and one integration test invoke `flush_all()` — production never does. The domain broadcast channels (above) are the actual production event path. So the pending-events plumbing is a second-pathway API kept around for tests / future use, not a live duplicate emission. Worth a follow-up to decide: keep as test-only API (and document) or remove. Filed here as an observation, not a duplicate to remove now.

### 6. Stale log/changelog modules — NONE FOUND
- `swissarmyhammer-entity/src/lib.rs` declares `changelog`, `cache`, `context`, `entity`, `error`, `events`, `filter`, `id_types`, `io`, `store`, `undo_commands`, `watcher` — all referenced. No orphan modules.
- `swissarmyhammer-entity/src/undo_stack.rs` confirmed deleted (Glob result: no file).
- `swissarmyhammer-kanban/src/types/` no longer has `log.rs` (confirmed in directory listing).
- `swissarmyhammer-operations/src/lib.rs` exports `ExecutionResult` with only `Success`/`Failed` variants (no `Logged` variant). `OperationProcessor::write_log` is gone — `grep` for `write_log` and `append_*_log` family returns zero hits across the workspace.
- `LogEntryId` is gone; `OperationId` is the rename and is the sole identifier used in `swissarmyhammer-kanban/src/types/{ids,operation}.rs`.

### 7. On-disk leftover shapes — NONE FOUND
Live `.kanban/` directory contains exactly: `actors/`, `boards/`, `columns/`, `perspectives/`, `projects/`, `tags/`, `tasks/`, `views/`, `undo_stack.yaml`, `.gitattributes`, `.gitconfig`, `.gitignore`. No `activity/`, `swimlanes/`, `views.jsonl`, or `board.jsonl`. Per-entity `.jsonl` files live alongside their `.yaml`/`.md` siblings exactly as the canonical model expects.

## Minor stale references (not duplicates — purely cosmetic)

The following hits are stale **mentions** of the deleted machinery, not duplicates. They do not affect correctness. Worth a separate small cleanup pass if anyone touches these files:

- `swissarmyhammer-entity/src/watcher.rs:298` — test asserts `parse_entity_path` ignores `/project/.kanban/activity/changelog.jsonl`. The `activity/` dir no longer exists in production, but the test still demonstrates the "ignore non-entity paths" contract. Path is now hypothetical; intent remains valid. Optional: rename to a different non-entity path.
- `swissarmyhammer-kanban/tests/perspective_integration.rs:43` — doc comment mentions `perspectives.jsonl` (singular flat file) which never existed in the unified model. Should read "per-perspective `.jsonl` changelogs".
- `swissarmyhammer-perspectives/src/lib.rs:1` — module doc says "Perspective registry and changelog system". The crate no longer owns its own changelog format — it implements `TrackedStore` and lets `swissarmyhammer-store` own the log shape. Could be tightened to "Perspective registry (with store-layer changelog/undo)".

## Conclusion

The single-changelog unification is **complete**. There are no remaining duplicate writers, parallel changelog formats, parallel undo stacks, or snapshot-on-write paths. The only observation worth follow-up is the `StoreHandle::flush_changes` / `StoreContext::flush_all` API that exists but is exercised only by tests (the production event path is the domain broadcast channels). The three cosmetic stale references above are purely doc-string drift and have no behavioral impact.

#single-changelog #audit #tech-debt