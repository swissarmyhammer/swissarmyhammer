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
position_column: todo
position_ordinal: f380
title: 'Commands: unify drag-drop with paste matrix (DragCompleteCmd uses PasteHandler)'
---
## What

Drop-onto-target is the same semantic operation as paste-onto-target: take a known source entity, put it somewhere based on the destination's type. Today drag-drop has its own dispatch path in `swissarmyhammer-kanban/src/commands/drag_commands.rs` (`DragCompleteCmd`). Refactor `DragCompleteCmd` to reuse the `PasteMatrix` — same handler lookup, same action, zero behavior drift between drag-drop and cut-paste.

### The design

The drag session carries `{ task_id, source_board_path, source_window_label, copy_mode }` — today it's task-specific. Generalize it the same way the clipboard generalized (card 01KPG5XK61ND4JKXW3FCM3CC97):

```rust
pub struct DragSession {
    entity_type: String,
    entity_id: String,
    fields: serde_json::Map<String, Value>,
    source_board_path: String,
    source_window_label: String,
    is_cut: bool,   // drag with modifier vs regular drag
}
```

`DragCompleteCmd::execute` reads the session, the drop-target moniker, and the `PasteMatrix` from the context. Looks up the handler, dispatches. Identical code path to `PasteEntityCmd` except the source payload comes from the drag session instead of the clipboard.

### Practical consequence

Any handler registered in the paste matrix is automatically a drop handler. Drag a task onto a column → `TaskIntoColumnHandler`. Drag a tag onto a task → `TagOntoTaskHandler`. No separate drag-handler registry, no duplicated matrix.

### Refactor opportunity — extract shared dispatcher

Both `PasteEntityCmd::execute` and `DragCompleteCmd::execute` become thin wrappers around:

```rust
pub async fn dispatch_via_matrix(
    source: &SourcePayload,  // unified: { entity_type, entity_id, fields, is_cut }
    scope_chain: &[String],
    target: &str,  // innermost drop target or chain head
    ctx: &CommandContext,
) -> Result<Value>
```

`ClipboardPayload` and `DragSession` both impl `Into<SourcePayload>` (or share the struct).

### Files to touch

- `swissarmyhammer-kanban/src/commands/drag_commands.rs` — `DragCompleteCmd` delegates to `dispatch_via_matrix`.
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — `PasteEntityCmd` delegates to the same.
- `swissarmyhammer-kanban/src/commands/paste_handlers/mod.rs` — add `dispatch_via_matrix` as the shared dispatcher. Possibly unify `ClipboardPayload` and `DragSession` into `SourcePayload`.
- `swissarmyhammer-commands/src/lib.rs` or wherever `UIState::drag_session` lives — extend from task-specific to generic entity source.
- Frontend drag event handler (`kanban-app/ui/src/lib/drag.ts` or similar) — confirm it populates the generalized session with entity type, id, and fields.

### Subtasks

- [ ] Unify `ClipboardPayload` and `DragSession` into `SourcePayload` (or share fields).
- [ ] Extract `dispatch_via_matrix` as the shared dispatcher.
- [ ] Rewrite `PasteEntityCmd::execute` to use it.
- [ ] Rewrite `DragCompleteCmd::execute` to use it.
- [ ] Update frontend drag handler to populate the generalized source.
- [ ] Audit existing `drag_*` tests; ensure behavior preserved.

## Acceptance Criteria

- [ ] Dragging a task onto a column lands the task in that column — identical behavior to pasting a task onto that column.
- [ ] Dragging a task onto a board lands it in the leftmost column (matches paste).
- [ ] Dragging a task onto a project sets the project field (matches paste).
- [ ] Dragging a tag onto a task tags the task (matches paste).
- [ ] Dragging a column onto a board duplicates the column (matches paste).
- [ ] No `DragCompleteCmd`-specific handler code remains — all dispatch goes through `PasteMatrix`.
- [ ] Zero user-visible behavior change vs pre-refactor drag.

## Tests

- [ ] `drag_complete_uses_paste_matrix` — mock a drag session, call `DragCompleteCmd`, assert the matched `PasteHandler` was invoked.
- [ ] `drag_equivalent_to_paste` — for each handler, run the same operation via drag and via paste; assert identical post-state.
- [ ] Existing `drag_start_cmd_stores_session`, `drag_cancel_cmd_clears_session` tests still pass.
- [ ] Frontend: `kanban-app/ui/src/lib/drag.test.ts` — drag event populates `SourcePayload` correctly.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban drag paste_handlers` — all green.

## Workflow

- Use `/tdd` — write `drag_equivalent_to_paste` for one handler pair first; it should fail because the dispatch paths differ; refactor until it passes.

#commands

Depends on: 01KPG5YB7GTQ6Q3CEQAMXPJ58F (paste mechanism) + all 7 paste handlers (matrix must be populated)