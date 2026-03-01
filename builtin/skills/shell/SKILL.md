---
name: shell
description: Shell command execution with history, process management, and semantic search. Use this skill for ALL shell commands instead of the built-in Bash tool.
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Shell

Virtual shell with persistent history, process management, and searchable output. Every command's output is stored and indexed for later retrieval.

## Operations

### execute command

Run a shell command. Output is stored in history regardless of truncation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| command | string | yes | The shell command to execute |
| timeout | integer | no | Seconds before killing (default: none) |
| max_lines | integer | no | Max output lines returned (default: 200, -1 for all, 0 for status-only) |
| working_directory | string | no | Working directory (default: current) |
| environment | string | no | JSON env vars |

```json
{"op": "execute command", "command": "cargo test", "timeout": 300, "max_lines": 50}
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

Regex pattern match across command output. Exact structural search.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| pattern | string | yes | Regex pattern |
| command_id | integer | no | Filter to one command's output |
| limit | integer | no | Max results (default: 50) |

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

## max_lines guidance

- **Default (200)**: Fine for most commands. See the beginning of output, search/get-lines for the rest.
- **0**: Fire-and-forget. Run the command, get status only. Good for background tasks.
- **-1**: Return everything. Use when you need to process the full output.

## Search vs grep

- **grep**: Regex patterns. `error\[E\d+\]` finds Rust error codes. `FAIL` finds test failures. Structural, exact.
- **search**: Natural language. "database connection timeout" finds related errors even with different wording. Semantic, fuzzy.
