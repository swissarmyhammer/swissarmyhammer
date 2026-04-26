---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffab80
title: Fix inspector stack not restoring on restart/hot reload
---
## What

Inspector stack is saved to UIState per-window (confirmed in ui-state.yaml), but on restart or hot reload the inspector panels don't reappear.

### Investigation needed
- Check if `get_ui_context` returns the inspector stack from UIState correctly
- Check if the frontend reads and restores inspector stack on mount (App.tsx startup logic)
- The old code restored from `AppConfig.windows` — verify the new path works end-to-end

## Acceptance Criteria
- [ ] Open inspector for a task, restart app → inspector reopens
- [ ] Hot reload preserves open inspectors

## Tests
- [ ] Manual verification with app restart