---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
title: Fix view switching affecting all windows instead of just the active one
---
## What

Switching views (board ↔ grid) changes the view in ALL windows, not just the one the user clicked in. This is because `active_view_id` is a global field in UIState and `useUIState()` broadcasts changes to all windows.

This is the same root cause as the per-window active_view_id card, but the symptom is more urgent — it's a user-visible regression.

### Root cause
- `ui.view.set` writes to global `UIState.active_view_id`
- `ui-state-changed` event broadcasts to ALL windows
- All windows' `ViewsProvider` reads from the same global and switches

### Fix
This card depends on making active_view_id per-window. Once that's done, the `ui-state-changed` event carries per-window data and each window reads its own view.

Alternatively, if per-window active_view_id isn't ready yet, the quick fix is: `ViewsProvider` should only react to changes for its own window label.

## Acceptance Criteria
- [ ] Switch to grid in window A → window B stays on board view
- [ ] Switch to board in window B → window A stays on grid view

## Tests
- [ ] Manual verification with two windows