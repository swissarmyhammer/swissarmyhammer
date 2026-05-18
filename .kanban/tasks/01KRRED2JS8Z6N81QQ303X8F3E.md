---
assignees:
- claude-code
depends_on:
- 01KRRE7WP7YY56R7MTBHJHZD12
- 01KRRE8D4712C7785TRN3GGR4H
- 01KRREC7YF5ENG2M2E7DQYSDGS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8580
project: plugin-arch
title: 'plugin: SDK e2e tests — operation _meta round-trip and callbacks'
---
## What
Capability integration tests for the SDK's operation-tool path sugar and the callback primitive, following the `files_dispatch_e2e.rs` reference shape.

`crates/swissarmyhammer-plugin/tests/integration/`:
- `operation_meta_e2e.rs` — register a real operation tool (one carrying `_meta["io.swissarmyhammer/operations"]`). A probe plugin calls `this.<server>.<tool>.<noun>.<verb>({...})`; assert the tool receives a `tools/call("<tool>", { op: "<verb> <noun>", ... })` — the SDK read `_meta` and built `op` from the path. Also assert the direct form `this.<server>.<tool>({op:..., ...})` works and an unknown verb path raises `UnknownOperation`.
- `callback_e2e.rs` — a probe plugin passes a function across the boundary; the host invokes it via `notifications/callbacks/invoke`; assert the function ran and its return value flowed back to where the plugin awaits it (observe via a value the plugin writes through the `files` server or returns).

Each test: own `TempDir`, fresh `PluginHost`, no shared/`static` state.

## Acceptance Criteria
- [x] `operation_meta_e2e.rs` proves the `_meta` round-trip: path-form call → `tools/call(tool, {op, ...})`; direct form works; unknown verb → `UnknownOperation`.
- [x] `callback_e2e.rs` proves a plugin-supplied function is invoked by the host and its return value flows back.
- [x] Both follow the reference-test isolation model; no mocked dispatcher/registry.

## Tests
- [x] Run: `cargo test -p swissarmyhammer-plugin` — the two new `*_e2e.rs` tests and the whole suite green.
- [x] Each test must genuinely fail if the SDK `_meta` resolution or the callback primitive is broken.

## Workflow
- Tests are the deliverable; no `/tdd` cycle. Reuse the harness/helpers from `files_dispatch_e2e.rs`.

## Depends on
TypeScript SDK, callback primitive, and the reference `files_dispatch_e2e.rs` harness.

## Implementation notes
- Tests are flat `tests/*.rs` files (`tests/operation_meta_e2e.rs`, `tests/callback_e2e.rs`) matching the crate convention — `tests/integration/` is not a crate convention here.
- `operation_meta_e2e.rs` drives the real `files` operation tool (a genuine operation tool carrying `io.swissarmyhammer/operations` `_meta`) through `PluginHost::discover_and_load_all`, identical harness to `files_dispatch_e2e.rs`.
- `callback_e2e.rs` loads a real multi-file plugin bundle from disk via `PluginRuntime::call_plugin_lifecycle` and invokes the plugin-supplied callback via `PluginRuntime::invoke_callback` (`notifications/callbacks/invoke`). Its `HostDispatcher` routes `toolsCall` into the real in-process `files` tool (obtained via `McpServer::plugin_tool_modules`) — no mocked registry/tool — and records `callbackDispatch` markers exactly as the production host's `callback_dispatch` does.
- Verified both tests genuinely fail when the SDK behavior is broken: breaking `_meta` op resolution in `plugin.ts` fails `operation_meta_e2e`; breaking `invokeStoredCallback` in `plugin.ts` fails `callback_e2e`. SDK restored after.