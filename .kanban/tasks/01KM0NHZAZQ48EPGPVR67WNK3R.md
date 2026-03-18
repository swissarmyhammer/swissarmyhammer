---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'nit: `execute_execute_command` is a stutter — redundant double-verb in function name'
---
swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs (line 76)\nswissarmyhammer-tools/src/mcp/tools/shell/mod.rs (line 143)\n\nThe public entry-point for the operation is named `execute_execute_command`. The module is already called `execute_command`, so the `execute_` prefix is redundant. Compare with the other operations which have clean names:\n- `execute_list_processes`\n- `execute_kill_process`\n- `execute_search_history`\n- `execute_grep_history`\n- `execute_get_lines`\n\nAll the others follow the convention `execute_{operation_name}`. The first one should be `execute_command` matching its module name, or if the `execute_` prefix pattern must be preserved, `execute_run_command`.\n\nSuggestion: Rename to `run_command` or simply `execute` (since it lives in the `execute_command` module and the module name already provides context)." #review-finding