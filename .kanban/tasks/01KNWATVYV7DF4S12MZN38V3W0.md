---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd080
title: Refactor dispatch_command_internal result handlers into smaller functions
---
## What

`dispatch_command_internal` in `kanban-app/src/commands.rs` is ~490 lines with 4-5 levels of nesting in its result-handling section. The prefix rewrite loop was already extracted into `rewrite_dynamic_prefix`, but the bulk of the function is the post-dispatch result handler — a long chain of `if let Some(...)` blocks for each result variant (BoardSwitch, BoardClose, NewBoardDialog, CreateWindow, DragStart, DragComplete, etc.), each with deep nesting.

### Approach

Extract each result handler into a standalone `async fn`:
- `handle_board_switch(app, state, result) -> Result<Value, String>`
- `handle_board_close(app, state, result) -> Result<Value, String>`
- `handle_new_board_dialog(app, result) -> Result<Value, String>`
- `handle_create_window(app, result) -> Result<Value, String>`
- `handle_drag_start(app, state, result) -> Result<Value, String>`
- `handle_drag_complete(app, state, result) -> Result<Value, String>`

Then `dispatch_command_internal`'s result section becomes a flat match/if-let chain calling these helpers.

### Risk

This is the core dispatch pipeline. Must run full test suite before and after. The filter editor guard tests (`filter-editor.test.tsx`) MUST pass — any regression to perspective.filter dispatch is unacceptable.

### Files to modify

- `kanban-app/src/commands.rs` — extract result handlers from `dispatch_command_internal`

## Acceptance Criteria
- [x] `dispatch_command_internal` is under 100 lines (down from ~490) — now 50 lines
- [x] No nesting deeper than 3 levels in any extracted handler
- [x] All existing behavior preserved — no dispatch regressions
- [x] `cargo check -p kanban-app` clean, no warnings

## Tests
- [x] `cargo nextest run -p swissarmyhammer-kanban` — all pass (workspace: 13484 passing, same as baseline)
- [x] `cd kanban-app/ui && npx vitest run src/components/filter-editor.test.tsx` — all 28 guard tests pass
- [x] `cd kanban-app/ui && npx vitest run src/components/` — full component suite passes (156 files, 1309 tests)

## Workflow
- Use `/tdd` — run ALL tests before and after. This is high-risk refactoring.

## Notes

Most of the original ~490-line decomposition was performed in earlier passes — `dispatch_command_internal` was already down to ~50 lines with `apply_post_command_side_effects` orchestrating the side-effects (`handle_board_switch_result`, `handle_board_close_result`, `handle_ui_trigger_results`, `handle_drag_events`, `emit_ui_state_change_if_needed`, `maybe_rebuild_menu_after_cmd`, `flush_and_sync_after_command`).

This pass split the two remaining conflated handlers into the per-variant shape the task called for:
- `handle_ui_trigger_results` → now dispatches to `handle_new_board_dialog`, `handle_open_board_dialog`, `handle_create_window`, `handle_quit`.
- `handle_drag_events` → now dispatches to `handle_drag_start`, `handle_drag_cancel`, and the existing `handle_drag_complete`.

Each new handler has a docstring describing its variant and side-effects. No behavior changes.