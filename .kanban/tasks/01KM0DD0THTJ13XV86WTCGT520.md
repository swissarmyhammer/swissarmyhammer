---
assignees:
- claude-code
depends_on:
- 01KM0DC89WJ5A3ZAE4YYM1WATR
position_column: done
position_ordinal: ffffffffc180
title: Extract grep_history operation
---
Move GrepHistory struct + impl Operation + handler into `execute/grep_history/mod.rs`.
- Handler receives `(args, state)`, calls `state.grep()`
- Move tests (7): `test_grep_history_missing_pattern_returns_error`, `test_grep_history_finds_matching_output`, `test_grep_history_no_matches`, `test_grep_history_with_command_id_filter`, `test_grep_history_with_limit`, `test_grep_history_regex_pattern`, `test_grep_history_invalid_regex_returns_error`

**Verify**: `cargo nextest run -p swissarmyhammer-tools`