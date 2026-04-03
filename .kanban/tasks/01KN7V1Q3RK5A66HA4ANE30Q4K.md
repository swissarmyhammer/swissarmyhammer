---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd480
title: File drag-drop silently fails on secondary windows (disable_drag_drop_handler)
---
## What

Dropping files from Finder onto a secondary window does nothing. The `FileDropProvider` global handlers prevent browser navigation (good), but `onDragDropEvent()` never fires so the file paths never reach the `AttachmentEditor` callback.

### Root cause

`kanban-app/src/commands.rs:689` calls `.disable_drag_drop_handler()` on dynamically created secondary windows. This removes Tauri's native OS-level drag-drop handler, so `getCurrentWebview().onDragDropEvent()` never emits events on those windows.

### Investigation needed

- Why was `disable_drag_drop_handler()` added? Check git blame on that line for the commit message.
- Was it working around a Tauri bug? A conflict with HTML5 drag? Something else?
- Can we simply remove it now that the FileDropProvider correctly discriminates MIME types?

### Files to investigate

- `kanban-app/src/commands.rs:689` — the `disable_drag_drop_handler()` call
- `kanban-app/ui/src/lib/file-drop-context.tsx` — `onDragDropEvent()` setup

## Acceptance Criteria

- [ ] Dropping a file from Finder onto a secondary window's attachment field works
- [ ] File paths are received by the `AttachmentEditor` callback
- [ ] Browser does not navigate to file:// URL on secondary windows
- [ ] Task card drag still works on secondary windows

## Tests

- [ ] Integration test: create a secondary window context, verify `onDragDropEvent` fires
- [ ] Run: `cd kanban-app/ui && npx vitest run` — all tests pass