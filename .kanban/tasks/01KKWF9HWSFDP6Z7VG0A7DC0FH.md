---
assignees:
- claude-code
depends_on:
- 01KKWF8YVWQ4XNC24DV8YCK650
position_column: done
position_ordinal: ffffffffd880
title: Cross-board task transfer and copy operations
---
## What
Implement `task.transfer` (move between boards) and `task.copy` (copy between boards) operations in the kanban crate. These are the backend primitives that `complete_drag_session` delegates to for cross-board drops.

**Files:**
- `swissarmyhammer-kanban/src/task/transfer.rs` (new) — TransferTask and CopyTask operations
- `swissarmyhammer-kanban/src/task/mod.rs` — re-export new operations
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — register `task.transfer` and `task.copy` commands

**TransferTask operation:**
- Takes: source_board_path, entity_id, target_board_path, target_column, before_id?, after_id?
- Reads entity from source board's EntityContext
- Creates new entity in target board with same title, description, body, priority, effort, custom fields
- Computes ordinal for target position (reuse `compute_ordinal_for_neighbors`)
- Sets position_column to the target column (visual targeting — user chose the column)
- Strips tags that don't exist in target board (no auto-creation of tags)
- Deletes entity from source board
- Returns affected resource IDs for both boards (triggers events in both windows)

**CopyTask operation:**
- Same as TransferTask but does NOT delete from source board
- Generates a new entity ID for the copy

**Field mapping:**
- Portable: title, description, body, priority, effort, any custom string/number/date fields
- Position: set fresh (target column + computed ordinal)
- Tags: only include tags whose slug exists in target board
- Assignees: preserve (actor IDs are OS usernames, shared across boards)

## Acceptance Criteria
- [ ] TransferTask reads from source, creates in target, deletes from source
- [ ] CopyTask reads from source, creates in target, keeps source intact
- [ ] Target column is set to the user-specified column (visual targeting)
- [ ] Tags stripped if not present in target board
- [ ] Ordinal computed correctly for before_id/after_id positioning
- [ ] Commands registered and routable via dispatch_command

## Tests
- [ ] Test: transfer task appears in target board, gone from source
- [ ] Test: copy task appears in target board, still in source
- [ ] Test: tags that don't exist in target are stripped
- [ ] Test: ordinal positioning works with before_id/after_id
- [ ] `cargo nextest run -p swissarmyhammer-kanban`