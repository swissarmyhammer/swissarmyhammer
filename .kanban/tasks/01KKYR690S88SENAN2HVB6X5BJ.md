---
assignees:
- assistant
position_column: done
position_ordinal: ffffffffffa480
title: Remove tauri-plugin-window-state and add main window geometry persistence
---
## What
Remove the `tauri-plugin-window-state` plugin entirely. Handle all window geometry ourselves through `AppConfig`. This eliminates the dual-persistence race where the plugin and our custom `window_boards` both try to save/restore positions.

### Files
- `kanban-app/Cargo.toml` — Remove `tauri-plugin-window-state` dependency
- `kanban-app/src/state.rs` — Add `WindowGeometry { x, y, width, height, maximized }` struct; add `main_window: Option<WindowGeometry>` to `AppConfig`
- `kanban-app/src/main.rs` — Remove plugin registration + `use` import; replace `win.restore_state(StateFlags::all())` with manual restore from `config.main_window`; extend `on_window_event` to save main window geometry on Moved/Resized (currently skips main)
- `kanban-app/src/commands.rs` — Update `reset_windows` to clear `main_window` + `window_boards` in config instead of deleting `.window-state.json`

### Subtasks
- [ ] Remove `tauri-plugin-window-state` from Cargo.toml
- [ ] Remove plugin registration in main.rs (`.plugin(tauri_plugin_window_state::...)`)
- [ ] Remove `use tauri_plugin_window_state::{StateFlags, WindowExt}` import
- [ ] Add `WindowGeometry` struct (x: i32, y: i32, width: u32, height: u32, maximized: bool)
- [ ] Add `main_window: Option<WindowGeometry>` to `AppConfig` with `#[serde(default)]`
- [ ] In setup(), restore main window position/size from `config.main_window` before `win.show()`
- [ ] Extend `on_window_event` Moved/Resized to also save main window geometry to `config.main_window`
- [ ] Update `reset_windows` — clear config fields instead of deleting `.window-state.json`
- [ ] `cargo nextest run` passes

## Acceptance Criteria
- [ ] `tauri-plugin-window-state` no longer in dependency tree
- [ ] Main window position/size persists across restarts via `config.main_window`
- [ ] Secondary windows still persist via existing `window_boards` geometry fields
- [ ] `reset_windows` resets all window state and restarts cleanly
- [ ] All tests pass

## Tests
- [ ] `cargo nextest run` — full suite green
- [ ] Manual: move main window, quit, restart — restores at same position
- [ ] Manual: move secondary window, quit, restart — restores at same position