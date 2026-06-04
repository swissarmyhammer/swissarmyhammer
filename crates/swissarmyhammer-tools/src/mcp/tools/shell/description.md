Virtual command shell with persistent history and process management used to run shell commands. Every command's output is stored for later retrieval and grep.

## Operations

### execute command

Run a shell command. Returns status only — use grep/get-lines to inspect output.

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

- **execute command**: Run any shell command. Returns status only — follow up with grep/get-lines to inspect output.
- **grep history**: When you know the exact text or pattern to find. Use for error codes, function names, file paths. Instant, precise.
- **get lines**: When you found something via grep and need surrounding context, or to read specific line ranges.
- **list processes**: Check what's running, review command history with timing.
- **kill process**: Stop a hung command or long-running process.

## Timeout guidance

Use `timeout` for:
- Commands that might hang (network operations, interactive prompts)
- Long builds where you want a safety net
- Tailing logs or watching files
