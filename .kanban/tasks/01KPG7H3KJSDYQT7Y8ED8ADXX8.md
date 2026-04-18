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
position_column: todo
position_ordinal: ef80
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

- [ ] Audit `DeleteEntityCmd`, `ArchiveEntityCmd`, `UnarchiveEntityCmd`, `PasteEntityCmd` (and handlers): confirm they invoke `run_op` (which goes through the processor) rather than calling operations directly.
- [ ] Write integration tests per mutation.
- [ ] Verify `entity.copy` / `entity.cut` YAML have `undoable: false`.

## Acceptance Criteria

- [ ] Undo after `entity.delete` on a task/tag/project/column/actor restores that entity.
- [ ] Undo after `entity.archive` restores the entity to its unarchived state.
- [ ] Undo after `entity.unarchive` returns the entity to archived.
- [ ] Undo after `entity.paste` (copy variant) removes the created entity.
- [ ] Undo after `entity.paste` (cut variant) removes the created entity AND restores the source.
- [ ] Redo after undo reapplies the mutation.

## Tests

- [ ] `undo_entity_delete_restores_tag` — create tag, delete via auto-emit, undo, assert tag exists.
- [ ] `undo_entity_archive_restores_project`.
- [ ] `undo_entity_paste_removes_created_task` — paste into column, undo, assert new task gone.
- [ ] `undo_entity_paste_cut_restores_source_task` — cut task, paste into column, undo, assert new task gone AND source restored.
- [ ] `entity_copy_is_not_undoable` — dispatch, confirm nothing lands on the undo stack.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban undo_cross_cutting` — all green.

## Workflow

- Use `/tdd` — write one test per mutation; if it fails, trace into the dispatch path to see where undoability drops.

#commands

Depends on: 01KPEME1897275TKE61EKN6EVX (retire DeleteProjectCmd), all 6 per-type YAML cleanup cards