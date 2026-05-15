---
assignees:
- claude-code
depends_on:
- 01KMTJ02YMZ071QJ6VWYTZ0X4C
position_column: done
position_ordinal: ffffffffffffffff9280
title: Move UndoCmd/RedoCmd to entity layer + add query API
---
## What

Undo/redo is a property of the entity/command system, not kanban specifically. Move the command implementations so they depend on `EntityContext` directly (via extension), not `KanbanContext`.

**Crate dependency:** `swissarmyhammer-entity` does NOT currently depend on `swissarmyhammer-commands`. This dependency must be added to `swissarmyhammer-entity/Cargo.toml`. This is safe — `swissarmyhammer-commands` has no entity dependency, so no circular risk.

**Move to `swissarmyhammer-entity`:**
- Add `swissarmyhammer-commands` as a dependency in `swissarmyhammer-entity/Cargo.toml`
- Add `swissarmyhammer-entity/src/undo_commands.rs` — `UndoCmd` and `RedoCmd` implementing the `Command` trait
- These use `ctx.require_extension::<Arc<EntityContext>>()` (note: EntityContext is now Arc-wrapped per card 3)
- Remove the old implementations from `swissarmyhammer-kanban/src/commands/app_commands.rs`

**Command behavior:**
- `UndoCmd::execute()` — call `undo_stack.undo_target()` to get the ID automatically, then `ectx.undo(id)`, then `undo_stack.record_undo()` + save. No `id` arg needed.
- `RedoCmd::execute()` — same pattern with `redo_target()` + `ectx.redo(id)` + `record_redo()` + save
- If stack empty, return `Ok(json!({ "noop": true }))` — no error

**Query API — add Tauri command in `kanban-app/src/commands.rs`:**
- `get_undo_state` → returns `{ can_undo: bool, can_redo: bool }` by reading the EntityContext's undo stack
- Register as `#[tauri::command]`

**Files to modify:**
- `swissarmyhammer-entity/Cargo.toml` — add `swissarmyhammer-commands` dependency
- `swissarmyhammer-entity/src/undo_commands.rs` (new)
- `swissarmyhammer-entity/src/lib.rs` — export
- `swissarmyhammer-kanban/src/commands/app_commands.rs` — remove old UndoCmd/RedoCmd
- `swissarmyhammer-kanban/src/commands/mod.rs` — update registrations to use entity crate's commands
- `kanban-app/src/commands.rs` — add `get_undo_state`, register undo/redo from entity crate
- `kanban-app/src/main.rs` — register `get_undo_state`

## Acceptance Criteria
- [ ] `swissarmyhammer-entity` depends on `swissarmyhammer-commands` (no circular dep)
- [ ] UndoCmd/RedoCmd live in entity crate, depend only on EntityContext
- [ ] `app.undo` undoes the most recent operation without needing an ID arg
- [ ] `app.redo` redoes the most recently undone operation
- [ ] Both are no-ops when the stack is empty
- [ ] `get_undo_state` returns can_undo/can_redo from backend
- [ ] Undo/redo is entity-layer infrastructure, reusable outside kanban

## Tests
- [ ] Unit test: write entities → undo → verify state reverted → redo → verify restored
- [ ] Unit test: undo on empty stack returns noop
- [ ] `cargo nextest run -p swissarmyhammer-entity` passes