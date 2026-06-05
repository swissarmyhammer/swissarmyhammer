---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff780
title: Fix "server is unavailable" callback failure on command dispatch (ui.setFocus, perspective.list)
---
## STATUS: lifecycle-disposal fix landed; live symptom has a DISTINCT root cause now tracked in 01KTBXTS8BH1VWPTS7AK7V9XFC

### What was fixed here (real reload/lifecycle bug — the area this card scoped)
`PluginHost::dispose_handle` (`crates/swissarmyhammer-plugin/src/host.rs`) now returns a disposed `{ rust }` module to the host's available-modules table when its LAST holder unregisters. Previously, activating a `{ rust }` source MOVED the module out of the table (one-shot) and disposal NEVER restored it, so any unload/hot-reload of the activating plugin left the module gone forever: the v2 `register({rust:id})` failed with `UnknownServer`, the plugin's `load()` threw, the load rolled back, and the registered name stayed tombstoned → callers into it saw `ServerUnavailable`. This is exactly the "loading/closing a board disposes servers a command needs" hypothesis from the original card. Fix is refcount-correct (only on `UnregisterOutcome::Removed`, only for `ServerSource::Rust`, under one lock span) and regression-tested by `crates/swissarmyhammer-plugin/tests/rust_module_reactivation_e2e.rs` (RED before, GREEN after).

### Why this did NOT clear the LIVE symptom (validated by running)
Ran `cargo tauri dev` with targeted DIAG tracing. The `views`/`focus` servers are ALIVE the entire time (activate OK in every host; `route` never resolves Disposed/Unknown; no reload/reconcile/unload events). The live `server is unavailable` faults come from `in_process.rs::map_rmcp_error` MASKING two real handler errors:
- `perspective.list` → `no ViewsBoardServices active on this tokio task` (-32603): the `views` board `tokio::task_local!` set by `try_dispatch_via_command_service` does NOT propagate across the command-callback round-trip into the plugin isolate + back through the host `bridge_runtime`, so the `views` toolsCall runs UNSCOPED.
- `ui.setFocus` → `invalid arguments for op "set focus": missing field 'fq'` (-32602): the focus arg plumbing drops `fq`.

Both are a distinct, deeper layer (task-local scope propagation across the plugin callback bridge + focus arg plumbing + error masking) — out of this card's stated lifecycle scope. Full evidence and acceptance in 01KTBXTS8BH1VWPTS7AK7V9XFC.

---
## (Original description)
After the command-dispatch regression fix (commands_registry façade now populated — count=67, no more Unknown command), dispatch reaches the CommandService but the plugin-callback step fails with `server is unavailable` for `ui.setFocus` and `perspective.list`. Original hypothesis: a per-board vs global host lifecycle/refcount bug disposing the `focus`/`views` in-process servers a command callback reaches back into.