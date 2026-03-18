---
assignees:
- claude-code
depends_on:
- 01KM0DC89WJ5A3ZAE4YYM1WATR
position_column: done
position_ordinal: ffffffffc280
title: Extract get_lines operation
---
Move GetLines struct + impl Operation + handler into `execute/get_lines/mod.rs`.
- Handler receives `(args, state)`, calls `state.get_lines()`
- Move tests (5): `test_get_lines_missing_command_id_returns_error`, `test_get_lines_retrieves_output`, `test_get_lines_with_range`, `test_get_lines_nonexistent_command`, `test_get_lines_shows_line_numbers`

**Verify**: `cargo nextest run -p swissarmyhammer-tools`