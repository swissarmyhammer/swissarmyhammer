---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd580
title: 'Cross-window card drag broken: cards cannot be dragged between windows'
---
## What

Dragging a task card from one window to another does not work. The card can be picked up but dropping on the target window has no effect.

### Investigation needed

Cross-window drag uses the `DragSession` system (`drag-session-context.tsx`). The flow is:\n1. Source window: `handleTaskDragStart` → `invoke(\"dispatch_command\", { cmd: \"drag.start\" })` → emits `drag-session-active` Tauri event\n2. Target window: receives `drag-session-active`, shows drop zones for incoming task\n3. Target window: `handleZoneDrop` detects different board path → calls `completeSession` → `invoke(\"dispatch_command\", { cmd: \"drag.complete\" })`\n4. Backend emits `drag-session-completed` → both windows clear session state\n\nPossible failure points:\n- Secondary window doesn't have `DragSessionProvider` set up correctly\n- `drag-session-active` event doesn't reach secondary window\n- HTML5 `dataTransfer` data doesn't survive cross-window drag (OS limitation?)\n- The `disable_drag_drop_handler()` on secondary windows may interfere with HTML5 drag (not just file drops)\n- `boardPath` comparison in `handleZoneDrop` might not detect cross-board correctly\n\n### Files to investigate\n\n- `kanban-app/ui/src/lib/drag-session-context.tsx` — session lifecycle\n- `kanban-app/ui/src/components/board-view.tsx:394-420` — drag start/end/zone-drop handlers\n- `kanban-app/src/commands.rs` — `disable_drag_drop_handler()` effect on HTML5 drag\n- Rust drag commands: `drag.start`, `drag.cancel`, `drag.complete`\n\n## Acceptance Criteria\n\n- [ ] Dragging a card from window A to window B moves the card\n- [ ] Source window removes the card after successful drop\n- [ ] Target window shows the card in the dropped column\n- [ ] Drag cancel (dropping outside any window) properly cancels the session\n\n## Tests\n\n- [ ] `drag-session-context.test.tsx`: verify session lifecycle events propagate\n- [ ] Integration test: simulate cross-window drag via Tauri events\n- [ ] Run: `cd kanban-app/ui && npx vitest run` — all tests pass