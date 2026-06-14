---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff580
title: 'callback-dispatch: views/focus board task-local scope is lost across the plugin callback bridge (real cause of "server is unavailable")'
---
## Summary
The live `ui.setFocus (kernel bridge) failed: ... server is unavailable` and `Failed to load perspectives: ... server is unavailable` faults are NOT a plugin-host server-lifecycle/disposal bug. Diagnosed by running `cargo tauri dev` with targeted DIAG tracing in `swissarmyhammer-plugin` (`route`, `activate_rust_module`, `map_rmcp_error`). The `views`/`focus` in-process servers are ALIVE the whole time — the callbacks fail for two DISTINCT reasons, both then MASKED into the misleading "server is unavailable" string. This is a separate, deeper layer than task 01KTBW3Y1604TCJ2PKD3FPH2ZD (which was about lifecycle/refcount disposal).

## Evidence (live OS log, subsystem com.swissarmyhammer.kanban)
- `DIAG activate_rust_module: OK (moved out of table) id=views` / `id=focus` for EVERY host (global + 3 boards) — servers activate and stay live. No `MISSING`, no reload/reconcile/unload/disposed events at all.
- `DIAG route` NEVER logged a Disposed/Unknown — routing resolves the servers Live.
- The real handler errors (logged by the views/focus crates themselves):
  - `perspective.list` → `no ViewsBoardServices active on this tokio task; the dispatcher must scope a board (see scope_views_board_services) before invoking a views tool  code=ErrorCode(-32603)`
  - `ui.setFocus` → `invalid arguments for op "set focus": missing field 'fq'  code=ErrorCode(-32602)`
- `crates/swissarmyhammer-plugin/src/server/in_process.rs::map_rmcp_error` flattens BOTH (any non-METHOD_NOT_FOUND `ErrorData`) to `Error::ServerUnavailable`, whose Display is "server is unavailable" → surfaces as the confusing CallbackFailed message.

## Root cause 1 (perspective.list) — task-local scope loss across the bridge
`apps/kanban-app/src/commands.rs::try_dispatch_via_command_service` wraps the dispatch future with `swissarmyhammer_views::scope_views_board_services(vsvc, dispatched)` (a `tokio::task_local!` — `CURRENT_VIEWS_BOARD_SERVICES` in `crates/swissarmyhammer-views/src/server.rs`). But `service.dispatch(...)` invokes the command's `execute` callback via `HostCallbackDispatcher::invoke` → `PluginHost::invoke_plugin_callback`, which runs the JS callback on the plugin's ISOLATE WORKER THREAD. When that callback calls `this.views.list()`, the SDK `toolsCall` is serviced by the host bridge's `block_on` on the host's separate long-lived `bridge_runtime` (`crates/swissarmyhammer-plugin/src/host.rs` `HostBridge::block_on` / `BridgeRuntime`). Tokio task-locals do NOT propagate from the dispatch task into the bridge-runtime task, so `CURRENT_VIEWS_BOARD_SERVICES` is UNSET when the `views` tool finally runs → the views resolver returns None → "no board scoped". Same applies to `store`/`entity` task-locals for any callback that reaches back into those servers.

## Root cause 2 (ui.setFocus) — missing `fq` argument to `set focus`
The `ui-commands` plugin's `ui.setFocus` execute callback invokes the `focus` server op `set focus` without the required `fq` field (code -32602 `missing field 'fq'`). The focus arg plumbing from the kernel-bridge dispatch to the `set focus` op is dropping/omitting `fq`. (Investigate `builtin/plugins/ui-commands` setFocus + how the kernel-bridge passes the focus FQM into the command args.)

## Root cause 3 (masking) — make domain errors legible
`map_rmcp_error` collapses every structured handler error into `ServerUnavailable`. It should preserve/propagate the handler's real `ErrorData` (code + message) so a "no board scoped" / "missing field" error is not disguised as a disposed-server fault. This masking is what made the whole bug look like a lifecycle problem.

## Acceptance
- Dispatching `perspective.list` from the live app succeeds; the `views` board task-local (and store/entity) is in scope when the command callback's `views` toolsCall runs — fix the scope propagation across the isolate/bridge boundary (e.g. thread the board services through the callback args / dispatcher, or re-establish the scope inside the bridge for HostInternal-originated callback dispatch).
- `ui.setFocus` passes a well-formed `fq` to `set focus`; no -32602.
- `map_rmcp_error` no longer masks domain errors as `ServerUnavailable` (propagate code+message).
- Real-path/e2e test: dispatch a callback-backed command (e.g. `perspective.list`) through a per-board CommandService and assert the callback's downstream `views` toolsCall sees the scoped board and succeeds — i.e. a test that fails if the task-local is lost across the bridge.
- Validate by RUNNING `cargo tauri dev`: OS log shows NO `server is unavailable` / `Failed to load perspectives` / `no ViewsBoardServices active` / `missing field 'fq'` on board open.

## Note / partial progress on the lifecycle layer
While diagnosing, a real (but DIFFERENT) reload bug was fixed under task 01KTBW3Y1604TCJ2PKD3FPH2ZD: `PluginHost::dispose_handle` now returns a disposed `{ rust }` activation to the available-modules table so a hot-reload re-activation does not fail with `UnknownServer` (regression test `crates/swissarmyhammer-plugin/tests/rust_module_reactivation_e2e.rs`). That fix is correct and orthogonal — it does NOT resolve THIS live symptom, which is the task-local/scoping + arg + masking issue above.