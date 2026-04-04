---
assignees:
- claude-code
position_column: todo
position_ordinal: b380
title: Remove client-side entity mutation commands from grid-view
---
## What

`grid-view.tsx` has three commands with frontend `execute` handlers that construct entity mutation args in React, violating the commands-in-rust architecture:

1. **`grid.deleteRow`** (line ~371) — reads `entities[row]`, dispatches `task.archive` with entity id
2. **`grid.newBelow`** (line ~388) — dispatches `task.add` with hardcoded title
3. **`grid.newAbove`** (line ~404) — same as newBelow

### Archive fix (straightforward)

`entity.archive` already exists in the task schema (`task.yaml:11`) and resolves from `ctx.target` in Rust (`entity_commands.rs:90-95`). The `entityCommands` scope now wraps the DataTable, so the cursor row's entity target is in the scope chain. **Just remove `grid.deleteRow` and add `keys: { vim: \"d\" }` to `entity.archive` in `task.yaml`.**

### Add fix (needs column resolution)

`task.add` in Rust (`task_commands.rs:18-19`) requires `column` in scope or as an arg. The grid has no column context. Options:
- Add a `column` arg to `task.add` from the grid (reads from the entity's `position_column` field, or first column as default) — keeps the Rust command as-is
- Add a Rust `grid.addRow` command that resolves the default column from the active perspective or first column in the board

Recommended: pass `column` arg derived from the cursor entity's `position_column` if available, else omit and let Rust use the first column (requires a small Rust change to default to first column when no column in scope).

**Files to modify:**
- `kanban-app/ui/src/components/grid-view.tsx` — remove execute handlers from grid.deleteRow/newBelow/newAbove, replace with backend dispatch
- `swissarmyhammer-kanban/builtin/fields/entities/task.yaml` — add keys to `entity.archive`
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — (optional) default to first column when no column in scope or args

## Acceptance Criteria
- [ ] `grid.deleteRow` has no execute handler — dispatches `entity.archive` through scope chain
- [ ] `grid.newBelow` / `grid.newAbove` dispatch `task.add` to backend (no client-side arg construction beyond column)
- [ ] No `entities[row]` lookup in grid command execute handlers
- [ ] Archive from grid works (vim `d`, palette)
- [ ] New row from grid works (vim `o`/`O`, Mod+Enter)

## Tests
- [ ] `cd kanban-app/ui && pnpm vitest run` — all unit tests pass
- [ ] Rust: `cargo test -p swissarmyhammer-kanban` — all tests pass
- [ ] Manual: grid view — archive row with `d`, new row with `o`, inspect with `i`