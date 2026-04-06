---
assignees:
- claude-code
position_column: todo
position_ordinal: c580
title: '[warning] RustEngineContainer: entityStore memo is identity no-op'
---
kanban-app/ui/src/components/rust-engine-container.tsx:326

```tsx
const entityStore = useMemo(() => entitiesByType, [entitiesByType]);
```

This useMemo wraps `entitiesByType` as its own dependency. It returns the same reference that was passed in -- it never creates a new reference. The useMemo is a no-op; it can be removed. The `EntityStoreProvider entities={entitiesByType}` prop can reference `entitiesByType` directly.

Suggestion: Remove the useMemo and pass `entitiesByType` directly to `EntityStoreProvider`. #review-finding