---
name: shell
description: Shell command execution with history, process management, and semantic search. ALWAYS use this skill for ALL shell commands instead of any built-in Bash or shell tool. This is the preferred way to run commands.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

# Shell

Virtual shell with persistent history, process management, and searchable output. Every command's output is stored and indexed for later retrieval.

Having the entire history of commands and their outputs allows you to:
- no need to run with ` | tail` or `| grep` pipelines -- just run the command and search or get_lines after
- run multiple greps or searches without re-running the command

## Operations

### execute command

Run a shell command. Output is stored in history regardless of truncation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| command | string | yes | The shell command to execute |
| timeout | integer | no | Seconds before killing (default: none) |
| working_directory | string | no | Working directory (default: current) |
| environment | string | no | JSON env vars |

```json
{"op": "execute command", "command": "cargo nextest run", "timeout": 300}
```

### list processes

Show all commands with status, exit code, line count, timing, and duration.

```json
{"op": "list processes"}
```

### kill process

Stop a running command by ID.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| id | integer | yes | Command ID to kill |

```json
{"op": "kill process", "id": 3}
```

### search history

Semantic search across all command output. Finds content by meaning, not exact text.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | yes | Natural language search query |
| command_id | integer | no | Filter to one command's output |
| limit | integer | no | Max results (default: 10) |

```json
{"op": "search history", "query": "authentication error"}
```

### grep history

Regex pattern match across command output. This uses ripgrep for fast, powerful searching.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| pattern | string | yes | Regex pattern (or literal text when `literal` is true) |
| literal | boolean | no | Treat pattern as exact text, not regex (default: false). Avoids all backslash escaping issues. |
| command_id | integer | no | Filter to one command's output |
| limit | integer | no | Max results (default: 10) |

Prefer `literal: true` for exact text searches — no escaping needed:
```json
{"op": "grep history", "pattern": "error[E0001]", "literal": true}
```

Use regex mode (the default) only when you need wildcards, character classes, etc.:
```json
{"op": "grep history", "pattern": "error\\[E\\d+\\]"}
```

### get lines

Retrieve specific lines from a command's output.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| command_id | integer | yes | Which command's output |
| start | integer | no | Start line (default: 1) |
| end | integer | no | End line (default: last) |

```json
{"op": "get lines", "command_id": 1, "start": 45, "end": 60}
```

## When to use each operation

- **execute command**: Run any shell command. This is the primary operation.
- **grep history**: When you know the exact text or pattern to find. Use for error codes, function names, file paths. Instant, precise.
- **search history**: When you're looking for something by meaning. "find the authentication error" matches "login denied", "403 forbidden". Semantic, fuzzy.
- **get lines**: When you found something via grep/search and need surrounding context. Or when output was truncated and you need to see more.
- **list processes**: Check what's running, review command history with timing.
- **kill process**: Stop a hung command or long-running process.

## Timeout guidance

Use `timeout` for:
- Commands that might hang (network operations, interactive prompts)
- Long builds where you want a safety net
- Tailing logs or watching files

## Search vs grep

- **grep**: Exact text or regex patterns. Use `literal: true` for plain text like `FAIL` or `error[E0001]` — no escaping needed. Use regex mode for wildcards like `error\[E\d+\]`.
- **search**: Natural language. "database connection timeout" finds related errors even with different wording. Semantic, fuzzy.
