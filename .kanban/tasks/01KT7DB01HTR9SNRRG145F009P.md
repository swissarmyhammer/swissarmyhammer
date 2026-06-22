---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvrcmnkqz9d5e4sckcnh30dc
  text: |-
    Picked up. Precondition research complete.

    PRECONDITION ANSWER: The SDK on/subscribe/once API is LIVE (sdk/plugin.ts makeServerOn/subscribe/Transport.subscribe). Dep egc0wxq is DONE — real-pipeline .on() delivery proven in crates/swissarmyhammer-plugin/tests/event_subscription_e2e.rs (real_command_execute_fires_the_on_subscriber).

    BUT: NO AI-streaming notification plane exists. NotificationBridge has exactly 4 planes (store/changed, commands/executed, commands/changed + tools/list_changed, ui_state/changed + store/undo_changed). No notifications/ai/* publisher anywhere. The only "streaming" notifications are ACP session/update — a different, non-subscribable transport. So I must ESTABLISH a publisher, mirroring BridgeActionSink → NotificationBridge::publish.

    DEAD PLUMBING CONFIRMED: UIState::ai_streaming() has ZERO production readers (callgraph edges:[]; only its own 3 unit tests in state.rs call it). The write path is live (ai-panel-container.tsx invoke("ai_set_streaming") → models.rs ai_set_streaming → UIState::set_ai_streaming) but the intended Rust AiCancelCmd::available() reader was never built (gate moved to frontend aiStreaming() in ai/commands.ts). Write-only dead-end.

    PLAN: Replace the dead ai_set_streaming write-only path with a live publisher. The ui_state MCP tool already models ephemeral UI state and the ui_state/changed plane exists. Approach: webview publishes streaming start/stop via the ui_state service onto NotificationBridge as notifications/ui_state/changed (key="ai_streaming"); declare that notification in the ui_state tool _meta so the ai-commands plugin can this.uiState.on("...", ...); ai-commands caches the flag and adds synchronous available to ai.cancel. Verifying the exact wiring (how the Tauri/webview reaches the bridge, whether ui_state already publishes ui_state/changed) before writing.
  timestamp: 2026-06-22T19:24:12.791664+00:00
- actor: claude-code
  id: 01kvrctzp76vh9j1ss7a8kravd
  text: |-
    DESIGN FINALIZED after full wiring trace.

    KEY: No production publisher of ui_state/changed exists either — McpNotification::ui_state_changed only has a test caller. So I'm establishing the first production ui_state-plane publisher. Builtin plugins (incl ai-commands) load into the GLOBAL PluginPlatform.host; ui_state server is exposed on that SAME global host; so the plugin's .on() subscribes against the global host's event pump draining the GLOBAL bridge. AI streaming is app-wide (not board-scoped), so the publisher targets the global bridge — clean and consistent. The webview is the SOURCE (already has aiStreaming() locally) and doesn't need to receive it back, so the per-board-forwarder caveat doesn't apply.

    IMPLEMENTATION (mirror commands/executed pattern: declared struct + from_declared + publish):
    1. ui-state crate: #[notification(method="notifications/ui_state/ai_streaming", event="aiStreaming")] struct AiStreamingChanged{streaming:bool} (pub, exported). Add ui_state_notifications() slice, wire notifications: into build_tool_definition so the ui_state tool _meta declares it for .on().
    2. Tauri ai_set_streaming (apps/kanban-app/src/ai/models.rs): make async, reach state.plugin_platform.lock().await.host().notification_bridge(), publish McpNotification::from_declared(payload.method(), &AiStreamingChanged{streaming}, Provenance::user()). Remove dead UIState.ai_streaming field/get/set + its 3 unit tests (zero prod readers).
    3. ai-commands plugin: ensureServices(["commands","ui_state"]); private streaming=false; this.ui_state.on("aiStreaming", p=>{this.streaming = !!p.streaming}); add available:()=> this.streaming ? {ok:true} : {ok:false, reason:"No AI generation is running"} to ai.cancel.
    4. Tests: real-pipeline integration test (mirror event_subscription_e2e) — load ai-commands-like plugin, install commands+ui_state modules, publish ai_streaming notification on bridge, assert available command for ai.cancel flips false→true→false. Update app-shell.ai-commands.test.tsx if relevant. Plugin bundle/SDK tests + meta_tree test for the new notification.

    Starting TDD.
  timestamp: 2026-06-22T19:27:39.719034+00:00
- actor: claude-code
  id: 01kvrg5gn3w81cnwddqkmfan2a
  text: |-
    IMPLEMENTED + all gates green. Establishing double-check before review.

    PRECONDITION ANSWER (final): No AI-streaming event plane existed; I established the FIRST production publisher on the ui_state notification plane (the ui_state/changed plane constructor existed but had ZERO production callers — only a test). Mirrored the BridgeActionSink→NotificationBridge::publish pattern exactly (declared #[notification] struct + from_declared + publish), no parallel mechanism.

    WHAT I IMPLEMENTED:
    1. ui-state crate: #[notification(method="notifications/ui_state/ai_streaming", event="aiStreaming")] struct AiStreamingChanged{streaming:bool}; UI_STATE_NOTIFICATIONS slice + notifications(); ai_streaming_notification(bool)->McpNotification helper; wired notifications: into build_tool_definition; exported from lib. 3 new unit tests + 1 new meta_snapshot integration test pin the declaration + _meta discovery surface.
    2. ai-commands plugin: ensureServices(["commands","ui_state"]); this.ui_state.on("aiStreaming", p=>cachedStreaming = p?.streaming===true); ai.cancel gains synchronous available:()=> cachedStreaming?{ok:true}:{ok:false,reason:"No AI generation is running"} (ok:false set explicitly). Module-level flag is per-isolate (one plugin instance per isolate).
    3. ai_set_streaming Tauri command (models.rs): now async, publishes ai_streaming_notification onto the GLOBAL host bridge (plugins load into global host & subscribe against its bridge; AI streaming is app-wide). Webview is the SOURCE so doesn't need the per-board forwarder.
    4. full_baseline_e2e: exposes the REAL UiStateServer (not generic stub) so .on("aiStreaming") resolves.

    DEAD PLUMBING REMOVED: UIState.ai_streaming field + ai_streaming() getter + set_ai_streaming() setter + initializer + 3 unit tests (verified zero production readers via grep — now zero references repo-wide).

    NEW REAL-PIPELINE TEST: builtin_ai_commands_e2e.rs — loads the real ai-commands bundle, exposes real commands+ui_state, publishes the streaming notification on the bridge, asserts available command for ai.cancel flips false(idle)→true(streaming)→false. PASSES.

    GATES (actual):
    - cargo nextest run -p swissarmyhammer-command-service: 167 passed, 0 failed (incl new builtin_ai_commands_e2e + full_baseline 3/3).
    - cargo nextest run -p swissarmyhammer-ui-state: 140 passed (incl 3 new notification unit tests + new meta_snapshot test).
    - cargo nextest -p kanban-app ai::models: 15 passed. (Pre-existing FAILS in plugins::tests [board-init harness] + ai_panel_e2e llama TIMEOUTs are unrelated — confirmed they fail identically with my changes stashed.)
    - npx tsc --noEmit: clean (exit 0).
    - vitest: app-shell.ai-commands.test.tsx + ai/commands.test.ts + ai-plugin-commands-mirror drift guard + no-direct-invoke + ai-panel.spatial + mode-indicator + ai-panel-dock: all passed (48 tests).
    - cargo clippy --no-deps -p swissarmyhammer-ui-state -p swissarmyhammer-command-service -p kanban-app -- -D warnings: clean. (clippy --tests surfaces PRE-EXISTING nits in untouched files builtin_nav_commands_e2e.rs / no_stale_imports.rs / caption.rs — out of scope, not in my files.)
    - cargo fmt: applied, clean.
  timestamp: 2026-06-22T20:25:50.499648+00:00
- actor: claude-code
  id: 01kvrkf2rbpnrgthdy6z1gwytq
  text: |-
    Addressed the 3 review findings (2026-06-22 16:05). All 3 checkboxes flipped to [x].

    == BLOCKER 1 — host mismatch (the real bug) ==
    Root cause: `ai_set_streaming` published the `ai_streaming` notification to the GLOBAL host's bridge only, but the palette gates `ai.cancel` via `command_tool_call`'s `available command` op routed to the PER-BOARD host (the AI panel mounts in a board window). The per-board `ai-commands` isolate's `cachedStreaming` never flipped -> "Stop AI Generation" stuck disabled mid-stream.

    How I resolved streaming window -> per-board host:
    - `apps/kanban-app/src/ai/models.rs`: `ai_set_streaming` now takes a `tauri::Window` (Tauri server-injects it; the JS `invoke("ai_set_streaming", { streaming })` in ai-panel-container.tsx is unchanged) and delegates to a new testable seam `pub(crate) async fn publish_ai_streaming(state, window_label, streaming)`.
    - `publish_ai_streaming` resolves the bridge via `crate::commands::resolve_window_bridge(state, window_label)` — the SAME helper `command_tool_call`/`mcp_subscribe` already use (board_handle_for_window(label) -> .platform() -> per-board host's bridge, global fallback for a boardless window). Publish + availability query now hit the same isolate.
    - `apps/kanban-app/src/commands.rs`: made `resolve_window_bridge` `pub(crate)` (was private); no logic change.
    - Dispatch-time consistency: `try_dispatch_via_command_service` ALREADY routes to the per-board host (match active_handle.and_then(|h| h.platform())). So the publish was the only path going to global; the fix makes publish + availability + dispatch all land on the same per-board isolate. No change needed there.
    - Per-window semantics preserved: routing to ONE board's host (not publish-to-all) means other board windows that aren't streaming are NOT wrongly enabled. (Note: two windows of the SAME board share one per-board isolate, so they'd both reflect that board's streaming — inherent to per-board host sharing; the per-webview dispatch-time aiStreaming() gate remains authoritative for execution.)

    == BLOCKER 2 — topology-blind test ==
    - `crates/swissarmyhammer-command-service/tests/integration/builtin_ai_commands_e2e.rs`: extracted `build_ai_commands_host()` helper (real PluginHost + ai-commands bundle + ui_state server + commands module) and added `ai_streaming_flag_is_independent_per_host`: builds a GLOBAL host PLUS a PER-BOARD host (two independent isolates), publishes on the per-board bridge, asserts ONLY the per-board isolate's `ai.cancel` flips while the global stays disabled — the per-host independence the single-host test structurally cannot observe.
    - The TRUE end-to-end routing regression (window -> per-board host resolution through the production seam) lives in the kanban-app crate, which owns the Tauri window->board routing the command-service crate cannot reach: `kanban_app::plugins::tests::ai_set_streaming_reaches_per_board_host_for_a_board_window` (seeds a real board, binds a window label, calls publish_ai_streaming, asserts the per-board host's ai.cancel flips false->true->false). Adjusted the e2e doc comment to accurately state what each test covers (was overstated as catching the routing bug itself) per the double-check finding.

    == NIT ==
    - `apps/kanban-app/ui/src/components/ai-panel.tsx`: dropped the stale "(plus the backend `UIState.ai_streaming` flag)" clause above the setAiStatus effect; the gate is now the ai-commands plugin's cached flag. Also updated builtin/plugins/ai-commands/index.ts's publish-target doc comment (was "loads into that same (global) host").

    == RED -> GREEN proof (kanban-app routing test, TDD) ==
    RED (seam reverted to global-only publish): `timed out waiting for ai.cancel available.ok == true; last: {"ok":false,"reason":"No AI generation is running"}` — per-board host never saw the flag. (idle assertion passed first, proving the test reaches the per-board host and only the publish target was wrong.)
    GREEN (per-board-resolved publish): `test plugins::tests::ai_set_streaming_reaches_per_board_host_for_a_board_window ... ok` (2.0s).

    == Gates (actual) ==
    - cargo nextest -p swissarmyhammer-command-service -p swissarmyhammer-ui-state: 312 passed, 0 failed (incl new ai_streaming_flag_is_independent_per_host + existing single-host e2e).
    - cargo nextest -p kanban-app ai_set_streaming_reaches_per_board_host_for_a_board_window: 1 passed; ai::models 15 passed.
    - Full -p kanban-app run: my new test PASSES; the 10 FAILs are ALL the pre-existing `open_board should succeed: "...has no board entity..."` board-init harness failures (open_temp_board doesn't seed a board — a helper I did not touch) + ai_panel_e2e llama tests passed this run. Confirmed unrelated (identical root-cause string across all 10).
    - cd apps/kanban-app/ui && npx tsc --noEmit: exit 0, clean.
    - vitest: app-shell.ai-commands.test.tsx + ai/commands.test.ts + ai-panel.spatial.test.tsx (38) + no-direct-invoke guard (1): all pass.
    - cargo clippy --no-deps -p swissarmyhammer-ui-state -p swissarmyhammer-command-service -p kanban-app -- -D warnings: clean (exit 0); --tests on touched crates surfaces no warnings in my files.
    - cargo fmt: applied, clean.

    double-check (adversarial): REVISE -> Finding 2 (e2e doc overstated) FIXED by accurate doc rewrite + test rename to ai_streaming_flag_is_independent_per_host. Finding 1 was advisory about my prompt summary undersell, not a code defect — code verified correct & compiling; proceeding per really-done advisory contract. Used an isolated CARGO_TARGET_DIR=target/ai-streaming-fix, now removed.
  timestamp: 2026-06-22T21:23:29.675879+00:00
- actor: claude-code
  id: 01kvrkm5vn0w4gezreag3mp69r
  text: |-
    Addressed all 3 review findings (2 blockers + 1 nit). All findings flipped to [x].

    ## BLOCKER 1 — host mismatch (the real bug) — FIXED
    The streaming publish now lands on the host that ANSWERS the palette for the streaming window, not the global host.

    How the streaming window → per-board host is resolved:
    - `ai_set_streaming` (apps/kanban-app/src/ai/models.rs) now takes the Tauri `Window` (Tauri injects it server-side; the JS caller `invoke("ai_set_streaming", { streaming })` is UNCHANGED — window params aren't passed from JS).
    - It delegates to a new testable seam `pub(crate) async fn publish_ai_streaming(state, window_label, streaming)` which resolves the bridge via `crate::commands::resolve_window_bridge(state, window_label)` and publishes there.
    - `resolve_window_bridge` is the EXACT same window→board→host resolution `command_tool_call` uses to route the `available command` op: `board_handle_for_window(label)` → per-board host's `notification_bridge()` when the window has a board, else the global host's bridge. So the publish and the availability query now hit the SAME isolate's cached flag.
    - Made `resolve_window_bridge` `pub(crate)` (was private) so the seam reuses it — no duplicate routing logic.
    - Per-board routing (not publish-to-all) is correct: a publish for a streaming board window flips ONLY that board's `ai.cancel`, never other board windows that aren't streaming.
    - Dispatch-time consistency: `try_dispatch_via_command_service` ALREADY routes to the per-board host (`match active_handle.and_then(|h| h.platform())`), so this fix makes the publish path consistent with both the availability path and the dispatch path. No change needed there.
    - Updated the stale "loads into that same (global) host" doc in builtin/plugins/ai-commands/index.ts to describe the per-board/global resolution.

    ## BLOCKER 2 — topology-blind test — FIXED
    crates/swissarmyhammer-command-service/tests/integration/builtin_ai_commands_e2e.rs: extracted a `build_ai_commands_host()` helper (each call = a fully independent host with its own ai-commands isolate + cached flag) and added a multi-host regression `ai_streaming_flag_is_independent_per_host`: builds global + per-board hosts, publishes on the per-board bridge, asserts ONLY the per-board isolate's `ai.cancel` flips while the global stays disabled — the global/per-board split the single-host test structurally cannot observe.

    The end-to-end proof that production `ai_set_streaming` actually RESOLVES the window to its per-board host lives in the kanban-app crate (which owns the Tauri window→board routing): new test `kanban_app::plugins::tests::ai_set_streaming_reaches_per_board_host_for_a_board_window` — opens a seeded board, binds a window label to it, calls `publish_ai_streaming(&state, label, ...)`, and asserts the PER-BOARD host's `ai.cancel` flips false→true→false. Added a `seed_board` helper.

    ## NIT — FIXED
    apps/kanban-app/ui/src/components/ai-panel.tsx: dropped the stale "(plus the backend UIState.ai_streaming flag)" clause above the `setAiStatus(status)` effect; the gate is now the ai-commands plugin's cached flag.

    ## TDD red→green proof (kanban-app regression test)
    - RED (seam reverted to global-only publish): `timed out waiting for ai.cancel available.ok == true; last: {"ok":false,"reason":"No AI generation is running"}` — the per-board host never sees the flag (the production bug, reproduced).
    - GREEN (per-board-resolved publish): `test ... ok` (2.1s). Idle assertion passed in both runs (per-board host correctly reports ok:false while idle).
    Diagnostic during development confirmed: `resolved_to_board=true`, `per_board.publish reached=1` (the per-board host has a live event-pump subscriber).

    ## Gates (actual)
    - cargo nextest -p swissarmyhammer-command-service -p swissarmyhammer-ui-state: 312 passed, 0 failed (incl. both e2e tests + the new multi-host regression).
    - cargo nextest -p kanban-app: new regression test PASSES. The 10 other failures are ALL the pre-existing board-init harness failure (`open_board should succeed: "...has no board entity (boards/board.yaml)"` from the un-seeded `open_temp_board` helper, which I did NOT touch) + ai_panel_e2e llama tests actually passed this run. Confirmed identical root cause across all 10 — unrelated to streaming.
    - cd apps/kanban-app/ui && npx tsc --noEmit: clean (exit 0).
    - vitest app-shell.ai-commands + ai/commands + ai-panel.spatial + no-direct-invoke: 39 passed.
    - cargo clippy --no-deps -p swissarmyhammer-ui-state -p swissarmyhammer-command-service -p kanban-app -- -D warnings: clean. (`--tests` surfaces only the PRE-EXISTING nits in untouched caption.rs / builtin_nav_commands_e2e.rs / no_stale_imports.rs — none in builtin_ai_commands_e2e.rs.)
    - cargo fmt --check: clean.

    Used an isolated CARGO_TARGET_DIR under target/ (gitignored), cleaned up after.

    NOTE: the adversarial double-check agent reviewed the change and refined the e2e test's name/doc comments (ai_streaming_publish_is_isolated_per_host → ai_streaming_flag_is_independent_per_host, sharper doc on what the single-host topology can/can't observe) — a benign improvement; re-verified green after.
  timestamp: 2026-06-22T21:26:16.693283+00:00
depends_on:
- 01KT9FY7SBW0MVVAZ4A1WZP4SS
- 01KT9FYTVE2CMAGZQW29G1M6Q6
- 01KT9FZ8GZWSPJTK04NEGC0WXQ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffde80
project: command-cutover
title: Gate ai.cancel in the palette via event-driven cached availability (needs SDK subscribe API)
---
DISCOVERED reviewing 01KT6WWYYWFQ2F4PGQ358SAHY7 (ai.yaml → ai-commands plugin migration).

The `ai.cancel` ("Stop AI Generation") command registered by `builtin/plugins/ai-commands/index.ts` carries NO `available` callback, so the registry-driven palette (`useCommandList`/`useCommandAvailability` → backend `available command`) shows it as ENABLED even when no AI generation is in flight.

## Why not fixed now
- The command-service contracts `available` as SYNCHRONOUS (`ideas/plugins/command-service.md`: "The service contracts `available` as synchronous, returning `boolean | { ok: false, reason: string }`").
- The streaming flag (`status === "streaming"`) lives webview-side in `apps/kanban-app/ui/src/ai/commands.ts`'s module bus. The plugin isolate has NO synchronous handle to it, and `CommandContext` (`scope_chain` / `target` / `args`) carries no streaming flag.
- The correct fix is the event-driven cached-flag pattern (command-service.md: "the plugin subscribes to whatever changes the precondition, maintains a cached flag, returns it synchronously"). That needs the SDK event/subscription API (`on`/`subscribe`), which is currently INERT/RESERVED — `crates/swissarmyhammer-plugin/src/sdk/plugin.ts`'s `reservedHandler()` returns a no-op ("event API not implemented in this SDK task").

## Current state (acceptable interim — updated 2026-06-11 after Card I, 01KTED9JYGWM815K2X41N4QDBY)
- Card I deleted `app-shell.tsx`'s `buildAiCommands(streaming)` and its `available: streaming` CommandDef gate — there is no ai.* `CommandDef` in any React scope anymore. The five `ai.*` ids are DEFINED solely by the `ai-commands` plugin registration and EXECUTED through webview command-bus handlers that `AppShell` registers (`useAiCommandBusHandlers` in `apps/kanban-app/ui/src/components/app-shell.tsx`).
- The authoritative frontend gate is now DISPATCH-TIME: the `ai.cancel` bus handler reads `aiStreaming()` (`apps/kanban-app/ui/src/ai/commands.ts`) when dispatched and no-ops while idle — behaviorally equivalent to the old `available: false` (keybinding and palette dispatch both funnel through it). Covered by `app-shell.ai-commands.test.tsx` (idle no-op / streaming cancels / gate re-closes).
- What is still MISSING is unchanged: the *registry* palette listing has no availability metadata, so "Stop AI Generation" renders enabled while idle (dispatching it is a safe no-op).

## Work (once the SDK subscribe API lands)
- In `ai-commands` plugin: subscribe to the streaming-status change, cache the flag, and add a synchronous `available: () => cachedStreaming || { ok: false, reason: "No AI generation is running" }` to the `ai.cancel` registration.
- Wire the webview streaming status to the plugin via the new event surface (replacing the now-dead `ai_set_streaming` Tauri command + `UIState.ai_streaming` plumbing in `apps/kanban-app/src/ai/models.rs` / `crates/swissarmyhammer-ui-state/src/state.rs`, which no backend `available` reads anymore).

## Acceptance
- Registry-driven palette ("Stop AI Generation") is disabled/hidden when idle, enabled mid-stream, matching the frontend dispatch-time gate in `useAiCommandBusHandlers`.
- Depends on: SDK event/subscription API (`on`/`subscribe`) being implemented (currently RESERVED no-op).

## Review Findings (2026-06-22 16:05)

### Blockers
- [x] `apps/kanban-app/src/ai/models.rs` `ai_set_streaming` — the streaming publish never reaches the plugin instance that gates the palette for a board window. `ai_set_streaming` publishes the `ai_streaming` notification to the GLOBAL host's bridge only (`state.plugin_platform.lock().await.host().notification_bridge().publish(...)`). But the production palette gates `ai.cancel` by invoking the `available command` op through `command_tool_call` (`apps/kanban-app/src/commands.rs`), which routes to the PER-BOARD host whenever the calling window has a board open: `match board.as_ref().and_then(|h| h.platform()) { Some(per_board) => per_board.lock().await.host()..., None => state.plugin_platform.lock().await.host()... }`. Each per-board host loads its OWN `ai-commands` isolate with its own module-level `cachedStreaming` and a subscription bound to the per-board bridge. The AI panel mounts inside `BoardContainer`, so the dispatching window is a board window. Net: the global publish updates the global isolate's flag, but the per-board isolate that actually answers the palette never sees it — `cachedStreaming` stays `false` and "Stop AI Generation" renders permanently disabled ("No AI generation is running") even mid-stream. This is the production common case, not an edge case, and it defeats the card's whole purpose. The same global/per-board split also affects `try_dispatch_via_command_service` (dispatch-time path) for a board window. Fix: publish to the bridge of the host that answers the palette for the streaming board's window — resolve the board/window the same way `command_tool_call` does and publish to that per-board host's `NotificationBridge` (falling back to global for a boardless window), or publish to ALL hosts (global + every per-board). Then add a regression test that builds a global host PLUS a per-board host, loads `ai-commands` into both, publishes via the production path, and asserts the PER-BOARD host's `ai.cancel` availability flips — i.e. a test that fails against the current code.
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_ai_commands_e2e.rs` — the e2e test is topology-blind: it constructs a SINGLE `PluginHost` and publishes to that same host's bridge, reproducing exactly the single-host topology the production code does NOT have. It passes (verified: 1 passed) yet cannot exercise the global-publish / per-board-subscribe split that exists in production, so a green run here is not evidence the production wiring reaches the subscriber. Add the multi-host regression described above so the test models the real global+per-board topology.

### Nits
- [x] `apps/kanban-app/ui/src/components/ai-panel.tsx` — stale comment above the `setAiStatus(status)` effect still references the removed backend flag: "`ai.cancel`'s `available` gate (plus the backend `UIState.ai_streaming` flag) tracks the streaming arm". The `UIState.ai_streaming` flag was removed by this diff (the `commands.ts` and `ai-panel-container.tsx` comments were updated, this one was missed). Drop the "(plus the backend `UIState.ai_streaming` flag)" clause; the gate is now the `ai-commands` plugin's cached flag.

### Verified clean (for the record — no action needed)
- [x] Production caller chain for the START/STOP transition is real: `AiPanelContainerBody` (`apps/kanban-app/ui/src/components/ai-panel-container.tsx`) mounts unconditionally in the board window and its `useEffect([streaming])` calls `invoke("ai_set_streaming", { streaming })` on both the `true` and `false` transitions, driven by `useSyncExternalStore(subscribeAiStreaming, aiStreaming)` over `conversation.ts`'s `status` ("streaming" → idle/error). The Tauri command is registered at `apps/kanban-app/src/main.rs`. (The publish FIRES; it just lands on the wrong host — see blocker 1.)
- [x] Wire method `notifications/ui_state/ai_streaming` + event `aiStreaming` are single-sourced (the one `#[notification(...)]` in `crates/swissarmyhammer-ui-state/src/operations.rs`); no parallel/duplicate mechanism.
- [x] `available` semantics: `ai.cancel` returns an EXPLICIT `{ ok: false, reason: "No AI generation is running" }` when idle; `cachedStreaming` defaults `false`. Idle truly disables.
- [x] Dead-code removal of `UIState.ai_streaming` field/getter/setter/3 tests is safe: zero remaining readers; `swissarmyhammer-ui-state` tests pass 22/0 after removal.