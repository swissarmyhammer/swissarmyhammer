---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffde80
title: 'warning: `validate_shell_request` and `parse_environment_variables` changed from private to pub(crate) without callers outside the shell directory'
---
swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs (lines 233, 274)\n\nIn the original `execute/mod.rs` these were bare `fn` (private). The refactoring made them `pub(crate)`, but they are only called inside `execute_command/mod.rs` itself and no other module in the crate uses them. Similarly `format_success_result` and `format_error_result` are `pub(crate)` but have no callers outside the module.\n\nSuggestion: Either make them `pub(super)` (accessible to the parent `shell` module and siblings) or keep them entirely private. Unnecessary `pub(crate)` bleeds internal implementation details across the whole crate." #review-finding