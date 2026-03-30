---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Fix Open Board — dialog completes but board doesn't load as active
---
## What

Open Board (File menu or palette) shows the folder picker dialog, user picks a board, but the board doesn't load and show as active. The dialog flow works but the board switch after selection fails silently.

### Suspected root cause
The new `OpenBoardCmd` Command impl returns `{ \"OpenBoardDialog\": true }`, which `dispatch_command_internal` handles by calling `menu::trigger_open_board(app)`. This triggers a folder picker dialog asynchronously. The dialog callback spawns `open_and_notify()` which calls `dispatch_command_internal(\"file.switchBoard\", ...)`.

Potential issues:
1. **Deadlock**: `dispatch_command_internal` calls `update_menu_enabled_state` which does `state.boards.blocking_read()`. If the dialog callback's `open_and_notify` also calls `dispatch_command_internal` → `update_menu_enabled_state` → `blocking_read`, and the first lock is still held, deadlock.
2. **Event timing**: The `board-opened` event may fire before the frontend has processed the board data, causing a race.
3. **The old path worked differently**: The old frontend `execute` callback called `invoke(\"open_board_dialog\")` directly — a dedicated Tauri command that triggers the dialog. The new path goes through `dispatch_command` → result marker → `trigger_open_board`. The extra layer may introduce timing issues.

### Files to investigate
- `kanban-app/src/commands.rs` — `dispatch_command_internal` lines 1009-1010 (OpenBoardDialog handler)
- `kanban-app/src/menu.rs` — `trigger_open_board` → `handle_open_board` → `open_and_notify`
- `kanban-app/src/menu.rs` — `update_menu_enabled_state` — the `blocking_read` on boards

### Fix approach
1. Check for deadlock in `update_menu_enabled_state` when called from `open_and_notify`'s nested dispatch
2. If deadlock: use `try_read` with fallback, or skip menu update in nested dispatches
3. Add logging to trace the flow

## Acceptance Criteria
- [ ] File > Open Board → pick folder → board loads and shows as active
- [ ] Palette > Open Board → same behavior
- [ ] No deadlock or silent failures
- [ ] `cargo nextest run -p kanban-app` passes

## Tests
- [ ] Manual: File > Open Board → pick existing board → board appears
- [ ] Manual: palette open board → same"
<parameter name="assignees">[]