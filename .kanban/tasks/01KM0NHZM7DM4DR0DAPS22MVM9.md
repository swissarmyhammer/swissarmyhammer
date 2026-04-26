---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffe380
title: 'nit: `EXECUTE_COMMAND_PARAMS` is `pub(crate)` while all other operation param statics are private'
---
swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs (line 27)\n\nAll other operation modules declare their params statics as plain `static` (module-private):\n- `list_processes/mod.rs`: `static LIST_PROCESSES_PARAMS`\n- `kill_process/mod.rs`: `static KILL_PROCESS_PARAMS`\n- `search_history/mod.rs`: `static SEARCH_HISTORY_PARAMS`\n- `grep_history/mod.rs`: `static GREP_HISTORY_PARAMS`\n- `get_lines/mod.rs`: `static GET_LINES_PARAMS`\n\nBut `execute_command/mod.rs` declares `pub(crate) static EXECUTE_COMMAND_PARAMS`. This is inconsistent with the pattern and exposes an internal detail unnecessarily.\n\nSuggestion: Remove the `pub(crate)` visibility to match all the other operations." #review-finding