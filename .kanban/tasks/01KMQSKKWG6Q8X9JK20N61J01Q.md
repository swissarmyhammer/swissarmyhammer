---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffbc80
title: popClaim dispatches redundant IPC when removing non-active claim
---
**Severity: Low / Performance**

In `entity-focus-context.tsx` lines 181-208, `popClaim` unconditionally calls `setFocusedMoniker` and `invokeFocusChange` after removing a claim, even when the removed claim was NOT the active (topmost) one. This means popping a background claim triggers:
1. A React state update (no-op if same value, but still enqueued)
2. A Rust IPC call to `dispatch_command("ui.setFocus", ...)` with the same scope chain

**Fix:** Before dispatching, compare the new active claim's moniker to the current `focusedMonikerRef.current`. Only call `setFocusedMoniker`/`invokeFocusChange` if the value actually changed:
```ts
const newMoniker = active?.moniker ?? null;
if (newMoniker !== focusedMonikerRef.current) {
  focusedMonikerRef.current = newMoniker;
  setFocusedMoniker(newMoniker);
  invokeFocusChange(newMoniker, registryRef);
}
```

**File:** `kanban-app/ui/src/lib/entity-focus-context.tsx` #review-finding