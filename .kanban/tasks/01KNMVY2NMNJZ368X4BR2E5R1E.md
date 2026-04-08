---
assignees:
- claude-code
position_column: todo
position_ordinal: a780
title: Fix "New Task" command appearing in command palette on non-task grid views
---
## What

The `task.add` ("New Task") command appears in the command palette on grid views that display non-task entities (e.g. tags grid, projects grid). This is confusing — on a tags grid, the only "add" command should be for tags, not tasks.

**Root cause:** In `swissarmyhammer-kanban/src/scope_commands.rs`, `commands_for_scope()` collects commands from ALL entity definitions whose `scope` field matches monikers in the current scope chain. The `task.add` command has `scope: "entity:column"`, and if a column moniker leaks into the scope chain (e.g. from a previously focused board view), `task.add` appears in non-task contexts. Additionally, `task.add` may be included as a global/fallback command.

**Files to investigate and modify:**
- `swissarmyhammer-kanban/src/scope_commands.rs` — `commands_for_scope()` function. The fix should filter entity commands so that only commands matching the current view's entity type appear, or ensure the scope chain is properly cleaned when switching views.
- `kanban-app/ui/src/lib/command-scope.tsx` — verify that `ui.setFocus` properly resets the scope chain when switching between views (so stale column monikers from board views don't persist into grid views).

**Approach:** The most targeted fix is in `commands_for_scope()`: when resolving commands for a `view:*` moniker, read the view's `entity_type` and only include entity commands that match that type. This ensures the palette shows `tag.add` on a tags grid and `task.add` on a tasks grid/board view.

## Acceptance Criteria
- [ ] Command palette on tags-grid view does NOT show "New Task" (`task.add`)
- [ ] Command palette on projects-grid view does NOT show "New Task" (`task.add`)
- [ ] Command palette on tasks-grid/board view still shows "New Task" (`task.add`)
- [ ] Grid keyboard commands (`grid.newBelow`, `grid.newAbove`) still work correctly per entity type

## Tests
- [ ] Add test in `swissarmyhammer-kanban/src/scope_commands.rs` — `commands_for_scope` with a tags-grid view moniker should not include `task.add`
- [ ] Add test — `commands_for_scope` with a tasks-grid view moniker should include `task.add`
- [ ] Run: `cargo test -p swissarmyhammer-kanban` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.