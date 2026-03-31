---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: 'Bug: secondary windows not restoring on app restart'
---
## What

Secondary board windows (created via `window.new` / Cmd+Shift+N) are not restored when the app is restarted.

### Progress so far

Persistence bugs are fixed (Bugs 1-3 from earlier). Window entries now survive quit correctly in `ui-state.yaml` with valid geometry. What remains: **the restore path doesn't work** — `restore_windows` was a frontend-invoked special function that was unreliable (React strict mode cancellation) and invisible to the command log.

### Remaining work: make window operations real commands

Replace the magic `restore_windows` function with composable commands that flow through `dispatch_command` and show up in the activity log.

**1. Extend `create_window_impl` to accept optional `label` and `geometry`**
- File: `kanban-app/src/commands.rs:640-720`
- Currently only accepts `board_path`. Add optional `label: Option<String>` (reuse saved label instead of generating new ULID) and optional geometry `(x, y, width, height, maximized)`
- When label is provided, use it instead of `new_window_label()`
- When geometry is provided, apply it after build (position + size)
- This makes `create_window` the single path for all window creation — new and restore

**2. Startup loop in `setup()` calls `create_window_impl` for each saved entry**
- File: `kanban-app/src/main.rs` setup block (already partially added)
- Read `all_windows()`, skip main/quick-capture, call `create_window_impl` with label + board_path + geometry
- Each call goes through the same code path as `window.new`, fully observable

**3. Delete `restore_windows` command**
- File: `kanban-app/src/commands.rs` — remove `restore_windows` function
- File: `kanban-app/src/main.rs` — remove from `invoke_handler` registration
- File: `kanban-app/ui/src/App.tsx:347-348` — remove `invoke("restore_windows")` call

**4. Remove the setup() restore block just added**
- The inline restore in setup() was a stopgap — replace with calls to `create_window_impl`

### Files to modify

- `kanban-app/src/commands.rs` — extend `create_window_impl`, delete `restore_windows`
- `kanban-app/src/main.rs` — setup() restore loop uses `create_window_impl`, remove from invoke_handler
- `kanban-app/ui/src/App.tsx` — remove `invoke("restore_windows")` call

## Acceptance Criteria

- [ ] Secondary windows created with Cmd+Shift+N reappear at saved positions after app restart
- [ ] Window geometry (position + size) is preserved across restarts
- [ ] Each restored window shows the correct board
- [ ] Mid-session close (X button) still removes window entry so it doesn't resurrect
- [ ] No `restore_windows` command — restore uses same `create_window` path as `window.new`
- [ ] Window creation during restore is visible in logs (same code path as user-initiated)
- [ ] No zombie window entries in `ui-state.yaml`

## Tests

- [x] Unit test: `update_window_geometry` round-trips, ignores unknown labels, doesn't resurrect removed windows, memory-only (no disk), persisted by explicit save
- [x] Unit test: `save_window_geometry` + `get_window_state` round-trip
- [x] Unit test: `remove_window` fully removes entry
- [ ] Verify `create_window_impl` accepts label + geometry params and applies them
- [ ] Manual test: create secondary window, quit, relaunch — window reappears at correct position
