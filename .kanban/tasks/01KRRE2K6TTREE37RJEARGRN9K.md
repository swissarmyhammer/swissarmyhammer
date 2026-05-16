---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: plugin-arch
title: 'operations: generate the io.swissarmyhammer/operations _meta tree'
---
## What
Add a `_meta`-tree generator to `swissarmyhammer-operations` alongside the existing `generate_mcp_schema` (in `crates/swissarmyhammer-operations/src/schema.rs`).

- New public fn `generate_operations_meta(operations: &[&dyn Operation]) -> serde_json::Value` that builds the **noun → verb → { op, description, parameters }** tree the plugin SDK consumes.
- Each leaf: `op` = `Operation::op_string()`, `description` = `Operation::description()`, `parameters` = a map of param name → `{ type, required, description }` derived from `Operation::parameters()` (`ParamMeta`). Reuse the existing `param_type_to_json_schema_type` mapping; arrays get `items: {type: string}`.
- This value is the thing that goes under the `_meta` key `io.swissarmyhammer/operations` on a Tool definition. The generator returns only the value; attaching it to a Tool is the macro task's job.
- Export from `lib.rs`.

Do NOT change `generate_mcp_schema` or the wire format — `op` stays the single selector. This task only adds discovery metadata.

## Acceptance Criteria
- [ ] `generate_operations_meta` exists, is `pub`, exported from `swissarmyhammer-operations` lib.
- [ ] Output is a JSON object keyed by noun; each noun maps verbs to `{op, description, parameters}`; each parameter carries `type`, `required` (bool), and `description`.
- [ ] Two verbs on the same noun land under one noun key; two nouns produce two top-level keys.
- [ ] Empty parameter descriptions are omitted (matches `collect_all_parameters` behavior).

## Tests
- [ ] In `schema.rs` `#[cfg(test)]`, reuse the existing `MockAddTask`/`MockGetTask`/`MockListTasks` mocks: assert the tree has `task.add.op == "add task"`, `task.add.parameters.title.required == true`, `task.add.parameters.description.required == false`, and a separate `tasks` noun key for `MockListTasks`.
- [ ] Add a test asserting array params (`MockWithArrayParam`) emit `type: "array"` with `items.type: "string"`.
- [ ] Run: `cargo test -p swissarmyhammer-operations` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.