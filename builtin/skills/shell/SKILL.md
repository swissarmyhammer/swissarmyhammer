---
name: shell
description: Run shell commands with a persistent history, process management, and searchable output. Use this skill when you need to run a shell command, search earlier command output, get output lines from a prior command, list running processes, or kill a stuck process. It triggers on phrases like "run X", "execute X", "grep the output", "grep the last build output", "kill that process", "show me the output of command N".
license: MIT OR Apache-2.0
compatibility: This skill needs the `shell` MCP tool, for persistent command history, process management, and searchable output. A plain built-in Bash tool cannot replace it. The skill does not work as documented without the `shell` MCP tool.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Shell

A virtual shell with persistent history, process management, and searchable output. The tool stores the output of every command for later retrieval.

**Always use this skill for shell commands.** Do not use the built-in Bash tool. Only this skill gives you the persistent history and process management.

This skill lets you do the following:
- Skip `| tail` or `| grep` pipelines. Run the command, then use grep or get lines.
- Run multiple searches without running the command again.

## Operations

### execute command

Run a command. The tool stores the output even when it truncates the displayed output.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| command | string | yes | Command to run |
| timeout | integer | no | Number of seconds before the tool stops the command |
| working_directory | string | no | Default: current |
| environment | string | no | JSON list of environment variables |

```json
{"op": "execute command", "command": "cargo nextest run", "timeout": 300}
```

### list processes

Lists every command with its status, exit code, line count, timing, and duration.

```json
{"op": "list processes"}
```

### kill process

```json
{"op": "kill process", "id": 3}
```

### grep history

Searches the output with a ripgrep regex, or with literal text.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| pattern | string | yes | Regex, or literal text if `literal: true` |
| literal | boolean | no | Default is false. When true, the tool does not escape the pattern. |
| command_id | integer | no | Scope to one command |
| limit | integer | no | Default: 10 |

Use `literal: true` for exact text. This skips escaping:
```json
{"op": "grep history", "pattern": "error[E0001]", "literal": true}
```

Use regex for wildcards or character classes:
```json
{"op": "grep history", "pattern": "error\\[E\\d+\\]"}
```

### get lines

```json
{"op": "get lines", "command_id": 1, "start": 45, "end": 60}
```

## When to use each

- **execute command** — the main operation
- **grep history** — for exact text or patterns, such as error codes, function names, or paths. It is instant and precise.
- **get lines** — to see the context around a match, or to see output that was truncated
- **list processes** — to see running state and command history with timing
- **kill process** — to stop a stuck or long-running command

## Timeout

Set `timeout` for a command that might hang, for example one that uses the network or waits for a prompt. Also set it for a long build, as a safety net, or for a command that tails or watches output.
