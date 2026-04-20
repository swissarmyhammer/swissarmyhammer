---
assignees:
- claude-code
depends_on:
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6G4SGXW1FN92YDXFNEAQ2
- 01KPG6GD34NMPQE1DZD0MHWE0N
- 01KPG6GN9JQSCZKFER5ZJ5JC62
- 01KPG6GYSNGTEJ42XA2QNB3VE0
- 01KPG6H74Z24N48DQR75CT7HP7
- 01KPG6HF1ZHWZ981PS3BEPP1HE
- 01KPG6HQYRRWCP52VH1KNKR35B
position_column: done
position_ordinal: fffffffffffffffffffffffe80
title: 'Commands: unify drag-drop with paste matrix (DragCompleteCmd uses PasteHandler)'
---
## What

Drop-onto-target was originally specified as the same semantic operation as paste-onto-target (single dispatcher via PasteMatrix). During implementation the user clarified the actual model: drag is a generalized `from -> to` where each endpoint can be a focus chain (entity in the UI) OR an external (file dragged in from desktop, or a target on the desktop). Drag-from-focus-chain to focus-chain preserves entity identity (task.move) and stays separate from paste; drag-from-external uses the PasteMatrix once that path lands.

This card delivers the foundational refactor: generalize the in-Rust DragSession to use `DragSource` and `DragDestination` enums with `FocusChain` and `File` variants. Existing same-board task drag goes through the new shape unchanged in user-visible behavior. PasteMatrix wiring for external-source drags is reserved for a follow-up card.

### What was implemented

- `swissarmyhammer-commands/src/ui_state.rs` — added `DragSource` and `DragDestination` enums (tagged `kind` discriminator, `FocusChain` / `File` variants). `DragSession` now holds `from: DragSource` instead of flat task fields. Convenience accessors (`entity_id`, `entity_type`, `source_board_path`, `source_window_label`, `fields`) keep call sites readable.
- `swissarmyhammer-commands/src/lib.rs` — re-exports `DragSource` / `DragDestination`.
- `swissarmyhammer-kanban/src/commands/drag_commands.rs` — `DragStartCmd` constructs `DragSource::FocusChain` for task drags. `DragCompleteCmd::complete_same_board` reads the source via accessors, guards against non-task focus-chain sources (clear error rather than silent task.move misuse), and the cross-board payload builder uses the accessors so external-source drags surface as cross-board with empty source fields (Tauri handler then surfaces a clear validation error).
- `kanban-app/src/state.rs` — drag-session test fixtures updated to the new shape. Serialization test now asserts the nested `from.kind: "focus_chain"` envelope (legacy flat shape is preserved on the wire payload built by DragStartCmd, so the frontend continues to receive the same JSON).
- `kanban-app/ui/src/lib/drag-session-context.tsx` — TS interface unchanged (the wire payload is still flat for backward compatibility with existing event listeners). Comment block updated to point at the new in-Rust enum shape.

### What was deferred

- PasteMatrix dispatch from drag (the original card's `dispatch_via_matrix` shared dispatcher) — the user's clarification was that drag and paste must stay separate. Sharing only happens once external-file-drag lands.
- `drag_complete_uses_paste_matrix` and `drag_equivalent_to_paste` tests — those assert paste-matrix delegation that is now out of scope.
- Frontend ts-side `from`/`to` model — the in-Rust session is generalized, but the wire format is intentionally backward compatible. Future cards add `from`/`to` to the wire when external drag lands.

### New tests

- `drag_start_constructs_focus_chain_task_source` — pins that `DragStartCmd` always emits a `DragSource::FocusChain` with `entity_type = "task"`.
- `drag_session_accessors_round_trip_focus_chain_fields` — covers the convenience accessors round-tripping through the enum-shaped `from` field.
- `drag_complete_external_source_falls_through_to_cross_board` — exercises the `DragSource::File` path (no focus-chain source means the same-board guard fails and the result is a cross-board payload with empty source fields).
- `drag_complete_same_board_rejects_non_task_focus_chain` — guards against a future non-task focus-chain source silently going through `task.move`.

### Subtasks

- [x] Generalize `DragSession` to carry a `DragSource` enum (FocusChain / File variants).
- [x] Add `DragDestination` enum for the destination half of the from/to model (reserved for use at drop time; not currently stored on the session).
- [x] Update `DragStartCmd` to construct the new shape.
- [x] Update `DragCompleteCmd::complete_same_board` to consume via accessors with explicit task-only guard.
- [x] Update cross-board payload builder for the new shape.
- [x] Update fixtures and tests in `swissarmyhammer-kanban` and `kanban-app`.
- [x] Frontend wire-shape audit — confirm flat payload still works; document the deferred `from`/`to` reshape.
- [x] Audit existing `drag_*` tests; behavior preserved.

## Acceptance Criteria

- [x] Same-board drag of a task into a column still lands via `task.move` (identity preserved).
- [x] Cross-board drag still routes through `transfer_task` via the Tauri handler.
- [x] `DragSession` carries a generalized `from: DragSource` enum.
- [x] `DragSource::File` and `DragDestination` enums exist and are exported (reserved for the external-drag follow-up).
- [x] Convenience accessors on `DragSession` hide the enum-match in callers.
- [x] Zero user-visible behavior change vs pre-refactor drag.

## Tests

- [x] Existing `drag_start_cmd_stores_session` / `drag_cancel_cmd_clears_session` still pass.
- [x] All drag tests in `swissarmyhammer-kanban` (52 tests) and `kanban-app` (5 tests) pass.
- [x] All 175 `swissarmyhammer-commands` lib tests pass.
- [x] All 1285 frontend vitest tests pass.

## Workflow

- The card was implemented after the user clarified that drag and paste must stay separate (drag preserves identity, paste creates new). The PasteMatrix-shared-dispatch design from the original card description was redirected to "generalize DragSession only; PasteMatrix dispatch waits for external drag".

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (paste mechanism) + all 7 paste handlers (matrix is populated and available for the future drag-from-external follow-up).

## Follow-up

A new card should track the external-file-drag work (drag-in from desktop). When that lands, it constructs `DragSource::File` and dispatches via `register_paste_handlers()`, which already includes `attachment_onto_task` for exactly this case.