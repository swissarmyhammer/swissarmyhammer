---
assignees:
- claude-code
depends_on:
- 01KPEMYJV7BMTJB6GZ8MGTD04J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe680
title: 'Commands: tracing for emit_cross_cutting_commands (mirror emit_entity_add instrumentation)'
---
## What

`emit_entity_add` in `swissarmyhammer-kanban/src/scope_commands.rs` logs at debug level at each decision point — view not found, entity_type missing, final push. That instrumentation was what let us diagnose the "New Tag missing" regression from 01KPCSY8R3413FB565CBA7PF9Z. The new `emit_cross_cutting_commands` pass needs the same treatment so future "command X missing on entity Y" bugs can be resolved from logs without code changes.

### Log points

Per the `use-tracing` memory principle (log via `tracing::`, never `eprintln!`), add `debug!` calls at these decision points inside `emit_cross_cutting_commands`:

- Entering the pass — log `scope_chain.len()` and total registry commands with `from: target` params.
- Per scope moniker: parsed `entity_type`, whether the moniker was skipped (field monikers), and how many commands matched.
- Per matched command: `cmd_id`, `target`, and the outcome of `check_available` (included / filtered).
- Dedup skips — when `(id, target)` is already in the seen set.

### Files to touch

- `swissarmyhammer-kanban/src/scope_commands.rs` — add instrumentation inside `emit_cross_cutting_commands`.
- `kanban-app/src/commands.rs` — if `log_scope_result` (from the WIP) doesn't already account for auto-emitted commands, extend it.

### Subtasks

- [ ] Add `debug!` calls inside `emit_cross_cutting_commands` at the log points above.
- [ ] Update `log_scope_result` (if needed) to break out auto-emitted commands as a separate line.
- [ ] Verify logs surface via `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` (per `reference_oslog` memory).

## Acceptance Criteria

- [ ] `emit_cross_cutting_commands` logs every decision point.
- [ ] A developer can diagnose "`entity.copy` not showing on tag" from `log show` output alone, without reading code.
- [ ] Log volume is reasonable — debug level, not info, so production logs aren't spammed.

## Tests

- [ ] No unit tests for log output (logs are side effects); the value is in the manual debug workflow. The acceptance criterion is verified by manually running `log show` after a known failure case.

## Workflow

- Reference the existing `emit_entity_add` instrumentation — same pattern, same level, same field names.

#commands

Depends on: 01KPEMYJV7BMTJB6GZ8MGTD04J (mechanism must exist)