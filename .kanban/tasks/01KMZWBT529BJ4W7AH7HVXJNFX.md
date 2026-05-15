---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe780
title: 'Fix: inspector opens in wrong window — pass windowLabel in context-menu and entity-command dispatch'
---
## What

When "Inspect" (or any command) is dispatched from a context menu or entity-command handler in a **non-main window**, the inspector panel opens in the first/main window instead of the invoking window. The root cause is that `windowLabel` is not passed to the Rust `dispatch_command` in two frontend dispatch paths, so the backend falls back to `"main"` (see `ui_commands.rs:46`).

### Files to modify

1. **`kanban-app/ui/src/lib/context-menu.ts`** — `dispatchContextMenuCommand()` at line 33: add `windowLabel: getCurrentWindow().label` to the `invoke("dispatch_command", ...)` call.
2. **`kanban-app/ui/src/lib/entity-commands.ts`** — two `invoke("dispatch_command", ...)` calls at lines 87 and 136: add `windowLabel: getCurrentWindow().label` (or pass it through from the hook's scope, similar to how `boardPath` is already threaded).

### Context

- `command-scope.tsx:dispatchCommand()` already correctly passes `windowLabel: getCurrentWindow().label` — these two files need the same fix.
- The backend `InspectCmd::execute()` in `swissarmyhammer-kanban/src/commands/ui_commands.rs:46` defaults `window_label` to `"main"` when `ctx.window_label` is `None`.
- `UIState::inspect()` stores the inspector stack per window label in `UIStateInner::windows` HashMap.

## Acceptance Criteria

- [x] Right-clicking an entity in a non-main window and selecting "Inspect" opens the inspector in **that** window, not the main window
- [x] All three dispatch paths (`context-menu.ts`, `entity-commands.ts` x2) pass `windowLabel` to `dispatch_command`
- [x] Existing single-window behavior is unchanged (main window still works as before)

## Tests

- [x] Update `kanban-app/ui/src/lib/context-menu.test.tsx` — verify `dispatchContextMenuCommand` passes `windowLabel` in the `dispatch_command` invoke call
- [x] Update `kanban-app/ui/src/lib/command-scope.test.tsx` — existing tests already verify `windowLabel` is passed via `dispatchCommand`; confirm no regressions
- [x] `cd kanban-app && npx vitest run` — all frontend tests pass