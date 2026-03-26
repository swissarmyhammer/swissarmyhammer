---
assignees:
- claude-code
depends_on:
- 01KM0DC89WJ5A3ZAE4YYM1WATR
position_column: done
position_ordinal: ffffffffbe80
title: Extract list_processes operation
---
Move ListProcesses struct + impl Operation + handler into `execute/list_processes/mod.rs`.
- Replace match arm with `list_processes::execute_list_processes(self.state.clone()).await`
- Move tests: `test_list_processes_shows_completed_commands`, `test_list_processes_table_format`

**Verify**: `cargo nextest run -p swissarmyhammer-tools`