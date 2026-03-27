---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8480
title: 'App.tsx: InspectorPanel uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/App.tsx:691`\n\n`InspectorPanel` has 5 props (`entry`, `entityStore`, `board`, `onClose`, `style`) defined inline. Extract to `interface InspectorPanelProps`. #props-slop