---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd180
title: 'Fix: FileDropProvider global event prevention breaks HTML5 card drag-and-drop'
---
## What

`FileDropProvider` in `kanban-app/ui/src/lib/file-drop-context.tsx:79-88` adds global `document.addEventListener("dragover", preventDragOver)` and `document.addEventListener("drop", preventDrop)` that call `e.preventDefault()` on **all** drag events. This was added to prevent the browser/webview from navigating when files are dragged from Finder.

However, this globally prevents the native HTML5 drag events that card dragging relies on (`sortable-task-card.tsx`, `drop-zone.tsx`, `board-view.tsx`). The symptoms:

1. **Global `preventDrop` on document** — when a card is dropped on an area that isn't a `DropZone` (column header, whitespace, gaps between zones), the global handler calls `preventDefault()` which changes the `dropEffect` from `"none"` to `"move"`. This makes `handleTaskDragEnd` in `board-view.tsx:406-418` think the drop succeeded (it only cancels the drag session when `dropEffect === "none"`), so the backend session is left dangling.

2. **Global `preventDragOver` on document** — makes every pixel of the page appear as a valid drop target for card drags (browser shows "move" cursor everywhere instead of "not allowed" for non-drop areas), breaking the visual feedback that guides users to valid `DropZone` targets.

### Fix approach

The global `dragover`/`drop` prevention must distinguish between **native file drags** (from Finder/OS) and **HTML5 card drags** (internal). Two options:

**Option A (recommended):** Check `e.dataTransfer.types` in the global handlers. Native file drags include `"Files"` in the types array. Card drags include `"application/x-swissarmyhammer-task"`. Only call `preventDefault()` when the drag is a file drag:

```typescript
const preventDragOver = (e: DragEvent) => {
  if (e.dataTransfer?.types.includes("Files")) {
    e.preventDefault();
  }
};
const preventDrop = (e: DragEvent) => {
  if (e.dataTransfer?.types.includes("Files")) {
    e.preventDefault();
  }
};
```

**Option B:** Track whether a card drag is active (via a shared ref or context) and skip prevention during card drags.

### Files to modify

- `kanban-app/ui/src/lib/file-drop-context.tsx` — fix global event handlers (lines 79-88)
- `kanban-app/ui/src/lib/file-drop-context.test.tsx` — add tests for coexistence
- `kanban-app/ui/src/components/board-drag-drop.test.tsx` — add integration-level test

## Acceptance Criteria

- [ ] Dragging a card between columns works: cursor shows "move" only over `DropZone` elements, "not allowed" elsewhere
- [ ] Dropping a card outside a valid `DropZone` results in `dropEffect === "none"` and the drag session is cancelled
- [ ] Dragging a file from Finder still shows the drop indicator on attachment displays
- [ ] Dropping a file from Finder does NOT navigate the webview
- [ ] Card drags do NOT trigger `isDragging` in `FileDropContext`

## Tests

- [ ] `kanban-app/ui/src/lib/file-drop-context.test.tsx`: add test — global `dragover` handler calls `preventDefault()` when `dataTransfer.types` includes `"Files"`, does NOT call `preventDefault()` for card MIME type
- [ ] `kanban-app/ui/src/lib/file-drop-context.test.tsx`: add test — global `drop` handler calls `preventDefault()` when `dataTransfer.types` includes `"Files"`, does NOT call `preventDefault()` for card MIME type
- [ ] `kanban-app/ui/src/components/board-drag-drop.test.tsx`: add test — `DropZone` `handleDrop` receives the event when the global file-drop prevention is active (simulates both systems coexisting)
- [ ] Run: `cd kanban-app/ui && npx vitest run` — all tests pass