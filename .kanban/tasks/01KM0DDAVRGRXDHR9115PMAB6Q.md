---
assignees:
- claude-code
depends_on:
- 01KM0DC89WJ5A3ZAE4YYM1WATR
position_column: done
position_ordinal: ffffffffd780
title: Extract execute_command operation
---
Move ExecuteCommand struct + impl Operation + handler into `execute/execute_command/mod.rs`.
- This is the largest — includes the entire command execution flow + security tests
- Handler receives `(args, state, context)` since it needs ToolContext for notifications
- Move tests: basic execution, exit status, security, output handling (~40 tests)

**Verify**: `cargo nextest run -p swissarmyhammer-tools`