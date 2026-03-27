---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff880
title: Inspector unmount restore captures stale focusedMoniker
---
**Severity: Medium** (Behavioral bug)

**File:** `kanban-app/ui/src/components/entity-inspector.tsx`, lines 146-159

**Problem:** The mount effect captures `focusedMoniker` at mount time into `prevFocusRef.current`:

```typescript
useEffect(() => {
  if (!firstFieldMoniker) return;
  prevFocusRef.current = focusedMoniker;
  setFocus(firstFieldMoniker);
  return () => {
    setFocus(prevFocusRef.current);
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, [firstFieldMoniker, setFocus]);
```

Because the deps are `[firstFieldMoniker, setFocus]`, the cleanup function is a closure over the *initial* `prevFocusRef`. The ref itself is fine (refs are mutable), but `focusedMoniker` is captured once and written to the ref at mount time. If the user navigated the board cursor *before* opening the inspector, `focusedMoniker` is the previous board task. On unmount, focus restores to that board task moniker.

**But:** If `firstFieldMoniker` changes (e.g. entity swap in the inspector), the effect re-runs. The new invocation captures the *current* `focusedMoniker` -- which is now one of the inspector's own field monikers. On unmount, it would restore to an inspector field moniker that no longer exists. The old claim stack handled this cleanly by restoring to the claim beneath.

**Recommendation:** Capture `prevFocusRef` only on the very first mount, not on every effect re-run. Guard with `if (prevFocusRef.current === null)` or use a separate one-time ref. #review-finding