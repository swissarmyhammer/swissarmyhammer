---
assignees:
- claude-code
depends_on:
- 01KM6AK9FH267W0VAES4MRF7M6
position_column: todo
position_ordinal: '8180'
title: 'Add #[operation] macro to ArchiveTask + create UnarchiveTask and ListArchived operations'
---
## What

The existing `ArchiveTask` in `swissarmyhammer-kanban/src/task/archive.rs` was written without the `#[operation]` macro. Add it so the operation gets verb/noun metadata for schema generation. Also create `UnarchiveTask` and `ListArchived` operations.

### Changes

**ArchiveTask** (update existing in `task/archive.rs`):
- Add `#[operation(verb = \"archive\", noun = \"task\", description = \"Archive a task and clean up dependencies\")]`
- Add `Serialize` derive (required by the macro)

**UnarchiveTask** (new, in `task/archive.rs` or new file):
- `#[operation(verb = \"unarchive\", noun = \"task\", description = \"Restore an archived task\")]`
- Takes `id: TaskId`
- Calls `ectx.unarchive(\"task\", id)`

**ListArchived** (new, in `task/archive.rs` or new file):
- `#[operation(verb = \"list\", noun = \"archived\", description = \"List archived tasks\")]`
- Calls `ectx.list_archived(\"task\")`
- Returns `{ tasks: [...], count: N }`

### Files
- `swissarmyhammer-kanban/src/task/archive.rs` — update ArchiveTask, add UnarchiveTask, ListArchived
- `swissarmyhammer-kanban/src/task/mod.rs` — export new types
- `swissarmyhammer-kanban/src/schema.rs` — register all three in KANBAN_OPERATIONS

## Acceptance Criteria
- [ ] `ArchiveTask` has `#[operation]` metadata
- [ ] `UnarchiveTask` restores a task from archive
- [ ] `ListArchived` returns archived tasks with count
- [ ] All three appear in `kanban_operations()` for schema generation

## Tests
- [ ] `test_unarchive_task` — archive then unarchive, verify task is live
- [ ] `test_list_archived` — archive some tasks, verify list returns them
- [ ] `cargo test -p swissarmyhammer-kanban`