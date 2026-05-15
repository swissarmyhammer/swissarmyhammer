---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8580
title: 'Fix: native menu bar not rebuilding from scope chain like palette and context menu do'
---
## What

The command palette and context menu already work correctly — they call `list_commands_for_scope` fresh from the scope chain every time they open. The native menu bar does not. It's built once at startup and manually invalidated from scattered call sites, causing the Window menu to be empty and command availability to go stale.

All three surfaces — menu bar, command palette, right-click menu — are the same thing: commands resolved from the scope chain. Two work. The third needs to catch up.

## Root cause

Menu bar updates use three ad-hoc mechanisms instead of the scope chain:
1. `rebuild_menu` — called from scattered sites (`create_window_impl`, `Destroyed`, `BoardSwitch`, keymap)
2. `update_menu_enabled_state` — after every dispatch, toggles enabled/disabled only
3. `update_window_focus_checkmarks` — OS `Focused(true)`, flips checkmarks only

None flow from the scope chain. The palette and context menu don't have this problem because they resolve fresh on every open.

Additionally, when a window gains OS focus (Alt-Tab, clicking), the frontend does NOT re-dispatch its scope chain. The backend's scope chain stays stale from whichever window last had a click interaction.

## Fix

Two changes:

### 1. Frontend: re-dispatch `ui.setFocus` on window focus

Add a window focus listener in `entity-focus-context.tsx` (or `App.tsx`) that re-dispatches the current entity focus scope chain when the window gains OS focus. This ensures the backend always knows the scope chain of the active window, not just the last-clicked window.

Use Tauri's `getCurrentWindow().onFocusChanged()` or the browser `window` focus event to detect OS focus gain, then call the existing `invokeFocusChange` with the current focused moniker and registry.

### 2. Backend: `rebuild_menu` on every `ui.setFocus`

In `dispatch_command_internal`, when `effective_cmd == "ui.setFocus"`, call `menu::rebuild_menu(app)`. This replaces all ad-hoc menu update mechanisms with a single trigger tied to the scope chain.

This covers every case through one path:
- **Startup**: frontend mounts → `ui.setFocus` → `rebuild_menu` → window appears in menu
- **New window**: `set_focus()` → frontend mounts → `ui.setFocus` → `rebuild_menu`
- **Restore windows**: last restored window gets `set_focus()` → same flow
- **Click a card/tag/column**: `FocusScope` → `ui.setFocus` → `rebuild_menu` → menu reflects scoped commands
- **Switch windows**: OS focus → frontend re-dispatches scope chain → `ui.setFocus` → `rebuild_menu`
- **Close window**: `Destroyed` handler stays (no focus event fires) → `rebuild_menu`

### Files to modify
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — add window focus listener that re-dispatches current scope chain via `invokeFocusChange`
- `kanban-app/src/commands.rs` `dispatch_command_internal` — call `rebuild_menu` when `effective_cmd == "ui.setFocus"`
- `kanban-app/src/main.rs` `Focused(true)` — remove `update_window_focus_checkmarks` call; keep `most_recent_board` bookkeeping only
- `kanban-app/src/commands.rs` `create_window_impl` — remove redundant `rebuild_menu` call (focus event handles it)
- `kanban-app/src/menu.rs` — delete `update_window_focus_checkmarks` (dead code)
- Keep existing `rebuild_menu` calls for `BoardSwitch`/`BoardClose`/keymap (those change registry contents, not scope)

## Why this won't regress

The palette doesn't regress because it resolves fresh every time. The menu bar will now do the same — rebuild on scope chain change. No call sites to remember. You can't interact with a window without focusing something in it, and switching windows re-dispatches automatically.

## Acceptance Criteria
- [ ] After app startup, Window menu shows the main window with a checkmark
- [ ] After `restore_windows`, all windows appear in the Window menu
- [ ] New window (Cmd+N) appears in the Window menu immediately
- [ ] Closing a secondary window removes it from the Window menu
- [ ] Clicking a card updates menu item names and enabled states
- [ ] Alt-tabbing between windows moves the checkmark and updates command availability
- [ ] Quick-capture toggle does not cause unnecessary rebuilds (returns early before focus dispatch)

## Tests
- [ ] Manual: launch app → Window menu lists main window with checkmark
- [ ] Manual: Cmd+N → new window in Window menu
- [ ] Manual: close secondary window → removed from Window menu
- [ ] Manual: click a task card → Edit menu items reflect task scope
- [ ] Manual: Alt-Tab between two windows → checkmark moves, commands update
- [ ] Manual: quit and relaunch with multiple windows → all appear after restore
- [ ] `cargo nextest run -p kanban-app` — no regressions
- [ ] `npx vitest run` in `kanban-app/ui` — no regressions (entity-focus-context tests)