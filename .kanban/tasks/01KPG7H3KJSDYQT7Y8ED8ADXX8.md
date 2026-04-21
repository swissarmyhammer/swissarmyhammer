---
assignees:
- claude-code
depends_on:
- 01KPEME1897275TKE61EKN6EVX
- 01KPG6XDVSY9DAN2TS26W52NN6
- 01KPG6XPMDHSH8PMD248YK6KAK
- 01KPG6XZ9GKP2VJPA6XWNE8WN4
- 01KPG6Y6WKHYH7EYDJ0NX8CR1R
- 01KPG6YDZDCPWGWKCC38TWM8AV
- 01KPG6YN15ECCK9SP262BJKGK2
position_column: done
position_ordinal: ffffffffffffffffffffffff8480
title: 'Commands: undo verification for cross-cutting mutations (delete, archive, unarchive, paste)'
---
## What

Every cross-cutting mutating command declares `undoable: true` in its YAML. The auto-emit dispatch path must route through the operation processor so undo/redo work. Test each mutation end-to-end: execute → undo → state is restored; redo → state reapplied.

### Commands to verify

Auto-emitted and mutating:

- `entity.delete` — undo restores the entity.
- `entity.archive` — undo restores the entity to its live state.
- `entity.unarchive` — undo returns the entity to archived.
- `entity.paste` — undo removes the created entity (and for cut, restores the source).
- `entity.copy` / `entity.cut` — non-mutating at the entity layer (they only touch the clipboard); `undoable: false` is correct. Verify the YAML says so.

### Files to touch

- `swissarmyhammer-kanban/tests/undo_cross_cutting.rs` (NEW) — integration tests per mutation.
- Any Rust impl that's NOT flowing through `KanbanOperationProcessor::process` — fix so it does.

### Subtasks

- [x] Audit `DeleteEntityCmd`, `ArchiveEntityCmd`, `UnarchiveEntityCmd`, `PasteEntityCmd` (and handlers): confirm they invoke `run_op` (which goes through the processor) rather than calling operations directly.
- [x] Write integration tests per mutation.
- [x] Verify `entity.copy` / `entity.cut` YAML have `undoable: false`.

## Acceptance Criteria

- [x] Undo after `entity.delete` on a task/tag/project/column/actor restores that entity.
- [x] Undo after `entity.archive` restores the entity to its unarchived state.
- [x] Undo after `entity.unarchive` returns the entity to archived.
- [x] Undo after `entity.paste` (copy variant) removes the created entity.
- [x] Undo after `entity.paste` (cut variant) removes the created entity AND restores the source.
- [x] Redo after undo reapplies the mutation.

## Tests

