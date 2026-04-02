---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffab80
title: Fix PerspectiveTabBar context menu — use backend command system, not custom React menu
---
## What

`perspective-tab-bar.tsx` hand-rolls a custom React context menu (lines 106-117, 170-196) with `onContextMenu`, local `{x, y}` state, and a positioned `<div>` for Rename/Delete. This bypasses the backend command system that every other context menu in the app uses.

### Current (wrong)
- `onContextMenu` captures click position into local React state
- Renders a custom `<div>` dropdown with hardcoded \"Rename\" and \"Delete\" buttons
- Directly calls `backendDispatch({ cmd: \"perspective.delete\" })` from the button
- Does NOT use `useContextMenu()`, does NOT call `show_context_menu`

### Correct pattern (used everywhere else)
- `useContextMenu(scopeChain)` from `kanban-app/ui/src/lib/context-menu.ts`
- Calls `invoke(\"list_commands_for_scope\", { scopeChain, contextMenu: true })` to get available commands from backend
- Calls `invoke(\"show_context_menu\", { items })` to show a **native OS context menu** via Tauri
- On selection, dispatches through `backendDispatch()` with proper scope chain

### What needs to happen
1. Wrap each perspective tab in a `CommandScopeProvider` with `moniker={moniker(\"perspective\", perspectiveId)}`
2. Use `useContextMenu()` hook for `onContextMenu` handler instead of custom state
3. Remove the custom React dropdown `<div>` and all `contextMenu`/`setContextMenu` local state
4. Ensure `perspective.delete` and any rename commands have `context_menu: true` and `scope: \"entity:perspective\"` in `perspective.yaml` so they appear in the native menu
5. If rename needs inline editing (which native menus can't do), keep it as a separate interaction triggered by double-click or a command, not via a fake context menu

### Files to modify
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — Replace custom context menu with `useContextMenu()` pattern, wrap tabs in `CommandScopeProvider`
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — Ensure delete/rename commands have `context_menu: true` and `scope: \"entity:perspective\"`

## Acceptance Criteria
- [ ] Right-click on a perspective tab shows a **native OS context menu** (not a React div)
- [ ] Menu items come from the backend command system via `list_commands_for_scope`
- [ ] Custom React context menu div and state completely removed
- [ ] Each perspective tab has a `CommandScopeProvider` with `perspective:{id}` moniker
- [ ] Delete works through the native menu
- [ ] Rename triggered by double-click or separate interaction (not via fake context menu)

## Tests
- [ ] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — update tests, verify `onContextMenu` calls `useContextMenu` hook
- [ ] `pnpm test` from `kanban-app/ui/` — all pass