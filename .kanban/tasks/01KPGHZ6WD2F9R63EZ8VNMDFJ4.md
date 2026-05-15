---
assignees:
- claude-code
depends_on:
- 01KPEMYJV7BMTJB6GZ8MGTD04J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe280
title: 'Commands: test — cross_cutting pass respects entity_type constraint on target param'
---
## What

Gap in the cross-cutting test suite: no test confirms that when a registry command declares an `entity_type` constraint on its target param (e.g. `params: [{name: moniker, from: target, entity_type: task}]`), the cross-cutting pass emits it only for matching moniker types.

### The test

Add in `swissarmyhammer-kanban/src/scope_commands.rs` tests module:

- `cross_cutting_respects_target_entity_type_constraint` — register a stub command with `params: [{name: moniker, from: target, entity_type: task}]`. Scope chain contains both `task:01X` and `tag:01T`. Assert the stub emits with `target: Some("task:01X")` and does NOT emit with `target: Some("tag:01T")`.

### Files to touch

- `swissarmyhammer-kanban/src/scope_commands.rs` tests module — one test function.

## Acceptance Criteria

- [ ] `cross_cutting_respects_target_entity_type_constraint` exists and passes.
- [ ] Regressing the entity_type filter would fail the test.

## Tests

- [ ] `cross_cutting_respects_target_entity_type_constraint`.
- [ ] Run: `cargo nextest run -p swissarmyhammer-kanban scope_commands::tests::cross_cutting_respects_target_entity_type_constraint` — passes.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPEMYJV7BMTJB6GZ8MGTD04J