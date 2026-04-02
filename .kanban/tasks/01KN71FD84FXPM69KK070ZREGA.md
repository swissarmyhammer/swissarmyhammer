---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: Fix card drag-and-drop broken by file drop introduction
---
## What

Card drag-and-drop on the board stopped working when the file-drop system was introduced for attachments. The two drag systems coexist but interact in ways that break card reordering/movement.

### Architecture of the two systems

1. **Card drag** — HTML5 native drag with custom MIME `application/x-swissarmyhammer-task`. Source: `sortable-task-card.tsx:39`. Targets: `drop-zone.tsx` between/around cards. Column container (`column-view.tsx:395-412`) calls `e.preventDefault()` on all dragover to allow drops and drive auto-scroll.

2. **File drop** — Tauri native `onDragDropEvent` in `file-drop-context.tsx`. Document-level `dragover`/`drop` listeners prevent default only for events with `"Files"` in `dataTransfer.types` (lines 82-97). `AttachmentDisplay`/`AttachmentListDisplay` register callbacks via `useFileDrop()`.

### Likely conflict points

- **Document-level event handlers vs React handlers**: `FileDropProvider` adds native `document.addEventListener("dragover", ...)` and `document.addEventListener("drop", ...)`. These fire in the same bubble phase as React's delegated handlers (React 18 attaches at root, not document). The `isFileDrag` guard (line 83-84) should protect card drags, but if browsers include `"Files"` in `dataTransfer.types` for any in-page drag (some do for certain drag configurations), the guard would incorrectly trigger and `preventDefault()` at document level before drop zones can process the event.
- **Column container unconditionally prevents default**: `column-view.tsx:399` calls `e.preventDefault()` on ALL dragover events (including file drags from Finder). This "accepts" file drops at the column level, potentially competing with Tauri's native handler.
- **AttachmentDisplay always registered**: When an attachment field is visible in the inspector, it registers a global drop target. If Tauri's `onDragDropEvent` fires during a cross-window card drag (OS mediates the drag), the file-drop system could intercept it.

### Files to investigate and fix

- `kanban-app/ui/src/lib/file-drop-context.tsx` — Document-level prevention may need to check for the task MIME explicitly, or use capture phase
- `kanban-app/ui/src/components/drop-zone.tsx` — May need to reject file drags explicitly
- `kanban-app/ui/src/components/column-view.tsx:395-412` — `handleContainerDragOver` should only preventDefault for card drags, not all drags

## Acceptance Criteria

- [ ] Cards can be dragged and dropped between columns on the board
- [ ] Cards can be reordered within a column via drag-and-drop
- [ ] Drop zone highlight (blue bar) appears when dragging a card over a valid target
- [ ] File drops from Finder still work on attachment fields in the inspector
- [ ] Card drag and file drop do not interfere when an attachment field is visible

## Tests

- [ ] Add test in `kanban-app/ui/src/lib/file-drop-context.test.tsx` — verify document handlers don't preventDefault when `dataTransfer.types` contains both `"Files"` and the task MIME (edge case in some browsers)
- [ ] Add test in `kanban-app/ui/src/components/drop-zone.test.tsx` — verify DropZone `onDrop` fires correctly when task MIME data is present and FileDropProvider is active
- [ ] Add integration test: render a DropZone inside a FileDropProvider, simulate a card drag, verify drop handler is called
- [ ] Run: `cd kanban-app/ui && npx vitest run` — all tests pass