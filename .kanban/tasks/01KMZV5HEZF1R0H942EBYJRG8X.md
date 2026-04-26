---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe080
title: Inspect does nothing from context menu or palette
---
## What

Clicking Inspect Task/Tag/Column/Board from context menu or palette does nothing. The inspector panel does not open.

### Root cause
`useUIState()` is called in the `App` function (line 147) which is OUTSIDE the `UIStateProvider` (line 554). The hook returns default empty state. The `useEffect` that syncs inspector_stack from UIState never sees real data.

### Fix
Move the inspector sync logic into a component that renders INSIDE `UIStateProvider`. Create an `InspectorSyncBridge` component that uses `useUIState()` and calls `setPanelStack` via a callback or ref.

### Files to modify
- `kanban-app/ui/src/App.tsx` — move inspector sync useEffect into a child component inside UIStateProvider

## Acceptance Criteria
- [ ] Right-click → Inspect Task → inspector opens
- [ ] Double-click task → inspector opens
- [ ] Palette → Inspect Task → inspector opens
- [ ] Works for tag, column, board too

## Tests
- [ ] `cargo nextest run -p kanban-app` passes"
<parameter name="assignees">[]