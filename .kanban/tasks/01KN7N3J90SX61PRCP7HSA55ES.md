---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd180
title: Fix task card drag-and-drop broken by FileDropProvider global event handlers
---
## Problem

The `FileDropProvider` (file-drop-context.tsx:79-88) registers global `document.addEventListener("dragover/drop")` handlers that call `e.preventDefault()` unconditionally on ALL drag events. This was added so the browser doesn't navigate to dropped files from Finder.

However, HTML5 drag-and-drop requires `preventDefault()` on `dragover` to mark an element as a valid drop target. The global handler makes the **entire document** a valid drop target, which:

1. **Breaks within-board card drag**: Drop events land on the document instead of on the intended DropZone (the thin 12px horizontal bars between cards). The global `drop` handler swallows the event.
2. **Breaks cross-board card drag**: `dragEnd` sees `dropEffect !== "none"` even when nothing handled the drop, because the document accepted it. This confuses the `DragSession` lifecycle (start/cancel/complete via Tauri events).
3. **DropZone stopPropagation doesn't fully help**: While DropZone calls `stopPropagation` (preventing events from reaching document when a DropZone IS hit), drops that miss the thin target still reach the document handler.

## Root Cause Analysis

Three drag systems coexist:

| System | API | Interference? |
|--------|-----|---------------|
| Task card drag | HTML5 Drag (`dataTransfer`) | YES — broken by global handlers |
| File drop from Finder | Tauri native `onDragDropEvent()` + browser HTML5 | The source of the bug |
| Column reorder | dnd-kit `PointerSensor` (pointer events) | NO — completely isolated |

**Key fact**: `dataTransfer.types` is `["Files"]` for Finder drags and `["application/x-swissarmyhammer-task"]` for task card drags. These are **mutually exclusive** — a drag originates from either the OS or a DOM element, never both. This makes MIME-based discrimination 100% reliable.

**Why Tauri doesn't save us**: Even with `dragDropEnabled: true` in `tauri.conf.json`, the browser still fires HTML5 `dragover`/`drop` DOM events alongside Tauri's native `onDragDropEvent()`. Without `preventDefault` on the DOM events, the browser navigates to `file:///path`. So the global handlers ARE necessary — but only for file drags.

**Secondary windows**: `commands.rs:689` calls `disable_drag_drop_handler()` on dynamically-created windows. These don't get Tauri native file drop events, but the `document` listeners still prevent browser navigation. File drop attachment won't work on secondary windows regardless — this is a pre-existing limitation, not part of this fix.

## Fix

### 1. Discriminate drag types in FileDropProvider global handlers

In `kanban-app/ui/src/lib/file-drop-context.tsx` (lines 79-88):

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

### 2. Add Files type check to DropZone

In `kanban-app/ui/src/components/drop-zone.tsx`, add early return in `handleDragOver` and `handleDragEnter` when `e.dataTransfer.types.includes("Files")`. This prevents the blue indicator bar from appearing when files are dragged over a DropZone (the drop is already a no-op since `getData(DRAG_MIME)` returns empty, but the visual highlight is misleading).

### 3. Integration tests for coexistence

See the dedicated test cards for each drag scenario.

## Files to modify

- `kanban-app/ui/src/lib/file-drop-context.tsx` — add `Files` type check to global handlers (lines 79-88)
- `kanban-app/ui/src/components/drop-zone.tsx` — add `Files` type check to `handleDragOver`/`handleDragEnter`

## Acceptance Criteria

- [ ] Task card drag within a column reorders correctly
- [ ] Task card drag between columns moves the card
- [ ] Cross-board task card drag (between windows) completes successfully
- [ ] File drop from Finder onto attachment field still works
- [ ] File drop on non-attachment area does NOT navigate the browser
- [ ] Column reorder drag (dnd-kit) still works
- [ ] DropZone does NOT show blue indicator for Finder file drags