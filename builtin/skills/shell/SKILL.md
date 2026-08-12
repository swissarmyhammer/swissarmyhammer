---
name: shell
description: Shell command execution with persistent history, process management, and searchable output. Use when you need to run a shell command, grep previous command output, get output lines from a prior command, list running processes, or kill a hung process. Triggers on phrases like "run X", "execute X", "grep the output", "grep the last build output", "kill that process", "show me the output of command N".
license: MIT OR Apache-2.0
compatibility: Requires the `shell` MCP tool for persistent command history, process management, and searchable output. A plain built-in Bash tool cannot replace it; this skill will not function as documented without the `shell` MCP tool.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Shell

Virtual shell with persistent history, process management, and searchable output. Every command that exits stores its full output for later retrieval.

**Always use this skill for shell commands** — never the built-in Bash tool. The persistent history and process management are only available here.

## How output works

`execute command` blocks until the command exits or the timeout kills it. There
is no partial or streaming result. When the command exits, the response shows
the last lines of the output, and the full output stays in the history.

When the timeout kills the command, no output is stored. `get lines` and
`grep history` find nothing for that command. Raise `timeout` and run the
command again.

## Rules

- Do not pipe to `tail`, `head`, or `grep`. Read output with `get lines` or `grep history`.
- Do not use grep to search files. Use your file search tools. If you must, use `rg`.
- Do not use shell to edit files. Use your file editing tools.

## Operations

### execute command

Run a command. A command that exits stores its full output, even when the
response is truncated to the last lines.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| command | string | yes | Command to run |
| timeout | integer | no | Seconds before kill |
| working_directory | string | no | Default: current |
| environment | string | no | JSON env vars |

```json
{"op": "execute command", "command": "cargo nextest run", "timeout": 300}
```

### list processes

All commands with status, exit code, line count, timing, duration.

```json
{"op": "list processes"}
```

### kill process

```json
{"op": "kill process", "id": 3}
```

### grep history

Ripgrep regex (or literal) across output.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| pattern | string | yes | Regex (or literal if `literal: true`) |
| literal | boolean | no | Default: false. Skips escaping. |
| command_id | integer | no | Scope to one command |
| limit | integer | no | Default: 10 |

Prefer `literal: true` for exact text — no escaping:
```json
{"op": "grep history", "pattern": "error[E0001]", "literal": true}
```

Regex for wildcards or character classes:
```json
{"op": "grep history", "pattern": "error\\[E\\d+\\]"}
```

### get lines

```json
{"op": "get lines", "command_id": 1, "start": 45, "end": 60}
```

## When to use each

- **execute command** — primary operation
- **grep history** — exact text/patterns (error codes, function names, paths) — instant, precise
- **get lines** — surrounding context after grep, or to see truncated output
- **list processes** — running state, command history with timing
- **kill process** — stop hung or long-running commands

## Timeout

Set `timeout` for commands that might hang (network, prompts), long builds where you want a safety net, or tailing/watching.

A timeout is a loss, not a safe stop. The kill discards the output, so nothing is stored for that command. Set the timeout high enough for the command to finish.
