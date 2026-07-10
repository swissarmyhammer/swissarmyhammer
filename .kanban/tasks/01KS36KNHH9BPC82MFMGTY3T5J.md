---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffad80
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
- [x] New crate `swissarmyhammer-command-service` is a workspace member, builds clean
- [x] All six operation structs compile with `#[operation]` and `#[derive(Default, Deserialize, JsonSchema)]`
- [x] `generate_mcp_schema` from `swissarmyhammer-operations` produces a valid `inputSchema` covering the union of all verbs' parameters
- [x] The forthcoming `_meta`-tree generator (from plugin-arch) produces a `command` noun with all six verbs and their parameters when given `&[&dyn Operation]`
- [x] `CommandRegistration` round-trips through JSON without losing any field that today's YAML carries (keys, menu, context_menu, tab_button, scope, params, undoable, visible)

## Tests
- [x] `crates/swissarmyhammer-command-service/tests/operations_schema.rs` — snapshot test of the generated `inputSchema` JSON
- [x] `crates/swissarmyhammer-command-service/tests/meta_tree.rs` — snapshot test of the `io.swissarmyhammer/operations` `_meta` tree
- [x] `crates/swissarmyhammer-command-service/tests/payload_roundtrip.rs` — read each of the 12 existing YAML files (7 in `swissarmyhammer-kanban/builtin/commands/`: attachment, column, file, perspective, tag, task, view; 5 in `swissarmyhammer-commands/builtin/commands/`: app, drag, entity, settings, ui); convert each entry to a `CommandRegistration`; serialize to JSON; deserialize back; assert structural equality. Proves we don't lose fidelity. Total: 62 commands across 12 files. (Note: also covers the newer `ai.yaml` file added since the task was written — 13 files / 67 commands total.)
- [x] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write the schema snapshot tests and the payload roundtrip test first; they will fail until the structs and types are defined.

## Implementation Notes
- The crate uses `rmcp::schemars` (not the workspace's `schemars 0.8`) because rmcp pulls in `schemars 1.2`; mixing the two on a `JsonSchema` derive would cause ambiguous-trait errors. This matches the pattern in `swissarmyhammer-plugin/src/server/in_process.rs`.
- `RegisterCommand` IS the registration payload — the `CommandRegistration` type is a re-export alias of it. This avoids a duplicate struct definition while keeping the noun-shaped name for callers who refer to "the registration data" outside the operation-tool context.
- `RegisterCommand` has a manual `Default` impl (rather than `#[derive(Default)]`) because `CallbackMarker` has no meaningful default — the operation-struct slice uses `Box::<RegisterCommand>::default()` solely as a placeholder for metadata generation; the runtime never accepts a default-constructed payload.
- `CallbackMarker` uses hand-written `Serialize`/`Deserialize` impls to pin the `{ "$callback": "<id>" }` wire shape.

## Review Findings (2026-05-27 13:55)

### Nits
- [x] `crates/swissarmyhammer-command-service/Cargo.toml:13,20` — `async-trait` and `tracing` are declared as dependencies but neither is referenced anywhere in `src/`. The task scope is "just the types, doc comments, and serde wiring" — service logic lands in follow-up tasks. Either drop these deps now and re-add when the dispatcher / callback plumbing actually needs them, or leave a one-line comment in `Cargo.toml` noting they are placeholders for the upcoming service crate so future readers do not assume they are in use.
  - Resolved: dropped `async-trait` and `tracing` (and the also-unused `swissarmyhammer-operations-macros`, which is re-exported through `swissarmyhammer-operations`) from `crates/swissarmyhammer-command-service/Cargo.toml`. Follow-up service-logic tasks will re-add what they actually need.
- [x] `crates/swissarmyhammer-command-service/src/operations.rs:241-264` — `operations()` uses `OnceLock<Vec<...>>::get_or_init(...).as_slice()`, but the prevailing pattern in this workspace (see `crates/swissarmyhammer-kanban/src/schema.rs:27` `KANBAN_OPERATIONS: LazyLock<Vec<&'static dyn Operation>>` + `kanban_operations() -> &KANBAN_OPERATIONS`) is `LazyLock<Vec<...>>`. Switching to `LazyLock` removes the `get_or_init` closure boilerplate and matches the existing convention for the same "static slice of operation trait objects" use case.
  - Resolved: replaced the `OnceLock` + `get_or_init` body with a `static COMMAND_OPERATIONS: LazyLock<Vec<&'static dyn Operation>>` plus a thin `operations()` accessor that returns `&COMMAND_OPERATIONS`, matching the kanban `KANBAN_OPERATIONS` / `kanban_operations()` pattern.
- [x] `crates/swissarmyhammer-command-service/tests/operations_schema.rs:88-107` — `schema_properties_union_covers_known_params` checks `["id", "ctx", "force", "scope", "category", "id_prefix"]` but skips the `RegisterCommand`-only fields (`name`, `execute`, `menu`, `keys`, `tab_button`, `view_kinds`, `context_menu_group`, `context_menu_order`, `menu_name`). The `meta_tree.rs` test covers them under `register.parameters`, so the gap is not load-bearing — but expanding the union assertion to the full set would make a future drop of any register field fail in BOTH snapshot tests, not just one.
  - Resolved: expanded the assertion to cover the full union — required register fields (`name`, `execute`) plus every YAML-equivalent optional field (`menu_name`, `description`, `scope`, `keys`, `menu`, `context_menu`, `context_menu_group`, `context_menu_order`, `tab_button`, `view_kinds`, `undoable`, `visible`, `params`, `available`). Dropping any of them now fails both snapshots, not just the meta-tree one.
