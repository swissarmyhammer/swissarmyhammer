---
assignees:
- claude-code
position_column: todo
position_ordinal: b680
title: 'Inline anonymous prop types: extract named interfaces for 10 components'
---
Components using `}: { ... }` anonymous inline prop types instead of named `interface FooProps` pattern:\n\n1. `entity-inspector.tsx` — `FieldDispatch` (7 props)\n2. `entity-card.tsx` — `CardFieldDispatch` (5 props)\n3. `App.tsx` — `ViewRouter` (2 props), `InspectorPanel` (5 props)\n4. `focus-scope.tsx` — `FocusScopeInner` (6 props)\n5. `command-palette.tsx` — `ResultList` (6 props), `ResultRow` (6 props)\n6. `mention-pill.tsx` — `PillInner` (7 props)\n7. `grid-view.tsx` — `GridFocusManager` (3 props)\n8. `command-scope.tsx` — `ActiveBoardPathProvider` (2 props)\n\nEach should get a named `interface FooProps` co-located with its component. #props-slop