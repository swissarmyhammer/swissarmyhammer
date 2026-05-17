---
assignees:
- claude-code
depends_on:
- 01KRRE573P9VNJ9ZSVEB9AV1K6
- 01KRRE5H68SDGZZDGCNMJWEMMK
- 01KRRE6XMJAK3WH3EVMPTMZX8M
- 01KRRE7CA82C81G10NG4GB27HE
- 01KRRE7WP7YY56R7MTBHJHZD12
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffa80
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
- [x] `PluginHost` loads a plugin from a directory, runs `load()`, and `this.register(...)` inserts a server reachable through the `Dispatcher`.
- [x] `expose_rust_module` makes a Rust module available; `register(name, {rust: id})` activates it under a chosen name.
- [x] Every `register`/callback append to the per-plugin ledger; `unload` drains it in reverse and disposes all of them.
- [x] After `unload`, the plugin's registered servers are gone from the registry and the isolate is torn down.
- [x] Both `PluginHost::for_tests` and `PluginHost::new` exist and take explicit layer roots; the platform hardcodes no host-specific directory config.

## Tests
- [x] Integration test: `PluginHost::for_tests` + a probe plugin whose `load()` registers a server and calls a tool on it; assert the call succeeds.
- [x] Unload disposal test: after `unload`, calls into the plugin's server fail with `ServerUnavailable` and the ledger for that plugin is empty.
- [x] Test `expose_rust_module` + `register({rust:id})` activation path with a real in-process rmcp server.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the load → register → call and unload → disposal tests first, then implement.

## Depends on
Dispatcher, InProcessServer, deno_core runtime, module loader, TypeScript SDK.

## Implementation note
The unload-disposal test asserts the disposed server is gone from the live registry *and* that a call into the disposed name fails with `Error::ServerUnavailable`. The `ServerRegistry` keeps a tombstone for every name it has unregistered: `unregister` records the freed name in a `disposed` set, `register` clears any tombstone, and `resolve(name)` returns `ServerStatus::Live` / `Disposed` / `Unknown`. The host's `route` primitive consults `resolve` so a disposed name fails with `Error::ServerUnavailable` (registered then disposed) while a never-registered name fails with `Error::UnknownServer` — the spec-mandated distinction. The `Dispatcher` type holds an immutable `Arc<ServerRegistry>` while the host's registry is mutated in place as plugins register/unregister; the host therefore routes through a shared `route` primitive that performs the identical `(server, tool)` resolution `Dispatcher::call` does.

## Review Findings (2026-05-17 16:05)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/host.rs:330-349,420-449` — Disposed/unregistered servers resolve to `Error::UnknownServer`, contradicting the architecture spec. `ideas/plugins/plugin-architecture.md` is explicit in *two* authoritative places: the *Unregistration* section (lines 554-559) states "Consumers' in-flight calls into that server reject with `ServerUnavailable`; subsequent calls fail the same way until the server is re-registered", and the integration-test matrix (line 1449) names the unload-disposal test as "its registered server fails with `ServerUnavailable`". The error vocabulary was designed for exactly this distinction — `Error::ServerUnavailable` is documented as "registered but currently not able to serve" and `Error::PluginReloaded` exists alongside it — so a consumer can tell "the server I was using was disposed out from under me" apart from "I named a server that never existed". The implementer's note reasons that full removal makes `UnknownServer` "accurate", but that trades away an intentional, spec-mandated semantic signal. Fix: on `dispose_handle`/`unregister` for a plugin-registered server, leave a tombstone (or have `route` consult a disposed-names set) so a call into a disposed server name returns `ServerUnavailable`; reserve `UnknownServer` for names that were never registered. Update the unload-disposal integration test (`tests/plugin_host.rs:186-196`) and the task's Implementation note to match. This deviation reaches the public error contract, so it should be resolved rather than silently accepted.
- [x] `crates/swissarmyhammer-plugin/src/host.rs:373-379` — `unload` on an unknown plugin id returns `Error::UnknownServer`, conflating "no such plugin" with "no such server". A caller cannot distinguish the two failures, and the `Error` enum has no plugin-not-loaded variant. The doc comment even says "Returns `Error::UnknownServer` when no plugin is loaded under `plugin_id`", documenting the conflation rather than fixing it. Suggest an `Error::UnknownPlugin` variant so unload of a stale id reports the actual failure mode.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/host.rs:653-659` — `HostBridge::unregister` ignores the result of `registry.unregister` and `ledger.consume_server`. A plugin unregistering a name it never registered (or already removed) silently succeeds. A `tracing::debug!` when `unregister` returns `None` — matching the diagnostic `dispose_handle` already emits at line 436-440 for the same situation — would make a buggy plugin observable.
- [x] `crates/swissarmyhammer-plugin/src/host.rs:88,547` vs `runtime/mod.rs:85` — `BRIDGE_TIMEOUT` (30s) equals the runtime's `COMMAND_TIMEOUT` (30s). A bridge call that itself triggers a runtime command races two equal timers; the worker-command timeout and the bridge timeout can fire near-simultaneously, making which error surfaces nondeterministic. Consider making `BRIDGE_TIMEOUT` modestly longer than `COMMAND_TIMEOUT` so the inner timeout wins deterministically, or add a comment explaining the equality is intentional.
- [x] `crates/swissarmyhammer-plugin/src/host.rs:807-808` — `tools_to_json` maps a `serde_json::to_value` failure to `Value::Null`, silently dropping a tool from the `toolsList` response a plugin receives. Serializing an `rmcp::model::Tool` should never fail in practice, but if it ever did the plugin would see a `null` entry in its tools array with no diagnostic. A `tracing::warn!` on the error arm would surface the (unexpected) case.