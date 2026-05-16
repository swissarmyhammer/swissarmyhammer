---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: todo
position_ordinal: '8680'
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
- [ ] `Dispatcher` exists, holds `Arc<ServerRegistry>`, and `call` forwards to the resolved server's `invoke`.
- [ ] `call` with an unregistered server name returns `Err(UnknownServer)`.
- [ ] `CallerId` reaches `McpServer::invoke` unchanged.

## Tests
- [ ] Unit test: register a fake `McpServer` that records the `CallerId` and `tool` it received; `Dispatcher::call(CallerId::Plugin(..), "srv", "t", json!({"op":"x"}))` reaches the fake with the same caller and tool, and the input map is passed through untouched.
- [ ] Unit test: `call` on an unknown server name yields `Err(UnknownServer)`.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — dispatcher routing tests first, then implement.

## Depends on
McpServer trait + ServerRegistry.