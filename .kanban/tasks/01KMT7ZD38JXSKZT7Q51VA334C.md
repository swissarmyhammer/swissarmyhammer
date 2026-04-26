---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8880
title: Add tests for TagTaskCmd::execute
---
task_commands.rs:273\n\n`async fn execute(&self, ctx: &CommandContext) -> Result<Value>`\n\nAvailability guards are tested, but no integration test dispatches `task.tag` through the command dispatch layer. The lower-level TagTask operation is tested in integration_tag_storage.rs, but the command-layer wiring is not.\n\nTest should dispatch task.tag via the command harness and verify the tag is applied. #coverage-gap #test