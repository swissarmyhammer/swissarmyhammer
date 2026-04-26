---
position_column: done
position_ordinal: fffff380
title: 'Bug: Drag-and-drop between columns doesn''t work — all cards end up in done'
---
Dragging and dropping tasks between columns does not work. All cards end up in the done column regardless of where they are dropped.

Key files: board-view.tsx (handleDragEnd, handleDragOver, persistMove), column-view.tsx