---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9580
title: 'Shell tool: clamp negative max_lines to 0'
---
## What

The shell skill prompt (`builtin/skills/shell/SKILL.md`) documents `max_lines` as a parameter for `execute command`, but the `ShellExecuteRequest` struct in `swissarmyhammer-tools/src/mcp/tools/shell/infrastructure.rs` doesn't have a `max_lines` field — it's silently dropped by serde.

MCP clients following the skill prompt send `max_lines: -1` to mean "return all output". Since the field doesn't exist in the struct, this is silently ignored. The ask: add `max_lines` to `ShellExecuteRequest`, and if the value is negative, clamp it to 0 (status-only mode). This makes the tool tolerant of the documented `-1` convention without needing to implement negative-means-all semantics.

### Files to modify

- `swissarmyhammer-tools/src/mcp/tools/shell/infrastructure.rs` — add `max_lines: Option<i64>` field to `ShellExecuteRequest`
- `swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs` — after deserializing, clamp `request.max_lines` to `max(0, value)` and use it to truncate stdout in the response

## Acceptance Criteria

- [ ] `{"op": "execute command", "command": "echo hi", "max_lines": -1}` succeeds, returns output as if max_lines were 0 (status only)
- [ ] `{"op": "execute command", "command": "echo hi", "max_lines": 5}` returns up to 5 lines of output inline
- [ ] `{"op": "execute command", "command": "echo hi", "max_lines": 0}` returns status only (current behavior)
- [ ] `{"op": "execute command", "command": "echo hi"}` (no max_lines) still works as before

## Tests

- [ ] Unit test in `infrastructure.rs`: deserialize `ShellExecuteRequest` with `max_lines: -1` — assert field is present, clamped to 0
- [ ] Unit test: deserialize with `max_lines: 5` — assert field is 5
- [ ] Unit test: deserialize with no `max_lines` — assert None
- [ ] `cargo nextest run -p swissarmyhammer-tools` passes