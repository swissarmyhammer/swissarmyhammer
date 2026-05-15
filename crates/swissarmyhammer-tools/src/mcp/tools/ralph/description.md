# ralph

Persistent agent loop instructions with per-session state.

Stores instructions as `.ralph/<session_id>.md`. Used by Stop hooks to prevent Claude from stopping while work remains. When a Stop hook fires and an instruction is active, ralph returns `"decision": "block"`. Call `clear ralph` when work is done to release the block.

## Operations

- `set ralph` — Store instruction. Required: `instruction`. Optional: `session_id`, `max_iterations` (default: 50), `body`
- `check ralph` — Check if blocked. Required: `session_id`. Returns `{"decision": "block"|"allow", ...}`
- `clear ralph` — Remove instruction. Optional: `session_id`
- `get ralph` — Read instruction. Optional: `session_id`

## Examples

```json
{"op": "set ralph", "instruction": "Implement all kanban cards until the board is clear", "max_iterations": 50}
{"op": "check ralph", "session_id": "abc123"}
{"op": "clear ralph"}
{"op": "get ralph"}
```
