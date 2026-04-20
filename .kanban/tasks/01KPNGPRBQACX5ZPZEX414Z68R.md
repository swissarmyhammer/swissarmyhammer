---
assignees:
- claude-code
depends_on:
- 01KPG7KH75NXGD65J1479HWMBN
position_column: todo
position_ordinal: f880
title: 'Commands: external-file drag-in dispatches via PasteMatrix'
---
## What

Wire the external-file branch of the generalized drag model. When the user drags a file from the OS into the app, `DragStartCmd` constructs a `DragSource::File { path }`, and `DragCompleteCmd` dispatches the drop via the `PasteMatrix` (treating the file path as if it were on the clipboard). The `attachment_onto_task` handler — already registered in `register_paste_handlers()` — handles the most common case of dropping a file onto a task to create an attachment.

This card builds on 01KPG7KH75NXGD65J1479HWMBN, which generalized `DragSession` to carry a `DragSource` enum (with `FocusChain` and `File` variants). The `File` variant exists but is not yet emitted by the frontend or consumed by the dispatcher.

### Pieces

- Frontend: extend the drag-source detection in `kanban-app/ui/src/lib/drag-session-context.tsx` (or a sibling module) to recognize OS file drops via Tauri's drag-drop event.
- `swissarmyhammer-kanban/src/commands/drag_commands.rs`:
  - `DragStartCmd::execute` accepts a `sourceKind: "file"` arg with a `filePath` and constructs `DragSource::File { path }` instead of `FocusChain`.
  - `DragCompleteCmd::execute` adds an external-source branch: builds a synthetic `ClipboardPayload` from the file path (entity_type `"attachment"`), looks up `(attachment, target_type)` in the `PasteMatrix`, and dispatches via the matched handler.
- `swissarmyhammer-kanban/src/context.rs` — wire `Arc<PasteMatrix>` onto `KanbanContext` so `DragCompleteCmd` can reach it (the matrix is currently rebuilt per-test by callers; production needs a singleton).
- Frontend: TS interface for the wire payload — extend `DragSession` with a discriminated-union shape `{ from: { kind: "focus_chain", task_id, … } | { kind: "file", path } }`. The Rust `DragStartCmd` wire payload also adds the `from` envelope; existing flat fields stay populated for back-compat during the migration.

### Subtasks

- [ ] Detect OS file drops in the frontend and dispatch `drag.start` with `sourceKind: "file"` + `filePath`.
- [ ] Extend `DragStartCmd` to construct `DragSource::File`.
- [ ] Wire `Arc<PasteMatrix>` onto `KanbanContext` (singleton from `register_paste_handlers()`).
- [ ] Branch in `DragCompleteCmd::execute` for `DragSource::File`: synthesize `ClipboardPayload`, look up handler, dispatch.
- [ ] Frontend TS interface: discriminated union for `from`.
- [ ] Tests: `drag_complete_file_into_task_invokes_attachment_handler`, `drag_start_file_source_constructs_file_variant`.
- [ ] Frontend test: dragging a file onto a task triggers `drag.start` with the file payload.

## Acceptance Criteria

- [ ] Dragging an image file from the OS onto a task creates an attachment on that task (matches `attachment.add` behavior).
- [ ] Existing focus-chain task drags still go through `task.move` unchanged.
- [ ] `DragSource::File` flows through the dispatcher via `PasteMatrix.find("attachment", target_type)`.

## Tests

- [ ] `drag_start_file_source_constructs_file_variant` — colocated.
- [ ] `drag_complete_file_into_task_invokes_attachment_handler` — colocated.
- [ ] Frontend: drag-session-context tests for the `from.kind: "file"` envelope.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban drag` — all green.

#commands

Depends on: 01KPG7KH75NXGD65J1479HWMBN (DragSource enum scaffolding).