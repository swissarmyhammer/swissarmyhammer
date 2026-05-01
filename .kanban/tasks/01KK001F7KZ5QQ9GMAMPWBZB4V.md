---
assignees:
- assistant
position_column: done
position_ordinal: ffff9580
title: Implement kanban domain commands with Command trait
---
Migrate existing `Execute` trait implementations in `swissarmyhammer-kanban` to the new `Command` trait from `swissarmyhammer-commands`. Each command gets both `available()` and `execute()`.

## Scope

- Implement `Command` for: `AddTask`, `MoveTask`, `DeleteTask`, `UntagTask`, `UpdateEntityField`, `DeleteEntity`, `ColumnReorder`
- Implement `Command` for UI commands: `Inspect`, `InspectorClose`, `InspectorCloseAll`, `SetActiveView`, `PaletteOpen`, `PaletteClose`
- Implement `Command` for app commands: `Undo`, `Redo`, `Quit`, `SetKeymapMode`
- Each `available()` uses `CommandContext` to check preconditions:
  - `AddTask::available` — `ctx.has_in_scope("column")`
  - `UntagTask::available` — `ctx.has_in_scope("tag") && ctx.has_in_scope("task")`
  - `Undo::available` — `ctx.can_undo()`
  - `Inspect::available` — target moniker is present
  - `Quit::available` — always true
- Each `execute()` resolves params from context: `ctx.resolve_moniker()`, `ctx.arg()`, defaults
- Move `default_task_title` from React (`task-defaults.ts`) to Rust
- Move `computeOrdinal`/`midpointOrdinal` from React (`board-view.tsx`) to Rust — commands accept `drop_index` and compute ordinal server-side
- Register all commands with `CommandsRegistry`

## Testing

- Test each command's `available()` with matching and non-matching scope chains
- Test `AddTask::execute` resolves column from scope chain, generates default title
- Test `MoveTask::execute` resolves task from scope chain, column from target, computes ordinal from drop_index
- Test `UntagTask::execute` resolves both task and tag from scope chain
- Test `Undo::available` returns false on empty stack, true when operations exist
- Test `Inspect::execute` updates UIState inspector stack
- Test `SetKeymapMode::execute` updates UIState keymap mode
- Integration test: full dispatch through registry → command resolution → execution