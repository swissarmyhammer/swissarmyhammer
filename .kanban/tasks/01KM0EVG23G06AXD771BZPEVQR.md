---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffbd80
title: Extract list_processes operation into its own module
---
Extract the list_processes operation from shell/mod.rs into shell/list_processes/mod.rs as specified by user. Move ListProcesses struct, LIST_PROCESSES_PARAMS, impl Operation, execute_list_processes handler, and tests.