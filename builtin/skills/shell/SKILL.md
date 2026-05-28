---
name: shell
description: Shell command execution with persistent history, process management, and searchable output. Use when you need to run a shell command, search or grep previous command output, get output lines from a prior command, list running processes, or kill a hung process. Triggers on phrases like "run X", "execute X", "search the last build output", "grep the output", "kill that process", "show me the output of command N".
license: MIT OR Apache-2.0
compatibility: Requires the `shell` MCP tool for persistent command history, process management, and searchable output. A plain built-in Bash tool cannot replace it; this skill will not function as documented without the `shell` MCP tool.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Shell

Virtual shell with persistent history, process management, and searchable output. Every command's output is stored and indexed.

**Always use this skill for shell commands** — never the built-in Bash tool. The persistent history, process management, and semantic search are only available here.

This lets you:
- skip `| tail` / `| grep` pipelines — just run, then search/get_lines
- run multiple greps without re-executing

## Operations

### execute command

Run a command. Output is stored regardless of truncation.

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

### search history

Semantic search — finds by meaning, not exact text.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| query | string | yes | Natural language |
| command_id | integer | no | Scope to one command |
| limit | integer | no | Default: 10 |

```json
{"op": "search history", "query": "authentication error"}
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
- **search history** — find by meaning ("authentication error" finds "login denied", "403") — semantic, fuzzy
- **get lines** — surrounding context after grep/search, or to see truncated output
- **list processes** — running state, command history with timing
- **kill process** — stop hung or long-running commands

## Timeout

Set `timeout` for commands that might hang (network, prompts), long builds where you want a safety net, or tailing/watching.

## Search vs grep

- **grep**: exact text or regex. `literal: true` for plain text like `FAIL` — no escaping.
- **search**: natural language. Finds related errors with different wording.
