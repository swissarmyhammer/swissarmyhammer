---
assignees:
- claude-code
position_column: todo
position_ordinal: e880
title: 'shell tool: a failed spawn leaves a permanent `running` record in `list processes`'
---
## What

`prepare_command` in
`crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs`
records the command before it starts the child process:

1. `state.start_command(...)` adds a `CommandRecord` with status
   `CommandStatus::Running`.
2. `spawn_shell_command(...)` runs next.

When the spawn fails, `prepare_command` returns `Err` and the caller `run`
returns that error straight to the agent. Nothing marks the record terminal.

Result: the record keeps status `Running` for the life of the server process.
`list processes` shows a command that never ends, and
`list_processes` renders a `Running` record as `Ns+`, so the reported duration
grows without bound. `kill process` cannot clear it either, because
`register_process` never ran, so `ShellState::processes` holds no PID for that
id and `kill_process` answers "no running process for command ID N".

## Why it matters

`list processes` is how an agent finds out what is still running. A phantom
record makes the agent think a command is still going, and there is no
operation that can clear it.

## Fix

Mark the record terminal before `prepare_command` returns the spawn error.
`mark_command_errored` already exists in the same file and sets `Completed`
with exit code -1, which is the state the shell uses for a run it could not
carry out.

Then correct the doc on `CommandStatus::Running` in `state.rs`, which this task
worded as "A command whose spawn failed also stays here, because nothing marks
that record." Once the leak is closed, that sentence must go.

## Tests

- New test: drive `execute command` with a command whose spawn fails, then call
  `list processes` and assert the record does NOT report `running`.
- Watch it fail before the change.

## Found by

`/double-check` while closing the round-2 review findings on ^mbran97. #shelltool #tools #bug