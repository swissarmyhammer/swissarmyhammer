---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: command-service
title: Design operation structs + registration payload schema
---
## What

Define the six `#[operation]` structs that make up the `command` operation tool, plus the registration payload schema. These structs ARE the source of truth — the noun/verb/description/parameters that the `_meta` generator (from plugin-arch) will pick up.

Files (new crate `crates/swissarmyhammer-command-service/`):
- `crates/swissarmyhammer-command-service/Cargo.toml` — workspace member, depends on `swissarmyhammer-operations`, `swissarmyhammer-operations-macros`, `rmcp`, `serde`, `schemars`, `async-trait`, `tracing`
- `crates/swissarmyhammer-command-service/src/lib.rs` — module exports
- `crates/swissarmyhammer-command-service/src/operations.rs` — the six operation structs:
  - `RegisterCommand` (verb=register, noun=command) — full payload below
  - `UnregisterCommand` (verb=unregister, noun=command) — `id`
  - `ExecuteCommand` (verb=execute, noun=command) — `id`, `ctx`, `force?`
  - `AvailableCommand` (verb=available, noun=command) — `id`, `ctx`
  - `ListCommand` (verb=list, noun=command) — optional `scope`, `category`, `id_prefix` filters
  - `SchemaCommand` (verb=schema, noun=command) — `id`
- `crates/swissarmyhammer-command-service/src/types.rs` — `CommandRegistration`, `CommandContext`, `ParamDef`, `CommandMetadata`, `CommandError`

`RegisterCommand` payload mirrors every field today's YAML supports, so built-in plugins can register without losing fidelity:

```rust
struct RegisterCommand {
    pub id: String,                              // "task.move"
    pub name: String,                            // "Move Task"
    pub description: Option<String>,
    pub category: Option<String>,                // "Cleanup", etc.
    pub scope: Option<Vec<String>>,              // ["entity:task"]
    pub keys: Option<HashMap<String, String>>,   // { vim: "x", cua: "Delete" }
    pub menu: Option<Value>,                     // free-form placement payload
    pub context_menu: Option<bool>,
    pub tab_button: Option<Value>,
    pub undoable: Option<bool>,
    pub visible: Option<bool>,
    pub params: Option<Vec<ParamDef>>,
    pub available: Option<CallbackMarker>,       // { $callback: "cb_..." }
    pub execute: CallbackMarker,                 // required
}
```

`ParamDef` mirrors today's params (name, from: scope_chain|target|args|default, entity_type, default).

`CommandError` enum: `UnknownCommand`, `CommandUnavailable { reason }`, `CallbackFailed`, `LatencyBudgetExceeded`.

No service logic in this task — just the types, doc comments, and serde wiring. The `#[operation]` attributes and field doc comments drive the `_meta` tree; correctness here means the SDK path sugar gets the right shape.

## Acceptance Criteria
- [ ] New crate `swissarmyhammer-command-service` is a workspace member, builds clean
- [ ] All six operation structs compile with `#[operation]` and `#[derive(Default, Deserialize, JsonSchema)]`
- [ ] `generate_mcp_schema` from `swissarmyhammer-operations` produces a valid `inputSchema` covering the union of all verbs' parameters
- [ ] The forthcoming `_meta`-tree generator (from plugin-arch) produces a `command` noun with all six verbs and their parameters when given `&[&dyn Operation]`
- [ ] `CommandRegistration` round-trips through JSON without losing any field that today's YAML carries (keys, menu, context_menu, tab_button, scope, params, undoable, visible)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/operations_schema.rs` — snapshot test of the generated `inputSchema` JSON
- [ ] `crates/swissarmyhammer-command-service/tests/meta_tree.rs` — snapshot test of the `io.swissarmyhammer/operations` `_meta` tree
- [ ] `crates/swissarmyhammer-command-service/tests/payload_roundtrip.rs` — read each of the 12 existing YAML files (7 in `swissarmyhammer-kanban/builtin/commands/`: attachment, column, file, perspective, tag, task, view; 5 in `swissarmyhammer-commands/builtin/commands/`: app, drag, entity, settings, ui); convert each entry to a `CommandRegistration`; serialize to JSON; deserialize back; assert structural equality. Proves we don't lose fidelity. Total: 62 commands across 12 files.
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write the schema snapshot tests and the payload roundtrip test first; they will fail until the structs and types are defined.