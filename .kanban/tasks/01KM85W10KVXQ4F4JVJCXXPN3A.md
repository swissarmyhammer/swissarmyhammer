---
assignees:
- claude-code
depends_on:
- 01KM85VKK0GZ27A4N1WX102HBK
position_column: done
position_ordinal: ffffffffffdb80
title: Migrate window state (geometry + inspector_stack) into UIState
---
## What

Window geometry (x, y, width, height, maximized) and per-window inspector_stack live in AppConfig.windows as WindowState. Three Tauri commands manage this: `save_window_geometry`, `restore_windows`, `create_window`.

### Changes
- Add per-window state to UIState: `windows: HashMap<String, WindowState>`
- Move `WindowState` struct into UIState (or a shared types location)
- UIState methods: `save_window_geometry(label, geo)`, `get_window_state(label)`, etc.
- `save_window_geometry` Tauri cmd → dispatch_command or just route to UIState directly (geometry is not undoable)
- `restore_windows` reads from UIState instead of AppConfig
- `create_window` can remain as Tauri cmd (it's OS windowing, not state) but reads board assignment from UIState
- Remove `inspector_stack` from UIState's flat inner (it becomes per-window)
- Remove `WindowState` and `windows` from AppConfig

## Acceptance Criteria
- [ ] Window geometry persists via UIState
- [ ] Inspector stack is per-window in UIState
- [ ] `save_window_geometry` Tauri command removed (or routes through UIState)
- [ ] Window restore on startup works from UIState

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes