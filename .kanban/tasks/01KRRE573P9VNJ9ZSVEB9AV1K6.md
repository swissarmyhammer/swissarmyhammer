---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff380
project: plugin-arch
title: 'plugin: Dispatcher and CallerId propagation'
---
## What
Implement the single dispatcher all calls flow through (host-to-host, plugin-to-host, plugin-to-plugin, agent-to-host).

In `crates/swissarmyhammer-plugin/src/dispatcher.rs`:
- `pub struct Dispatcher { registry: Arc<ServerRegistry> }`.
- `pub async fn call(&self, caller: CallerId, server: &str, tool: &str, input: Value) -> Result<Value>` — looks up `server` in the registry (`Err(UnknownServer)` if absent), then forwards `caller`, `tool`, `input` to `McpServer::invoke`.
- The dispatcher routes by `(server, tool)` and forwards ONE arguments map. No verb/noun axis in the signature — `op` is already a key inside `input` when present; the platform never reads `input`.
- `CallerId` (placeholder from the registry task) is finalized here: it distinguishes host-internal, plugin-id, and external-agent callers, is `Clone`, and is threaded through to `invoke`. The platform does not gate calls on it.

## Acceptance Criteria
- [x] `Dispatcher` exists, holds `Arc<ServerRegistry>`, and `call` forwards to the resolved server's `invoke`.
- [x] `call` with an unregistered server name returns `Err(UnknownServer)`.
- [x] `CallerId` reaches `McpServer::invoke` unchanged.

## Tests
- [x] Unit test: register a fake `McpServer` that records the `CallerId` and `tool` it received; `Dispatcher::call(CallerId::Plugin(..), "srv", "t", json!({"op":"x"}))` reaches the fake with the same caller and tool, and the input map is passed through untouched.
- [x] Unit test: `call` on an unknown server name yields `Err(UnknownServer)`.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — dispatcher routing tests first, then implement.

## Depends on
McpServer trait + ServerRegistry.

## Review Findings (2026-05-16 13:00)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/dispatcher.rs:30-31` — The `Dispatcher` doc comment promises "a `Dispatcher` is cheap to clone and share across the platform's async tasks," but the struct has no `Clone` impl (no derive, no manual impl). A reader following the doc would write `dispatcher.clone()` and hit a compile error. Either add `#[derive(Clone)]` to `Dispatcher` (it holds only an `Arc<ServerRegistry>`, so a derived `Clone` is correct and cheap) or reword the doc to drop the clonability claim — e.g. state that the registry handle is an `Arc` so wrapping the dispatcher in an `Arc` is cheap. Make the doc and the type agree.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/dispatcher.rs:32` — `Dispatcher` is a new public type with a non-empty representation but derives no traits. Per the project's Rust review guidelines (`Debug` on all public types with a non-empty representation; a new public type missing obvious trait impls is a silent semver hazard), add `#[derive(Debug)]`. Sibling public types in the crate follow this — `ToolMetadata`, `PluginId`, and `CallerId` all derive `Debug`. `Arc<ServerRegistry>` is `Debug` only if `ServerRegistry` is; `ServerRegistry` currently derives only `Default`, so deriving `Debug` on `Dispatcher` also requires `#[derive(Debug)]` on `ServerRegistry` (its single `HashMap` field is `Debug` once the trait-object values are addressed, or use a manual `Debug` impl that omits the server map).