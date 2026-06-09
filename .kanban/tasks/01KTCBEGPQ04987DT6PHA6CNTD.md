---
assignees:
- claude-code
depends_on:
- 01KTCBDAH9GC2EYJHD80WGQ4RF
position_column: todo
position_ordinal: '9480'
project: cli-schema-gen
title: Split full vs wire schema in swissarmyhammer-operations
---
## What
In `crates/swissarmyhammer-operations/src/schema.rs`, split today's single `generate_mcp_schema(operations, config)` (line 82) into two surfaces off the same operations list:

1. `generate_mcp_schema_full(operations, config) -> Value` — the CURRENT behavior, byte-for-byte: flat `properties`, `op` enum, `x-operation-schemas` (`operation_to_schema:190`), `x-operation-groups` (`group_operations_by_noun:246`), `x-forgiving-input`, `examples`, custom extensions. This is the CLI-facing schema (read in-process by the shared `cli_gen` generator from card B).
2. A slim WIRE variant — shape per card A's decision:
   - Always: `type/object`, `additionalProperties:true`, tool `description`, flat `properties` with the `op` enum (so the model knows valid op strings).
   - Option 1 (compact signatures): plus a small per-op required-field map under a single extension key (drop the heavy `x-operation-schemas` array).
   - Option 2 (bare): nothing beyond the op enum + description.
   - Either way DROP `x-operation-schemas`, `x-operation-groups`, `x-forgiving-input`, and `examples`.

Keep `generate_mcp_schema` as a thin alias of `generate_mcp_schema_full` for one transition step so the ~10 existing callers (per-tool wrappers like `generate_kanban_mcp_schema`, plus direct callers in `shell/mod.rs:506`, `git/changes/mod.rs:265`, `ralph/execute/mod.rs:217`) keep compiling. Card D then re-points the wire-facing `schema()` methods at the slim variant. Do NOT change `collect_all_parameters`, `operation_to_schema`, or the param-type mapping — both surfaces share them.

Files: `crates/swissarmyhammer-operations/src/schema.rs` (primary), `crates/swissarmyhammer-operations/src/lib.rs` (export the new fn).

Depends on card A because the slim wire shape is the decision recorded there.

## Acceptance Criteria
- [ ] `generate_mcp_schema_full` exists and reproduces the exact current full schema (including `x-operation-schemas`/groups/forgiving/examples).
- [ ] A slim wire generator exists matching card A's recorded shape and DROPS `x-operation-schemas`, `x-operation-groups`, `x-forgiving-input`, `examples`.
- [ ] `generate_mcp_schema` still compiles (alias of `_full`) so no caller breaks in this card.

## Tests
- [ ] In `crates/swissarmyhammer-operations/src/schema.rs` `#[cfg(test)]` (extend the existing mock-op tests at `schema.rs:260+`): assert the FULL schema contains `x-operation-schemas`, `x-operation-groups`, and (with aliases/examples) `x-forgiving-input`/`examples`.
- [ ] Assert the WIRE schema OMITS all four of those keys and still contains `properties.op.enum` with every op string.
- [ ] Size/token budget assertion on the wire form against the budget recorded in card A (serialize to string, assert byte length under the target).
- [ ] `cargo nextest run -p swissarmyhammer-operations schema` passes.

## Workflow
- Use `/tdd` — write the full-vs-wire key-presence and size-budget assertions first (they fail until the split lands), then implement the two generators.