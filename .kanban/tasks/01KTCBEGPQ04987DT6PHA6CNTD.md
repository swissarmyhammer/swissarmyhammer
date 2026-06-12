---
assignees:
- claude-code
depends_on:
- 01KTCBDAH9GC2EYJHD80WGQ4RF
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff580
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
- [x] `generate_mcp_schema_full` exists and reproduces the exact current full schema (including `x-operation-schemas`/groups/forgiving/examples).
- [x] A slim wire generator exists matching card A's recorded shape and DROPS `x-operation-schemas`, `x-operation-groups`, `x-forgiving-input`, `examples`.
- [x] `generate_mcp_schema` still compiles (alias of `_full`) so no caller breaks in this card.

## Tests
- [x] In `crates/swissarmyhammer-operations/src/schema.rs` `#[cfg(test)]` (extend the existing mock-op tests at `schema.rs:260+`): assert the FULL schema contains `x-operation-schemas`, `x-operation-groups`, and (with aliases/examples) `x-forgiving-input`/`examples`.
- [x] Assert the WIRE schema OMITS all four of those keys and still contains `properties.op.enum` with every op string.
- [x] Size/token budget assertion on the wire form against the budget recorded in card A (serialize to string, assert byte length under the target).
- [x] `cargo nextest run -p swissarmyhammer-operations schema` passes.

## Workflow
- Use `/tdd` — write the full-vs-wire key-presence and size-budget assertions first (they fail until the split lands), then implement the two generators.

## Review Findings (2026-06-07 07:44)

### Nits
- [x] `crates/swissarmyhammer-operations/src/schema.rs:194` — The protocol discriminator field name "op" is hardcoded as a bare string literal in 4+ places (operation_to_schema's `vec!["op".to_string()]` and const-op insert, collect_all_parameters' op-property insert, and generate_mcp_schema_wire's op property + `"required": ["op"]`). It is the single most load-bearing key in the schema contract; if it ever changes, every scattered literal must be found and changed in lockstep, and a missed one silently breaks the wire/full schemas apart. Introduce one `const OP_FIELD: &str = "op";` (module-level) and reference it at each insertion/required-list site so the discriminator name is defined once. [NOTE: the operation_to_schema / collect_all_parameters literals are in functions this card was told NOT to change; the in-scope portion is the two new literals in generate_mcp_schema_wire. A module-level const cleanly covers both without altering the shared functions' behavior.]

## Review Findings (2026-06-07 07:55)

### Nits
- [ ] `crates/swissarmyhammer-operations/src/schema.rs:1` — The newly added test functions (full_schema_contains_all_custom_extensions, wire_schema_omits_dropped_keys, wire_schema_keeps_op_enum_and_top_level_shape, wire_schema_signatures_cover_every_op_with_ordered_required_names, wire_schema_is_dramatically_smaller_than_full, alias_returns_full_schema) drop the test_ prefix used by every pre-existing test in the same module (test_param_type_mapping, test_operation_to_schema, test_collect_all_parameters, test_generate_mcp_schema_minimal, etc.). This breaks the prevailing test-naming pattern within the file, making the suite inconsistent and harder to scan. Pick one convention for the module. To match the established pattern, rename the new tests with the `test_` prefix (e.g. `test_wire_schema_omits_dropped_keys`, `test_alias_returns_full_schema`). [non-blocking cosmetic]
- [ ] `crates/swissarmyhammer-operations/src/schema.rs:702` — `4` is an unexplained ratio defining how much smaller 'dramatically smaller' must be; the divisor encodes a behavioural threshold with no named meaning. Name the ratio, e.g. `const WIRE_SHRINK_FACTOR: usize = 4;` with a comment that wire must be at least 4x smaller than full, and use it in both the assertion and the message. [non-blocking cosmetic]