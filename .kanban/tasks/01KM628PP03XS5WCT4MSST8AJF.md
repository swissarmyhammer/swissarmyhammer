---
assignees:
- claude-code
depends_on:
- 01KM621VQF672VCZ26S8DG350S
position_column: done
position_ordinal: ffffffffffb280
title: 'Test: undo/redo cycle for archive and unarchive operations'
---
## What

Verify that archive/unarchive integrate correctly with the existing undo/redo system in EntityContext. The undo infrastructure already handles delete/restore via trash — archive needs the same treatment.

### Test scenarios

1. **Undo archive** — archive an entity, undo it, entity reappears in `list()`
2. **Redo archive** — archive, undo, redo — entity is archived again
3. **Undo unarchive** — archive, unarchive, undo unarchive — entity goes back to archive
4. **Full cycle** — archive → undo → redo → undo — entity is live at the end
5. **Changelog correctness** — verify each operation writes the correct `op` string and `undone_id`/`redone_id` references

### Implementation notes

The undo system in `EntityContext` dispatches on `entry.op` — currently handles "update", "create", "delete". It needs to handle "archive" and "unarchive" ops:
- Undo "archive" = restore from `.archive/` (like undo "delete" restores from `.trash/`)
- Undo "unarchive" = move back to `.archive/` (like undo "create" moves to `.trash/`)

This may require additions to `undo_single()` / `redo_single()` match arms in `context.rs`.

### Files
- `swissarmyhammer-entity/src/context.rs` — add "archive"/"unarchive" arms to undo/redo match, add tests

## Acceptance Criteria
- [ ] `undo()` of an archive operation restores entity to live
- [ ] `redo()` of an archive operation re-archives the entity
- [ ] `undo()` of an unarchive operation puts entity back in archive
- [ ] Changelog entries have correct `undone_id` / `redone_id` references
- [ ] No panics or errors in full undo/redo cycles

## Tests
- [ ] `test_undo_archive` — archive entity, undo, verify entity is live
- [ ] `test_redo_archive` — archive, undo, redo, verify entity is archived
- [ ] `test_undo_unarchive` — archive, unarchive, undo unarchive, verify entity is archived
- [ ] `test_archive_undo_redo_cycle` — full cycle, verify final state
- [ ] `test_archive_changelog_has_undone_id` — verify changelog metadata
- [ ] `cargo test -p swissarmyhammer-entity`