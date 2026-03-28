---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffb780
title: Add tests for DragCompleteCmd::execute (same-board path)
---
drag_commands.rs:133-342\n\n`async fn execute(&self, ctx: &CommandContext) -> Result<Value>`\n\nThe entire same-board drag completion path is untested at the command layer. This includes:\n- Starting a drag session, then completing it\n- Ordinal resolution via before_id/after_id/drop_index within drag context\n- Verifying the task actually moves to the target column\n- Error cases (no active session, invalid target column)\n\nThe underlying MoveTaskCmd ordinal logic is tested separately, but DragCompleteCmd has its own orchestration that duplicates some of that logic. #coverage-gap