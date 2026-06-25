---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvy0a7r93g19e8d1z5m5evhm
  text: |-
    Implemented. TDD RED->GREEN:
    - Added kernel method SpatialRegistry::remove_layers_for_window(&WindowLabel) -> usize (registry.rs) via HashMap::retain on window_label; returns count removed. RED proof: 2 registry tests failed to compile (method missing), then PASS.
    - Added focus op RemoveLayersForWindow (verb="remove", noun="layers" -> op string "remove layers"), wired into server.rs handle_remove_layers_for_window + dispatch + operations list + lib re-export. RED proof: focus_server e2e test failed with "unknown op \"remove layers\"", then PASS. Updated meta_snapshot snapshot (13 ops now).
    - Host call site: new commands.rs::reconcile_window_layers(state, label) dispatches the op to the SAME host the window's React side uses (per-board host w/ global fallback, mirroring command_tool_call routing). Called from main.rs on_window_destroyed (spawned off the sync OS handler) AND from mcp_subscribe BEFORE bind_window_forwarder (covers Vite full reload, which fires no Destroyed event). Host wiring test reconcile_window_layers_removes_destroyed_windows_overlays PASS.

    Key discovery: the focus registry is NOT in AppState (removed in Stage 3); it lives inside each host's FocusServer (per-board + global). So the reconcile must route to the right host, not a shared AppState registry.

    focus crate: 131 tests pass. Next: full kanban-app suite + fmt/clippy.
  timestamp: 2026-06-24T23:44:14.601215+00:00
- actor: claude-code
  id: 01kvy0s3fx21gywz1hsvcr2cwk
  text: |-
    Adversarial double-check found a HIGH-severity bug in the FIRST implementation and I fixed it:

    BUG: Tauri fires CloseRequested BEFORE Destroyed. on_window_close_requested clears the window->board mapping (remove_window) before on_window_destroyed ran the reconcile. So the destroy-path reconcile resolved board_handle_for_window -> None -> GLOBAL host, but a board window's overlay layers live in the PER-BOARD focus registry. Result: destroy-path reconcile removed 0 from the wrong registry; stale overlays lingered for any board with a per-board platform (the normal case).

    FIX:
    - Moved the reconcile from on_window_destroyed into on_window_close_requested, where the window->board mapping is still live. Capture board_path before remove_window, then in the spawned task resolve the per-board host via new AppState::board_handle_for_path(board_path) (does NOT consult the cleared mapping).
    - reconcile_window_layers now takes an explicit Option<Arc<BoardHandle>> resolved by the caller (mcp_subscribe resolves via board_handle_for_window while mapping is live; close path via board_handle_for_path from captured path). on_window_destroyed now only rebuilds the menu.
    - Added regression test reconcile_resolves_per_board_host_after_window_mapping_cleared: opens a real board (own per-board registry), pushes root+overlay via the per-board host, simulates the close ordering (capture path, clear mapping, resolve via path), reconciles, asserts the PER-BOARD registry is empty.

    Verification: focus 131 pass, kanban-app 193 pass (ai_panel_e2e excluded), fmt clean, clippy clean on touched files (only pre-existing warnings in navigate.rs/state.rs/menu.rs/window-service remain).
  timestamp: 2026-06-24T23:52:21.757049+00:00
- actor: wballard
  id: 01kvy1p149tc5pkzn72x0qr621
  text: |-
    Review resolution — reviewer certified CLEAN on correctness (0 blockers). All 4 load-bearing concerns passed with zero engine findings: exact-window `remove_layers_for_window` (HashMap::retain on window_label, returns count, focus crate stays generic); the real CloseRequested-before-Destroyed routing fix (reconcile runs while window→board mapping is live, captures board path, resolves the PER-BOARD host — guarded by `reconcile_resolves_per_board_host_after_window_mapping_cleared`); reload-path ordering (mcp_subscribe reconciles before bind_window_forwarder + fresh push, window-root self-heal preserved); TDD genuine + conventions honored.

    Two clarity-only warnings — both WAIVED/deferred, neither a defect:
    - commands.rs `reconcile_window_layers` ~52 lines (2 over the ~50 soft threshold) — cohesive function (resolve host → call op → log result); extracting 2 lines adds indirection without clarity gain. Waived.
    - server.rs `call_tool` ~79 lines / 13-arm dispatch — PRE-EXISTING pattern; this task only added one `"remove layers"` arm. Out of scope (a dispatch-table refactor would touch all 13 ops). Not actioned here.

    Verified state holds: focus 131 pass, kanban-app 193 pass, fmt clean, clippy clean on touched files. Moving to done.
  timestamp: 2026-06-25T00:08:09.609516+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe780
title: Stale focus-kernel layers linger after webview reload / window destroy
---
Discovered while verifying 01KTCQFJAR5EJSKMTRD95AX1M8 (HMR command re-mount).

When a webview fully reloads (Vite full page reload in dev) or a window is destroyed, the old page's React effect cleanups never run, so `spatial_pop_layer` is never called for layers that were mounted at that moment. The window-root layer self-heals because the new page re-pushes the same FQM (`SpatialRegistry::push_layer` is an idempotent keyed insert), but OVERLAY layers that were open at reload time (e.g. `/<label>/window/inspector`, `/<label>/window/palette`) linger forever in the kernel's `SpatialRegistry.layers` map.

On window destroy there is no cleanup at all: `on_window_destroyed` in apps/kanban-app/src/main.rs only rebuilds the menu; the focus crate has no remove-layers-for-window op (registry.rs only has push_layer / remove_layer by fq).

Potential impact: stale child layers show up in `children_of_layer` and any topmost-layer / dismiss enumeration over the registry, which could misroute Escape/dismiss after a dev reload, and leak entries when window labels are reused. Low severity — dev-leaning — but the registry should be reconciled.

Suggested fix shape: on `WindowEvent::Destroyed` (and/or webview page-load, e.g. the idempotent `mcp_subscribe` bind for a (label, board) pair) remove all layers whose `window_label` matches the window, before the new page pushes fresh ones. Needs a focus-crate op like `remove layers for window` plus a host call site, TDD with a kernel-level test that a re-bound window starts with only the layers the new page pushed. #ui

## Review Findings (2026-06-24 17:56)

### Warnings
- [ ] `apps/kanban-app/src/commands.rs:878` — reconcile_window_layers is approximately 52 lines of actual code, just over the ~50-line threshold. The function combines two distinct concerns: (1) calling the focus operation via either a per-board or global host, and (2) handling the result with logging. These could be split into helper functions to improve focus and testability. Extract the result-handling logic (match on Ok/Err and logging) into a separate function like `log_reconciliation_result()`. This would reduce reconcile_window_layers to ~20-25 lines focused on calling the operation.
- [ ] `crates/swissarmyhammer-focus/src/server.rs:299` — call_tool is approximately 79 lines of actual code, significantly exceeding the ~50-line threshold. The function is dominated by a single match statement with 13 identical arms that each deserialize an operation, call a handler, and return the result. This repetitive pattern suggests a dispatch table would be more maintainable and would reduce function length by 50%. Create a dispatch map type `HashMap<&str, fn(...) -> BoxFuture<Result<Value>>>` populated during FocusServer::new(), then replace the match with a single map lookup and handler invocation. This reduces call_tool to ~20 lines and makes adding new operations a data-driven change rather than code duplication.