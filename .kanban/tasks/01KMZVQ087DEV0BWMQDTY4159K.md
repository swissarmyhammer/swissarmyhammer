---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: 'Shell tool: tolerant parsing of integer params from strings'
---
## What

MCP clients (including Claude Code) sometimes send integer parameters as strings (e.g. `"60"` instead of `60`). The `ShellExecuteRequest` struct in `swissarmyhammer-tools/src/mcp/tools/shell/infrastructure.rs` uses `#[derive(Deserialize)]` with strict `Option<u64>` types, causing `Invalid arguments: invalid type: string "60", expected u64`.

Fix by adding `#[serde(deserialize_with = ...)]` to coerce string values to their target integer types. Affected fields:

- `timeout: Option<u64>` in `ShellExecuteRequest` (infrastructure.rs:94)
- Any other integer fields across shell tool request structs (e.g. `command_id`, `start`, `end`, `limit`, `id` in `ShellGrepRequest`, `ShellGetLinesRequest`, `ShellSearchRequest`, `ShellKillRequest`)

Create a shared helper `deserialize_optional_u64_tolerant` (or use `serde_aux::field_attributes::deserialize_number_from_string`) that accepts both `42` and `"42"`.

## Acceptance Criteria

- [ ] `{"op": "execute command", "command": "echo hi", "timeout": "60"}` succeeds (string timeout)
- [ ] `{"op": "execute command", "command": "echo hi", "timeout": 60}` still works (integer timeout)
- [ ] `{"op": "get lines", "command_id": "1"}` succeeds (string command_id)
- [ ] All other integer params across shell operations accept both string and integer values
- [ ] Invalid strings like `"abc"` still return a clear error

## Tests

- [ ] Unit test in `swissarmyhammer-tools/src/mcp/tools/shell/infrastructure.rs`: deserialize `ShellExecuteRequest` from JSON with string timeout `"60"` — assert timeout == Some(60)
- [ ] Unit test: deserialize with integer timeout `60` — assert timeout == Some(60)
- [ ] Unit test: deserialize with invalid string `"abc"` — assert error
- [ ] Unit test: deserialize with null/missing timeout — assert timeout == None
- [ ] `cargo nextest run -p swissarmyhammer-tools` passes