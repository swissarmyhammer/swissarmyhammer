---
assignees:
- claude-code
depends_on:
- 01KS36P9C8CFT5HMQWY2WCA9ZE
position_column: todo
position_ordinal: '8580'
project: command-service
title: Wire Command service into host bootstrap + ledger-driven auto-cleanup
---
## What

Expose the Command service as an in-process MCP server through `swissarmyhammer-tools`' bootstrap, and integrate with the per-plugin ledger so plugin unload auto-purges that plugin's registrations.

Files:
- `crates/swissarmyhammer-tools/src/mcp/server.rs` (or whichever bootstrap location plugin-arch chose) — `host.expose_rust_module("commands", CommandService::new(callback_dispatcher.clone()))`
- `crates/swissarmyhammer-plugin/src/ledger.rs` (touched, not owned by this task) — Command service hooks an `Opaque(Box::new(move || registry.purge_caller(caller)))` into the ledger at construction so the platform can dispose without knowing the service's internals

Wiring:
- The plugin platform creates a `CallbackDispatcher` (already exists from plugin-arch); pass it as `Arc<dyn CallbackDispatcher>` to `CommandService::new`
- On every `register command`, the service appends an `Opaque` dispose-fn to the calling caller's ledger entry — that fn calls `registry.purge_caller(caller_id)` and `flush()` the notification debouncer. (Multiple register calls from one caller append multiple dispose-fns, but they're idempotent — `purge_caller` is harmless to call twice.)
- Alternative simpler design: at service construction, the platform gives the service a callback that fires "this caller is unloading" — service handles it directly. Choose whichever fits the ledger primitive cleanest in plugin-arch.

This is the milestone where the architecture-doc test scenario becomes possible: two probe plugins register the same id; second wins; second unloads; first re-emerges.

## Acceptance Criteria
- [ ] `swissarmyhammer-tools` (or designated bootstrap crate) exposes `commands` as an in-process server at startup
- [ ] Calling `tools/call("command", { op: "register command", ... })` from any caller through the plugin platform's dispatcher succeeds
- [ ] When a plugin unloads, all its registrations are purged automatically (no need for the plugin to call `unregister`)
- [ ] Override stack re-emergence works after plugin unload: plugin B overrides plugin A's command; B unloads; A's command re-emerges as active
- [ ] `notifications/commands/changed` is flushed at the plugin-unload boundary

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/host_bootstrap_e2e.rs` — real `PluginHost::for_tests`; assert the `commands` server appears in the registry; a host caller registers a command; assert it appears in `list`
- [ ] `crates/swissarmyhammer-command-service/tests/integration/unload_cleanup_e2e.rs` — write a probe plugin in a temp dir that registers `probe.foo` in `load()`; load the plugin; assert `list` shows it; unload the plugin; assert `list` is empty; assert no zombie callback ids remain
- [ ] `crates/swissarmyhammer-command-service/tests/integration/override_stack_e2e.rs` — two probe plugins both register `core.archive` with different `execute` callbacks that write distinct sentinel files; load A then B; execute → B's file appears; unload B; execute → A's file appears; unload A → host's original registration is active again
- [ ] `cargo test -p swissarmyhammer-command-service --test integration` passes

## Workflow
- Use `/tdd` — write `override_stack_e2e.rs` first; it's the headline scenario from command-service.md and exercises the full stack.

Depends on plugin-arch tasks: per-plugin ledger, callback dispatcher, `expose_rust_module`, host bootstrap.