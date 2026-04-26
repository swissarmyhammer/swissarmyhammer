---
assignees:
- claude-code
depends_on:
- 01KPEMYJV7BMTJB6GZ8MGTD04J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe380
title: 'Commands: test — cross_cutting pass ignores commands whose primary param is from:args (not from:target)'
---
## What

Gap in the cross-cutting test suite: no test confirms the emitter *rejects* commands whose primary param is `from: args` or `from: scope_chain`. The current `emit_cross_cutting_commands` (`swissarmyhammer-kanban/src/scope_commands.rs`) filters on `ParamSource::Target` on the first param, but no test pins that contract. If a future refactor loosened the check to "any param is from: target" or "scope_chain counts too," the emission would silently start surfacing wrong commands.

### The test

Add a test in `swissarmyhammer-kanban/src/scope_commands.rs` tests module:

- `cross_cutting_ignores_from_args_commands` — register a stub command with `params: [{name: moniker, from: args}]` and `context_menu: true`. Call `commands_for_scope` with a task moniker in scope. Assert the stub does NOT appear with a task target in the output. Repeat for `from: scope_chain`.

This is a targeted correctness guard — tiny card, single test.

### Files to touch

- `swissarmyhammer-kanban/src/scope_commands.rs` tests module — add one new test function.

## Acceptance Criteria

- [x] `cross_cutting_ignores_from_args_commands` exists and passes.
- [x] The test would FAIL if `emit_cross_cutting_commands` were changed to accept `from: args` as the primary-param signal (hypothetical regression). (Verified by temporarily flipping the rule and observing FAIL: `stub.from_args (primary param `from: args`) must NOT be emitted by the cross-cutting pass with a task target ... got: [("stub.from_args", Some("task:01X"))]`.)

## Tests

- [x] `cross_cutting_ignores_from_args_commands` — inline stub + assertion.
- [x] Run: `cargo nextest run -p swissarmyhammer-kanban scope_commands::tests::cross_cutting_ignores_from_args_commands` — passes.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPEMYJV7BMTJB6GZ8MGTD04J (the pass being tested)