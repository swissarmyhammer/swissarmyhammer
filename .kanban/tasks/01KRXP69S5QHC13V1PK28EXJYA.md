---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8380
project: plugin-arch
title: 'plugin: HostBridge must drive cli/url transports on a long-lived runtime'
---
## What
**Platform bug** discovered by plugin-arch task `01KRRECT97X0SK2DPYGEW28GSE` (the CLI/URL transport e2e tests): a plugin can `this.register({cli:...})` / `this.register({url:...})` and the source connects, but the very next `tools/call` through the bridge fails with `ServerUnavailable`.

Root cause in `crates/swissarmyhammer-plugin/src/host.rs`: `HostBridge::block_on` (~line 1607) spawns a **brand-new ephemeral current-thread Tokio runtime on a scratch thread for every bridge call**, then drops it when the scratch thread exits. `CliServer`/`UrlServer` hold an `rmcp` `RunningService` whose background service loop is a task on whatever runtime was live at `connect` time. When the `register` call's scratch runtime is dropped, that service loop dies; the next bridge call (`toolsCall`) runs on a *different* ephemeral runtime and finds a dead peer.

The `HostBridge` doc comment (host.rs:~1584) already says cli/url connects "are sent to a fresh task on **the host's runtime**" — the doc describes the correct design; the implementation does not match it. The `InProcessServer` (`rust`) transport is unaffected (no background service loop), which is why every e2e test so far passed — T21 is the first to drive a `cli`/`url` source through a real plugin's bridge.

## Fix
Change `HostBridge`'s host-async dispatch so all bridge calls (`register`, `toolsCall`, `unregister`, `flush`, callbacks) run on **one long-lived host runtime**, not a per-call throwaway. Transport background tasks (the rmcp `RunningService` loops behind `CliServer`/`UrlServer`) must survive across bridge calls for the lifetime of the registration. Make the implementation match the `HostBridge` doc comment. Confirm no deadlock (the bridge op runs synchronously on the isolate worker thread — the host runtime must be a separate, persistent runtime, and the bounded-timeout discipline must be preserved).

## Acceptance Criteria
- [x] `HostBridge` dispatches host async work onto a single persistent runtime; no per-call ephemeral runtime is created.
- [x] A `CliServer`/`UrlServer` registered by a plugin survives across bridge calls — a `tools/call` after `register` succeeds.
- [x] The `HostBridge` doc comment and the implementation agree.
- [x] `InProcessServer` (`rust`) transport behavior is unchanged; existing tests still pass.

## Tests
- [x] The already-written T21 e2e tests `crates/swissarmyhammer-plugin/tests/cli_server_e2e.rs` and `tests/url_server_e2e.rs` are the regression gate — they currently FAIL with `ServerUnavailable` and must go GREEN after this fix.
- [x] `cargo test -p swissarmyhammer-plugin` — all green (including the two e2e tests).
- [x] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` and `cargo build --workspace` — clean.

## Workflow
- The T21 e2e tests are pre-written and serve as the RED→GREEN gate; no new `/tdd` test needed for the gate itself, but add a unit test if one cleanly pins the long-lived-runtime invariant.

## Depends on
PluginHost lifecycle (the `HostBridge` being fixed).

## Resolution
`HostInner` now owns a `BridgeRuntime`: one multi-thread Tokio runtime created at host construction (`with_roots`) and alive for the host's whole lifetime. The runtime is moved onto a dedicated parked OS thread and only its `Handle` is kept — so the runtime's blocking drop always runs on a plain non-async thread, never inside the embedder's own runtime (every `#[tokio::test]` drops the last `PluginHost` clone inside an async context). `HostBridge::block_on` became a `&self` method that submits the future via `Handle::spawn` + an `mpsc` channel and blocks the isolate worker thread on `recv_timeout(BRIDGE_TIMEOUT)` — the bounded-timeout discipline is preserved, and no per-call runtime is ever created. The `HostBridge` doc comment now describes the real (fixed) design. Added unit test `host::tests::a_task_spawned_in_one_bridge_call_outlives_it` pinning the long-lived-runtime invariant. T21 e2e tests went RED (`ServerUnavailable`) → GREEN.