---
assignees:
- claude-code
position_column: todo
position_ordinal: d280
title: 'Focus steal: BoardFocusBridge unconditionally calls setFocus on every scope change'
---
**Severity: Medium** (Behavioral bug)

**File:** `kanban-app/ui/src/components/board-view.tsx`, lines 57-73 (BoardFocusBridge)

**Problem:** The two-effect split in BoardFocusBridge is intended to separate scope registration (effect 1) from focus changes (effect 2, fires only on moniker change). However, effect 2 depends on `[mk, setFocus]` -- and `setFocus` is a stable `useCallback` with `[]` deps, so that's fine. The real issue is that `mk` (the focus bridge moniker) can change even when the user hasn't moved the cursor, because `focusBridgeMoniker` is recomputed whenever `currentBoardEntity` changes identity. If the entity store emits a new Entity object for the same task (e.g. a field update from the inspector), `currentBoardEntity` gets a new reference, `moniker("task", sameId)` produces the same string, so `mk` stays the same -- this is fine due to string equality.

However, when the inspector is open and focused on a field (e.g. `task:abc.title`), and a *different* entity's data updates causing the board's tasks array to re-render, the board cursor doesn't move but `focusBridgeMoniker` stays `task:abc`. Effect 2 won't re-fire (same `mk`). So **this specific theft path is safe**.

**Actual remaining risk:** On initial mount or when the board view replaces the grid view, effect 2 will call `setFocus(mk)` unconditionally, which could steal focus from the inspector if it's still mounted. The old FocusClaim used a LIFO stack that prevented this. Now there's no guard -- the last `setFocus` call wins.

**Recommendation:** Add a guard to effect 2: only call `setFocus` if the board is the active view or if focus is not currently inside the inspector scope chain. Alternatively, skip setFocus on mount when another view already has focus. #review-finding