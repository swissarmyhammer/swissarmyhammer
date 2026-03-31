---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: Remove optimistic inspector panel state — derive from backend UIState only
---
## What

The inspector panel stack in `App.tsx` currently has two competing sync paths:

1. **Optimistic local state** — `closeTopPanel`, `dismissTopPanel`, `closeAll` call `setPanelStack(...)` immediately for instant visual feedback, then reconcile from the command response via `parsePanelStack()`.
2. **`InspectorSyncBridge`** (line 72–95) — reactively reads `useUIState().windows[WINDOW_LABEL].inspector_stack` and overwrites `panelStack` whenever UIState changes.

This dual-path creates complexity and potential divergence. The fix: **remove all optimistic `setPanelStack` calls** and let `InspectorSyncBridge` be the single source of truth.

### Files to modify

- **`kanban-app/ui/src/App.tsx`** (primary)
  - `inspectEntity` (line ~184): remove `.then(res => { parsePanelStack; setPanelStack })` — just fire-and-forget the `ui.inspect` command
  - `closeTopPanel` (line ~200): remove optimistic `setPanelStack(prev.slice(0,-1))` and the `.then` reconciliation — just dispatch `ui.inspector.close`
  - `dismissTopPanel` (line ~217): same treatment; still returns `boolean` based on `panelStackRef.current.length`
  - `closeAll` (line ~235): remove optimistic `setPanelStack([])` and `.then` — just dispatch `ui.inspector.close_all`
  - Remove `parsePanelStack` helper (line 135–153) — no longer needed since we never read the command response
  - Remove manual inspector restore in `useEffect` mount (line ~326–338) — `InspectorSyncBridge` handles this via `get_ui_state` already
  - Keep `InspectorSyncBridge` as-is — it becomes the **only** writer to `panelStack`

### Approach

Fire-and-forget: each inspector command dispatches to backend, backend updates UIState, UIState change propagates through `UIStateProvider` → `useUIState()` → `InspectorSyncBridge` → `setPanelStack`. The UI re-renders when the backend state arrives.

## Acceptance Criteria

- [ ] No `setPanelStack` call outside of `InspectorSyncBridge`
- [ ] `parsePanelStack` helper is removed
- [ ] Inspector open/close/close-all still work via command dispatch
- [ ] Inspector stack restores correctly on window mount (via `InspectorSyncBridge` reading UIState)
- [ ] No visual regression — panels appear/disappear when commands execute
- [ ] `panelStackRef` still works for `dismissTopPanel` boolean return

## Tests

- [ ] Manual: click entity → inspector opens; click close → inspector closes; click backdrop → all close
- [ ] Manual: open inspector, reload page → inspector restores from backend UIState
- [ ] Manual: open multiple inspectors, close top one → correct panel removed
- [ ] `cargo nextest run` — existing `inspector_close_pops`, `inspector_close_all_clears`, `inspector_stack_empty_for_unknown_window` tests in `swissarmyhammer-commands/src/ui_state.rs` still pass (backend unchanged)
- [ ] Verify no TypeScript errors: `cd kanban-app && npx tsc --noEmit`