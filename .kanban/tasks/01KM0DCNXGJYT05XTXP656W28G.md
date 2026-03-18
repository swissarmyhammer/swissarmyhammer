---
assignees:
- claude-code
depends_on:
- 01KM0DC89WJ5A3ZAE4YYM1WATR
position_column: done
position_ordinal: ffffffff9380
title: Extract kill_process operation
---
Move KillProcess struct + impl Operation + handler into `execute/kill_process/mod.rs`.
- Handler receives `(args, state)`, parses `id` param, calls `state.kill_process()`
- Move tests: `test_kill_process_missing_id_returns_error`, `test_kill_process_nonexistent_id_returns_error`, `test_kill_process_stops_running_command`

**Verify**: `cargo nextest run -p swissarmyhammer-tools`