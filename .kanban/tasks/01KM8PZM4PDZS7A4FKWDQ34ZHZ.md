---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffac80
title: Make active_view_id per-window in UIState
---
## What

`active_view_id` is stored as a flat global in UIState, not per-window. With multi-window, each window should have its own active view (one could show board view, another grid view).

### Changes
- Move `active_view_id` into the per-window `WindowState` in UIState (alongside inspector_stack and geometry)
- Update `UIState.set_active_view()` and `UIState.active_view_id()` to take `window_label`
- Update `ui.view.set` command impl to pass window_label
- Update `to_json()` to include per-window active_view_id
- Frontend: `ViewsProvider` reads active_view_id for its own window, not a global
- Update `useUIState()` or views-context to be window-aware

## Acceptance Criteria
- [ ] Window A on board view + Window B on grid view → each keeps its own view
- [ ] Switching view in one window doesn't affect the other
- [ ] View selection persists per-window across restart

## Tests
- [ ] `cargo nextest run -p kanban-app -p swissarmyhammer-commands` passes
- [ ] `pnpm --filter kanban-app test` passes