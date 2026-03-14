# ralph

Persistent agent loop instructions with per-session state.

Ralph stores instructions as markdown files in `.ralph/<session_id>.md`. Used by Stop hooks to prevent Claude from stopping while work remains.

## Overview

The ralph tool maintains per-session instructions that survive across tool calls. When a Stop hook fires, it calls `check ralph` — if an instruction is active, ralph returns `"decision": "block"` with the reason, preventing the agent from stopping. When the work is done, call `clear ralph` to release the block.

## Operations

The tool accepts `op` as a "verb noun" string.

### Ralph Operations

- `set ralph` - Store a persistent instruction for a session
  - Required: `instruction`
  - Optional: `session_id` (defaults to current MCP session), `max_iterations` (default: 50), `body` (notes)

- `check ralph` - Check if a session has an active instruction
  - Required: `session_id`
  - Returns: `{"decision": "block", "reason": "...", ...}` or `{"decision": "allow"}`

- `clear ralph` - Remove a session's instruction
  - Optional: `session_id` (defaults to current MCP session)

- `get ralph` - Read a session's instruction and metadata
  - Optional: `session_id` (defaults to current MCP session)

## Examples

### Set an instruction (beginning of work)

```json
{"op": "set ralph", "session_id": "abc123", "instruction": "Keep implementing kanban cards until the board is clear", "max_iterations": 50}
```

### Check for Stop hook

```json
{"op": "check ralph", "session_id": "abc123"}
```

Response when blocked:
```json
{"decision": "block", "reason": "Keep implementing kanban cards until the board is clear", "iteration": 3, "max_iterations": 50}
```

Response when allowed (no active instruction):
```json
{"decision": "allow"}
```

### Clear when work is complete

```json
{"op": "clear ralph", "session_id": "abc123"}
```

### Read current instruction

```json
{"op": "get ralph", "session_id": "abc123"}
```
