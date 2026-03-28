---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9e80
title: EntityInspector focus restore reads stale focusedMoniker on mount
---
**Severity: Medium (Functionality)**

In `kanban-app/ui/src/components/entity-inspector.tsx`, the mount effect captures `focusedMoniker` from the hook at first render:

```tsx
if (!mountedRef.current) {
  prevFocusRef.current = focusedMoniker;
  mountedRef.current = true;
}
setFocus(firstFieldMoniker);
return () => {
  setFocus(prevFocusRef.current);
};
```

The `focusedMoniker` value comes from the render-time state of `useEntityFocus()`. Since `setFocus(firstFieldMoniker)` is called in the same effect, the cleanup function will restore to whatever `prevFocusRef.current` was at mount time. However, if the board layout changes while the inspector is open (e.g., a task is deleted), `prevFocusRef.current` may point to a moniker that no longer exists.

**Recommendation:** On cleanup, check if `prevFocusRef.current` still exists in the scope registry before restoring. If it does not, fall back to null or the nearest valid moniker. #review-finding