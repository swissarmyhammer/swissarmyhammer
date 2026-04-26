---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe980
title: 'Test: file drag from Finder to attachment field'
---
## What

Regression test suite for dragging files from Finder/Explorer onto an attachment field.

## Why

File attachment drop uses a completely different pipeline (Tauri native `onDragDropEvent`) from task card drag (HTML5 drag API). The fix to discriminate drag types in the global handlers must not break file drops. Existing tests in `file-drop-context.test.tsx` cover the Tauri event pipeline but not the interaction with task card drag.

## Key components under test

- `FileDropProvider` (file-drop-context.tsx) — global browser prevention + Tauri native listener
- `AttachmentEditor` (attachment-editor.tsx) — registers as drop target via `useFileDrop().registerDropTarget()`
- Global `dragover`/`drop` handlers — must call `preventDefault` for `Files` type, must NOT for task MIME type
- Callback stack (LIFO) — routes drops to the topmost registered target

## Tests to add

- [ ] Global `dragover` handler calls `preventDefault` when `dataTransfer.types` includes `Files`
- [ ] Global `dragover` handler does NOT call `preventDefault` when `dataTransfer.types` includes only `application/x-swissarmyhammer-task`
- [ ] Global `drop` handler calls `preventDefault` for `Files`, does NOT for task MIME
- [ ] Tauri `onDragDropEvent` with type `enter` sets `isDragging: true`
- [ ] Tauri `onDragDropEvent` with type `drop` delivers file paths to the topmost registered callback
- [ ] Tauri `onDragDropEvent` with type `leave` sets `isDragging: false`
- [ ] `AttachmentEditor` shows visual highlight ring when `isDragging` is true
- [ ] Dropping files on `AttachmentEditor` appends paths to attachment list
- [ ] File drop on a non-attachment area does NOT navigate the browser (global handler prevents it)
- [ ] LIFO callback stack: later-registered target receives the drop, not earlier one

## Files

- `kanban-app/ui/src/lib/file-drop-context.test.tsx` — extend with type discrimination tests
- `kanban-app/ui/src/components/fields/editors/attachment-editor.test.tsx` — new or extend