- [x] `undo_entity_delete_restores_tag` — create tag, delete via auto-emit, undo, assert tag exists.
- [x] `undo_entity_archive_restores_project`.
- [x] `undo_entity_paste_removes_created_task` — paste into column, undo, assert new task gone.
- [x] `undo_entity_paste_cut_restores_source_task` — cut task, paste into column, undo, assert new task gone AND source restored.
- [x] `entity_copy_is_not_undoable` — dispatch, confirm nothing lands on the undo stack.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban undo_cross_cutting` — all green.

## Resolution

All handlers already used `run_op` and the processor pipeline was correct — the real bug was that `StoreContext::undo`/`redo` move files on disk (trash / restore / archive) without touching the `EntityContext` in-memory cache. After undo, the cache still held the stale entity (for paste-create / unarchive-update), so subsequent `ectx.read` returned the pre-undo value.

Fix:
- `StoreContext::undo` and `StoreContext::redo` now return a new `UndoOutcome { store_name, item_id }` identifying which item's files were rewound/replayed.
- Added `EntityContext::sync_entity_cache_from_disk(entity_type, id)` that refreshes the cache from disk when the file exists, evicts it when the file is gone.
- `UndoCmd` / `RedoCmd` call the sync helper immediately after `StoreContext::undo` / `redo` when an `EntityContext` extension is attached — no-op otherwise, so raw store callers are unaffected.

Result: every cross-cutting mutation round-trips cleanly through execute → undo → redo with the cache and disk in lockstep.

## Workflow

- Use `/tdd` — write one test per mutation; if it fails, trace into the dispatch path to see where undoability drops.

#commands

Depends on: 01KPEME1897275TKE61EKN6EVX (retire DeleteProjectCmd), all 6 per-type YAML cleanup cards

## Review Findings (2026-04-20 17:45)

### Warnings

- [x] `swissarmyhammer-kanban/tests/undo_cross_cutting.rs:532-617` — Test `undo_entity_paste_cut_restores_source_task` does not test what its name and docstring claim. The body explicitly acknowledges it cannot drive the paste-cut source-restore path end-to-end (lines 554-575 contain multi-paragraph explanations) and falls back to a copy-paste scenario that duplicates `undo_entity_paste_removes_created_task`. The acceptance criterion "Undo after `entity.paste` (cut variant) removes the created entity AND restores the source" is marked complete but is not verified here. Either (a) rename the test and its docstring to reflect that it is a copy-paste variant and add a separate cut-paste test that drives the real path (perhaps via synthetic clipboard payload construction or by staging two independent add_task calls so the cut's delete target actually exists when paste runs), or (b) delete the test entirely since it duplicates coverage. As currently written the test is misleading documentation — a future reader grep'ing for the cut-restore guarantee would land here and find no assertions for it.

  **Resolution:** Took option (a): the test now stages a synthetic cut-mode clipboard payload via `serialize_to_clipboard` + `clipboard.write_text` so the source is still live when `entity.paste` runs. The handler's create-then-delete path (`AddEntity` then `DeleteTask`) executes through `run_op`, pushing two entries onto the shared undo stack. The test asserts (1) after paste: new task exists, source deleted; (2) after one undo (LIFO pops delete-source): source restored, new task still present; (3) after second undo: new task removed, board back to pre-paste state with only the source. This is the genuine cut-restore guarantee, not a copy-paste duplicate. Going through `entity.cut` directly is not a viable path because cut pre-deletes the source before paste runs, starving the handler's delete-source branch.

### Nits

- [x] `swissarmyhammer-entity/src/context.rs:741-748` — Dead-code duplication in `sync_entity_cache_from_disk`. Lines 741-743 perform `if self.entity_def(entity_type).is_err() { return; }` and lines 745-748 immediately repeat the same check as a `match` that binds `def` for the subsequent `io::entity_file_path` call. The first check is unreachable-on-error since the match below handles the error path identically. Remove lines 741-743 (and its comment) — the match at 745 already expresses "skip unknown entity types" exactly once.

  **Resolution:** Removed the duplicate early-return. The match on `entity_def(entity_type)` now handles "skip unknown entity types" exactly once, and the "unknown entity type" comment was preserved on the match arm.

- [x] `swissarmyhammer-kanban/tests/undo_cross_cutting.rs:676-693` — `stack_depth` helper drives real `undo`/`redo` round-trips against `StoreContext` to measure depth. It is correct but expensive (O(n) file-system writes per probe, doubled for the redo restore) and fragile (if any probed `redo` fails, the stack is left inconsistent and the `expect` panics, masking the original test failure). Consider exposing a `StoreContext::stack_len()` / `StoreContext::can_undo_at(idx)` read-only accessor in `swissarmyhammer-store` so the probe is a single atomic read — this also avoids the filesystem churn during `entity_copy_is_not_undoable`. Not blocking; the current helper passes and the overhead is in test code only.

  **Resolution:** Added `StoreContext::undo_depth()` as a read-only accessor that returns the current stack pointer without any filesystem writes. `entity_copy_is_not_undoable` now calls `engine.store_context.undo_depth()` before and after the dispatch; the `stack_depth` test helper was deleted.

- [x] `swissarmyhammer-entity/src/undo_commands.rs:68-74` — `sync_entity_cache` helper sits at file scope as an `async fn`. It reads cleanly but could be a `CommandContext` method (or an inherent method on `UndoCmd`/`RedoCmd`) to keep the undo_commands module free-floating-helpers count down. Pure style — the current shape is fine and matches the "small focused helper" pattern already used elsewhere in the crate.

  **Resolution:** Kept as-is per reviewer's explicit acknowledgment ("the current shape is fine and matches the 'small focused helper' pattern already used elsewhere in the crate"). The file-scope `async fn` is a conscious choice — promoting it to a method would force the helper into the public surface of `CommandContext` or `UndoCmd`/`RedoCmd` without a downstream caller, which trades one kind of free-floating helper for another.
