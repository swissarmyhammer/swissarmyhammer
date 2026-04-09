---
assignees:
- claude-code
position_column: todo
position_ordinal: c680
title: '[warning] InspectorContainer: same identity no-op useMemo pattern'
---
kanban-app/ui/src/components/inspector-container.tsx:61

```tsx
const entityStore = useMemo(() => entitiesByType, [entitiesByType]);
```

Same issue as RustEngineContainer -- this useMemo returns its own dependency. It provides no memoization benefit and adds cognitive overhead.

Suggestion: Remove the useMemo and use `entitiesByType` directly. #review-finding