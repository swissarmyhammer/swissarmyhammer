---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffb180
title: Prevent browser default file drop and support drag-to-attach
---
## What

Two problems to fix:

### 1. Browser default takes over on file drag
When a user drags a file from Finder into the app, the webview's default behavior opens/displays the file, taking over the screen. This must be prevented globally.

Add a global `dragover` and `drop` listener on `document` (or the app shell) that calls `e.preventDefault()` for file drags. This stops the browser from opening files regardless of where they're dropped.

### 2. Attachment editor should accept file drops
When a file is dragged onto the attachment editor area, it should work like clicking "Add file" — add the dropped file as an attachment.

In the attachment editor component:
- Add `onDragOver` (prevent default + set visual feedback)
- Add `onDragEnter` / `onDragLeave` (highlight the drop zone)
- Add `onDrop` — extract file paths from the drop event and fire `onChange`

Note: In Tauri v2 with `dragDropEnabled: false`, browser drag events still fire but `dataTransfer.files` contains `File` objects with names but no usable paths. To get actual filesystem paths from drops, we may need to enable Tauri's drag-drop event system (`dragDropEnabled: true`) and listen via `listen("tauri://drag-drop", ...)`. Research which approach works — the key requirement is getting the absolute file path from the drop.

### Approach
- Global prevention: add in `App.tsx` or the app shell component via `useEffect`
- Drop zone: add drag event handlers to the attachment editor's container div
- Visual feedback: show a highlighted border/background when dragging over the editor

### Files to modify
- `kanban-app/ui/src/App.tsx` or `kanban-app/ui/src/components/app-shell.tsx` — global drag prevention
- `kanban-app/ui/src/components/fields/editors/attachment-editor.tsx` — drop zone handling
- `kanban-app/tauri.conf.json` — may need `dragDropEnabled: true` if Tauri events are needed for file paths

### Files to create
- `kanban-app/ui/src/components/fields/editors/attachment-editor.test.tsx` — add drop zone tests (file already exists, extend it)

## Acceptance Criteria
- [ ] Dragging a file from Finder into the app does NOT open it in the webview
- [ ] Dragging a file onto the attachment editor adds it as an attachment
- [ ] Visual feedback (highlight) when dragging over the editor
- [ ] Dragging a file onto non-editor areas is a no-op (just prevented, not acted on)

## Tests (vitest + React Testing Library)
- [ ] Test: drop event on editor with file → onChange called with file path
- [ ] Test: dragover on editor → visual highlight class applied
- [ ] Test: dragleave on editor → highlight removed
- [ ] Run: `pnpm test` in `kanban-app/ui/` — all pass