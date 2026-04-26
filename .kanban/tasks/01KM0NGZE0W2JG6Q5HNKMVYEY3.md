---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffdc80
title: 'warning: `execute_with_guard` and helpers gained pub(crate) visibility without justification'
---
swissarmyhammer-tools/src/mcp/tools/shell/process.rs (lines 513, 560, 576, 604, 625, 644, 693, 707)\n\nIn the original `execute/mod.rs`, `execute_with_guard`, `spawn_shell_command`, `prepare_working_directory`, `prepare_shell_command`, `spawn_command_process`, `send_completion_notification`, and `format_execution_result` were all bare `fn` or `async fn` (module-private). In the refactored code they are all `pub(crate)`. The only caller is `execute_command/mod.rs` within the same crate, so `pub(crate)` expands visibility across the entire crate unnecessarily.\n\nSuggestion: Keep these `pub(super)` or, if cross-module access within the shell directory is all that is needed, use `pub(super)` to limit exposure to sibling modules only. `pub(crate)` makes them callable from anywhere in the crate and widens the API surface for no reason." #review-finding