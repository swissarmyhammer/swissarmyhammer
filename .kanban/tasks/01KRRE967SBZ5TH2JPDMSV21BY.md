---
assignees:
- claude-code
depends_on:
- 01KRRE573P9VNJ9ZSVEB9AV1K6
- 01KRRE5H68SDGZZDGCNMJWEMMK
- 01KRRE6XMJAK3WH3EVMPTMZX8M
- 01KRRE7CA82C81G10NG4GB27HE
- 01KRRE7WP7YY56R7MTBHJHZD12
position_column: todo
position_ordinal: '8e80'
project: plugin-arch
title: 'plugin: PluginHost lifecycle and per-plugin ledger'
---
## What
Implement `PluginHost` — the top-level object that loads/unloads plugins, owns the `ServerRegistry` + `Dispatcher`, bridges the SDK transport to the dispatcher, and tracks every plugin registration in a per-plugin ledger so unload disposes everything without the plugin's cooperation.

In `crates/swissarmyhammer-plugin/src/host.rs` + `src/ledger.rs`:
- `PluginHost` owns `Arc<ServerRegistry>`, a `Dispatcher`, and a map of loaded plugins (each with its own isolate from the runtime task).
- Loading a plugin: create a fresh isolate, install the module loader, evaluate the entry `.ts`, instantiate the `Plugin` subclass, call `load()`. The SDK transport's host bridge is wired here — `tools/call` from a plugin's Proxy flows through `Dispatcher::call` with `CallerId::Plugin(id)`; `this.register(name, source)` connects the source (in-process `rust` id, `cli`, or `url`) and inserts it into the `ServerRegistry`; `unregister` removes it.
- `expose_rust_module(id, server)` — registers an rmcp handler in a separate "available modules" table (NOT the live registry); a later `register(name, {rust: id})` activates it. Decouples compiled-in Rust code from which servers are live and under what names.
- `PluginLedger` (`src/ledger.rs`): `HashMap<PluginId, Vec<RegistrationHandle>>` where `RegistrationHandle` is `Server(ServerName)`, `Callback(CallbackId)`, or `Opaque(Box<dyn FnOnce()+Send>)`. Every long-lived registration a plugin makes appends to its vec.
- `unload(plugin_id)`: drain the plugin's ledger vec in reverse, dispose each handle (unregister servers, drop callbacks, run opaque dispose-fns), then tear down the isolate. `unload()` on the `Plugin` is optional and called only for external side effects.
- **Constructors take explicit layer roots so the platform stays host-agnostic** — the kanban app, CLI, and TUI each compute and supply their own; the platform hardcodes no global/`SwissarmyhammerConfig` config:
  - `PluginHost::for_tests(...)` — explicit roots for tests.
  - `PluginHost::new(...)` — production constructor taking the builtin plugin set + the writable layer roots (user, optionally project). The host computes its roots and passes them in (e.g. the kanban app passes `~/.config/kanban/plugins` — see the kanban-app bootstrap task).

Scope boundary: discovery from disk and hot-reload triggering are separate tasks; this task delivers explicit `load(plugin_dir)` / `unload(id)` APIs, the ledger, and both constructors.

## Acceptance Criteria
- [ ] `PluginHost` loads a plugin from a directory, runs `load()`, and `this.register(...)` inserts a server reachable through the `Dispatcher`.
- [ ] `expose_rust_module` makes a Rust module available; `register(name, {rust: id})` activates it under a chosen name.
- [ ] Every `register`/callback append to the per-plugin ledger; `unload` drains it in reverse and disposes all of them.
- [ ] After `unload`, the plugin's registered servers are gone from the registry and the isolate is torn down.
- [ ] Both `PluginHost::for_tests` and `PluginHost::new` exist and take explicit layer roots; the platform hardcodes no host-specific directory config.

## Tests
- [ ] Integration test: `PluginHost::for_tests` + a probe plugin whose `load()` registers a server and calls a tool on it; assert the call succeeds.
- [ ] Unload disposal test: after `unload`, calls into the plugin's server fail with `ServerUnavailable` and the ledger for that plugin is empty.
- [ ] Test `expose_rust_module` + `register({rust:id})` activation path with a real in-process rmcp server.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the load → register → call and unload → disposal tests first, then implement.

## Depends on
Dispatcher, InProcessServer, deno_core runtime, module loader, TypeScript SDK.