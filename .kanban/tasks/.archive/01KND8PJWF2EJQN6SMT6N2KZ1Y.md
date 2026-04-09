---
assignees:
- claude-code
position_column: todo
position_ordinal: c780
title: '[warning] ViewContainer: non-null assertion on possibly-null board'
---
kanban-app/ui/src/components/view-container.tsx:61

```tsx
<ActiveViewRenderer
  activeView={activeView}
  board={board!}
  ...
```

ViewContainer calls `useBoardData()` which returns `BoardData | null`, then passes `board!` with a non-null assertion. ViewContainer sits inside BoardContainer which guards against null board, but this is an implicit coupling -- if the component hierarchy changes, this assertion will produce a runtime error.

Suggestion: Either add an explicit null guard with early return, or change the prop type to accept `BoardData | null` and handle it in ActiveViewRenderer. #review-finding