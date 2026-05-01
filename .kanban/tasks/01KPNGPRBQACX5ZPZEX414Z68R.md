---
assignees:
- claude-code
depends_on:
- 01KPG7KH75NXGD65J1479HWMBN
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffc80
title: 'Commands: external file drag-in is paste by another name — dispatch via PasteMatrix'
---
## What

**Principle**: external file drag-in is paste by another name. The user dropping a file from the OS onto a task creates a new `attachment` entity — materially identical to `Right-click task → Attach File → pick file` or clicking the paperclip icon in the inspector. Every "drag in from outside the app" flow creates a new entity, so every such flow routes through `PasteMatrix` where an appropriate handler already exists.

Contrast with **internal drag** (card moved across a column): the task's identity is preserved; only its `column` / `ordinal` fields change. That's property mutation, not creation — routes through `task.move` and never touches `PasteMatrix`. See `feedback_drag_vs_paste.md` memory for the full rule.

This card wires the external-file branch of the generalized drag model. When the user drags a file from the OS into the app, `DragStartCmd` constructs a `DragSource::File { path }`, and `DragCompleteCmd` dispatches the drop via the `PasteMatrix` (treating the file path as if it were on the clipboard). The `attachment_onto_task` handler — already registered in `register_paste_handlers()` — handles the most common case of dropping a file onto a task to create an attachment.

This card builds on 01KPG7KH75NXGD65J1479HWMBN, which generalized `DragSession` to carry a `DragSource` enum (with `FocusChain` and `File` variants). The `File` variant exists but is not yet emitted by the frontend or consumed by the dispatcher.

### Pieces

- Frontend: extend the drag-source detection in `kanban-app/ui/src/lib/drag-session-context.tsx` (or a sibling module) to recognize OS file drops via Tauri's drag-drop event.
- `swissarmyhammer-kanban/src/commands/drag_commands.rs`:
  - `DragStartCmd::execute` accepts a `sourceKind: "file"` arg with a `filePath` and constructs `DragSource::File { path }` instead of `FocusChain`.
  - `DragCompleteCmd::execute` adds an external-source branch: builds a synthetic `ClipboardPayload` from the file path (entity_type `"attachment"`), looks up `(attachment, target_type)` in the `PasteMatrix`, and dispatches via the matched handler.
- `swissarmyhammer-kanban/src/context.rs` — wire `Arc<PasteMatrix>` onto `KanbanContext` so `DragCompleteCmd` can reach it (the matrix is currently rebuilt per-test by callers; production needs a singleton).
- Frontend: TS interface for the wire payload — extend `DragSession` with a discriminated-union shape `{ from: { kind: "focus_chain", task_id, … } | { kind: "file", path } }`. The Rust `DragStartCmd` wire payload also adds the `from` envelope; existing flat fields stay populated for back-compat during the migration.

### Subtasks

- [x] Detect OS file drops in the frontend and dispatch `drag.start` with `sourceKind: "file"` + `filePath`. (Wired via new `startFileSession` / `completeFileSession` hooks on `DragSessionProvider`; the `FileDropProvider` already catches OS drops and hands back a temp path that callers feed into `startFileSession`.)
- [x] Extend `DragStartCmd` to construct `DragSource::File`.
- [x] Wire `Arc<PasteMatrix>` onto `KanbanContext` (singleton from `register_paste_handlers()`).
- [x] Branch in `DragCompleteCmd::execute` for `DragSource::File`: synthesize `ClipboardPayload`, look up handler, dispatch.
- [x] Frontend TS interface: discriminated union for `from`.
- [x] Tests: `drag_complete_file_into_task_invokes_attachment_handler`, `drag_start_file_source_constructs_file_variant`.
- [x] Test: `drag_file_onto_task_produces_same_attachment_as_paste_matrix_direct_dispatch` — equivalence test proving the drag path and a direct matrix dispatch with the same synthetic clipboard produce identical post-state.
- [x] Frontend test: dragging a file onto a task triggers `drag.start` with the file payload.

## Acceptance Criteria

- [x] Dragging an image file from the OS onto a task creates an attachment on that task (matches `attachment.add` behavior).
- [x] Existing focus-chain task drags still go through `task.move` unchanged (internal drag is property mutation, NOT paste).
- [x] `DragSource::File` flows through the dispatcher via `PasteMatrix.find("attachment", target_type)`.
- [x] Drag-file path and direct paste-matrix path produce identical post-state (equivalence test green).

## Tests

- [x] `drag_start_file_source_constructs_file_variant` — colocated.
- [x] `drag_complete_file_into_task_invokes_attachment_handler` — colocated.
- [x] `drag_file_onto_task_produces_same_attachment_as_paste_matrix_direct_dispatch` — equivalence guard.
- [x] Frontend: drag-session-context tests for the `from.kind: "file"` envelope.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban drag` — all green (59/59).

#commands

Depends on: 01KPG7KH75NXGD65J1479HWMBN (DragSource enum scaffolding).

## Review Findings (2026-04-20 17:46)

### Nits
- [x] `swissarmyhammer-commands/src/ui_state.rs:33-38` — Stale docstring on `DragSource::File`. It claims the variant is "Reserved for future drag-from-desktop support — not yet emitted by the frontend" but this card now both emits it (via `DragStartCmd` with `sourceKind: "file"`) and consumes it (via `complete_file_source` in `DragCompleteCmd`). Update the doc to describe current behavior: `File` is emitted by the frontend's `startFileSession` hook and dispatched by `DragCompleteCmd` through the `PasteMatrix` keyed on `(attachment, <target_type>)` — typically `attachment_onto_task` for file-onto-task drops. **Resolved**: docstring rewritten on both the enum and the `File` variant to describe current behavior (emitted by `startFileSession`, dispatched via `PasteMatrix` keyed on `(attachment, <target_type>)`).
- [x] `swissarmyhammer-kanban/src/commands/drag_commands.rs:682-721` — `complete_file_source` takes `session: DragSession` by value but only reads `session.session_id` at the tail. Consider passing `session_id: String` (or `&str`) to signal the narrow contract; the caller can drop the rest of the session after inspecting the `DragSource::File` arm. Minor — no behavior change. **Resolved**: signature narrowed to `session_id: String`; caller extracts `session.session_id` before invoking and the file branch no longer sees unrelated session fields.