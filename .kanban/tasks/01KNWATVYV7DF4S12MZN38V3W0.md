---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
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
- [ ] `dispatch_command_internal` is under 100 lines (down from ~490)
- [ ] No nesting deeper than 3 levels in any extracted handler
- [ ] All existing behavior preserved — no dispatch regressions
- [ ] `cargo check -p kanban-app` clean, no warnings

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — all pass
- [ ] `cd kanban-app/ui && npx vitest run src/components/filter-editor.test.tsx` — all 14 guard tests pass
- [ ] `cd kanban-app/ui && npx vitest run src/components/` — full component suite passes

## Workflow
- Use `/tdd` — run ALL tests before and after. This is high-risk refactoring.