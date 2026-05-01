---
assignees:
- claude-code
position_column: todo
position_ordinal: 8c80
title: 'Remove or enable 4 #[ignore]''d undo_commands tests in swissarmyhammer-entity'
---
What: swissarmyhammer-entity/src/undo_commands.rs has 4 tests marked `#[ignore = "requires StoreContext undo stack not yet on this branch"]` at lines 126, 139, 152, 178. They panic when run (`StoreContext not available`). Per the test discipline, skipped tests must be fixed or deleted.

Tests:
- undo_commands::tests::undo_cmd_execute_noop_when_stack_empty
- undo_commands::tests::undo_cmd_execute_undoes_last_operation
- undo_commands::tests::redo_cmd_execute_noop_when_stack_empty
- undo_commands::tests::redo_cmd_execute_redoes_undone_operation

Error when run with `--run-ignored only`:
    thread 'undo_commands::tests::undo_cmd_execute_undoes_last_operation' panicked at swissarmyhammer-entity/src/undo_commands.rs:167:50:
    called `Result::unwrap()` on an `Err` value: ExecutionFailed("swissarmyhammer_store::context::StoreContext not available")

Acceptance Criteria:
- Either wire these tests up to a real StoreContext test harness and unignore them, or delete them outright (if the feature they cover is dead/moved).
- `cargo nextest run -p swissarmyhammer-entity --run-ignored all` reports zero failures and zero ignored tests.

Tests: existing tests must pass (or be removed); no new coverage required beyond making these green. #test-failure