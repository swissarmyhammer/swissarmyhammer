---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa680
title: 'Fix: "Do This Next" context menu command not appearing — frontend command lost in backend-driven menu migration'
---
## What

The "Do This Next" command (right-click on a task card to promote it to top of todo column) exists in the frontend code but doesn't appear in context menus. The code is present in `column-view.tsx:170-193` (added in commit `8661d398`), but the context menu system was migrated to be **backend-driven** — native context menus are built by `list_commands_for_scope` on the Rust side. Frontend-only `extraCommands` with `contextMenu: true` are added to the `CommandScope` but never appear in the native menu.

### Root cause
- `column-view.tsx:176` defines `{ id: "task.doThisNext", name: "Do This Next", contextMenu: true, execute: () => ... }`
- This is a frontend-only `CommandDef` passed via `extraCommands` to `useEntityCommands`
- Context menus are built by `kanban-app/src/commands.rs` → `list_commands_for_scope` which only knows about backend-registered commands
- `show_context_menu` in the Rust layer calls `list_commands_for_scope` — it doesn't see frontend-only commands

### Fix approach
Either:
**A) Move to backend**: Register `task.doThisNext` as a proper backend command in `entity.yaml` + implement `DoThisNextCmd` in `task_commands.rs`. It dispatches `task.move` with `before_id` set to the first task in todo column. This is the right approach — aligns with the backend-driven command architecture.

**B) Hybrid**: Keep the frontend execute callback but also register the command ID in the backend so `list_commands_for_scope` includes it. The backend marks it available when a task is in scope.

Option A is cleaner.

### Files to modify
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — add `task.doThisNext` command def with `scope: "entity:task"`, `context_menu: true`, `undoable: true`
- `swissarmyhammer-kanban/src/commands/task_commands.rs` — add `DoThisNextCmd` that reads task's column, finds first task in todo, dispatches `MoveTask` with `before_id`
- `swissarmyhammer-kanban/src/commands/mod.rs` — register `task.doThisNext`
- `kanban-app/ui/src/components/column-view.tsx` — remove frontend-only `buildDoThisNextCommand` and `taskExtraCommands` (now handled by backend)

## Acceptance Criteria
- [ ] "Do This Next" appears in right-click context menu on task cards
- [ ] Clicking it moves the task to the top of the todo column
- [ ] Works from command palette when task is focused
- [ ] Undoable via Cmd+Z
- [ ] Frontend `extraCommands` workaround removed from column-view.tsx

## Tests
- [ ] `swissarmyhammer-kanban/src/commands/mod.rs` — test: `task.doThisNext` available with task in scope
- [ ] `swissarmyhammer-kanban/src/dispatch.rs` — integration test: 3 tasks, doThisNext on last → moves to top of todo
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — all pass