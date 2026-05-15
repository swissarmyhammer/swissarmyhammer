---
assignees:
- claude-code
depends_on:
- 01KM85W10KVXQ4F4JVJCXXPN3A
position_column: done
position_ordinal: ffffffffffffb380
title: Replace active_board_path with most_recent_board_path driven by window focus
---
## What

`active_board_path` is a global singleton that doesn't make sense with multi-window. Each window has its own board via `window_boards`. The global value should reflect \"which board did the user most recently interact with\" for features like quick capture that need a default board.

### Changes
- Rename `active_board_path` → `most_recent_board_path` in UIState
- Update on window focus change: when a window gains focus, set `most_recent_board_path` to that window's board (from `window_boards`)
- Frontend: listen for Tauri window focus events and dispatch a command or call set_focus-like mechanism to update UIState
- Quick capture should read `most_recent_board_path` instead of `active_board_path`
- Update all references (UIState methods, command impls, dispatch_command)

### Window focus detection
Options:
- Tauri `on_window_event(WindowEvent::Focused(true))` in the Rust event handler → update UIState
- Frontend `window.onFocusChanged()` → dispatch to Rust

The Rust event handler approach is simpler — no frontend involvement needed.

## Acceptance Criteria
- [ ] `active_board_path` renamed to `most_recent_board_path`
- [ ] Focusing a window updates `most_recent_board_path` to that window's board
- [ ] Quick capture defaults to the most recently focused board
- [ ] Multi-window: switching between windows correctly tracks which board is \"current\"

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `cargo nextest run -p swissarmyhammer-commands` passes