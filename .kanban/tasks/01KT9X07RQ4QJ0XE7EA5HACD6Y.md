---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvvwx4sf67nvw0kkypx1anz3
  text: |-
    Picked up. Studied reference commit ead83ad8 (ui-state twin ^rxrfswf) — captured the canonical pattern: kind()/payload single source of truth, #[notification] struct==payload via from_declared, publish helper in the declaring crate, per-window bridge publish from the kanban-app adapter, frontend swap to a subscribe* bridge listener, forbidden-listener guard, declared==raised coverage guard, real-pipeline test (real mutation → real NotificationBridge → live subscriber).

    Corrected STALE refs from the 2026-06-04 task description:
    - TauriFocusEventSink lives in apps/kanban-app/src/command_services.rs (struct ~line 78, attached ~line 291 via FocusServer::new().with_sink(Arc::new(TauriFocusEventSink))). It synchronously emit_to(window_label,"focus-changed",event).
    - FocusEventSink trait: crates/swissarmyhammer-focus/src/observer.rs (emit(&self,&FocusChangedEvent), synchronous).
    - FocusChangedEvent struct: crates/swissarmyhammer-focus/src/state.rs (~line 58); fields window_label, prev_fq, next_fq, next_segment — these ARE the payload.
    - focus tool build_tool_definition: crates/swissarmyhammer-focus/src/server.rs (~line 178) uses operation_tool! WITHOUT a notifications: field — must add one.
    - PRODUCTION frontend focus-changed listener: apps/kanban-app/ui/src/lib/spatial-focus-context.tsx (~line 380), NOT the path in the task. Type FocusChangedPayload (apps/kanban-app/ui/src/types/spatial.ts) already mirrors FocusChangedEvent fields, so the bridge params deserialize straight into it.
    - Forbidden-listener guard: apps/kanban-app/ui/src/lib/no-tauri-change-listeners.node.test.ts.
    - Bridge→Tauri forwarder (commands.rs spawn_window_forwarder) emits each notification as a Tauri event named by its method, payload=params, emit_to(label) — so a per-window bridge publish reaches only that window's listener.

    Design: the focus sink is SYNCHRONOUS but the bridge is reachable per-window. Plan: a bridge-publishing sink that resolves the window's bridge and publishes notifications/focus/changed. Need to handle the sync→async + per-window bridge resolution.
  timestamp: 2026-06-24T04:06:10.991824+00:00
- actor: claude-code
  id: 01kvvy7pf6vme9f330qg5vdvhr
  text: |-
    Implementation landed. Summary:

    RUST (swissarmyhammer-focus):
    - operations.rs: added #[notification] FocusChanged (method notifications/focus/changed) whose fields ARE the FocusChangedEvent (window_label, prev_fq, next_fq, next_segment). Manual Default impl (the define_id! newtypes don't derive Default). Added FOCUS_NOTIFICATIONS slice + notifications() + focus_changed_notification(&FocusChangedEvent) publish helper (struct==payload via from_declared, user provenance). 5 unit tests incl declared==raised coverage guard.
    - server.rs: operation_tool! now passes notifications: notifications(); updated with_sink doc to describe the bridge.
    - lib.rs: re-export focus_changed_notification, notifications, FocusChanged.
    - tests/integration/meta_snapshot.rs: production-path test focus_tool_meta_advertises_changed_notification (real list_tools _meta advertises changed).
    - tests/integration/focus_server_e2e.rs: REAL-PIPELINE test focus_change_publishes_changed_on_the_bridge — real set focus kernel mutation → FocusEventSink → focus_changed_notification → real NotificationBridge → live subscriber.

    APP (kanban-app):
    - command_services.rs: replaced TauriFocusEventSink (direct app.emit_to(window,"focus-changed",event)) with TauriFocusBridgeSink that publishes notifications/focus/changed onto the originating window's bridge via resolve_window_bridge (sync sink spawns a task; window_label from the event targets the right bridge). Dropped the Emitter import, added Manager + focus_changed_notification.
    - commands.rs: updated the two focus-cutover comments.

    FRONTEND:
    - mcp-notifications.ts: added FOCUS_CHANGED_EVENT constant, FocusChanged payload type, subscribeFocusChanged helper (public seam for plugins/MCP clients).
    - spatial-focus-context.tsx: swapped listen("focus-changed") → listen(FOCUS_CHANGED_EVENT) (the new bridge event). Kept a DIRECT listen (not subscribeFocusChanged) deliberately: the spatial browser-test harness fires focus events immediately on mount and the dynamic-import path in subscribeFocusChanged registers a tick late in chromium → tests miss the event. Direct listen registers synchronously, preserving harness timing. Both target FOCUS_CHANGED_EVENT.
    - no-tauri-change-listeners.node.test.ts: added "focus-changed" to FORBIDDEN_EVENTS guard.
    - Migrated 48 spatial/focus test files + 2 shared harnesses (spatial-shadow-registry.ts, mock-spatial-kernel.ts) + kernel-simulator.ts from the "focus-changed" listener key to "notifications/focus/changed" (shared helpers use the FOCUS_CHANGED_EVENT constant).

    DEAD END logged: subscribeFocusChanged in the provider failed 1 spatial browser test (extra dynamic-import microtask delays listener registration; a 50ms macrotask sleep fixed it but is too slow/brittle for 48 files). Resolved by using a direct synchronous listen in the provider.

    GOTCHA: parallel cargo tauri dev (PID 9580) + sccache produced stale focus rlibs WITHOUT the new export, making nextest -p kanban-app fail E0432 intermittently while cargo check passed. touch focus src + rerun nextest = all 185 kanban-app tests pass.
  timestamp: 2026-06-24T04:29:25.350974+00:00
- actor: claude-code
  id: 01kvvyrgdd63gaj8yhxxw9ca94
  text: |-
    Adversarial double-check ran (advisory gate). Verdict REVISE with one HIGH + one LOW finding — both now fixed:

    HIGH (regression): two spatial test files — focus-scope.test.tsx and focus-scope.scroll-transition.test.tsx — gated their listen mock on `event === "focus-changed"` (an EQUALITY comparison, not the listeners.get(...)/["..."] bracket-access forms my migration grep+perl matched), so they missed the migration → 11 failing chromium tests. Fixed: changed the comparison to "notifications/focus/changed" in both. Both files now pass (32/32). Swept the whole UI tree for any other non-bracket comparison/includes/case forms against "focus-changed" — only these two existed (plus the forbidden-listener guard, which correctly KEEPS the literal as the forbidden name).

    LOW: subscribeFocusChanged in mcp-notifications.ts was dead in production (provider uses direct listen for synchronous registration) and its docstring falsely claimed the provider mounts it. Fixed: corrected the docstring to state it's the public plugin/MCP-client seam (not the provider's path), and added src/lib/mcp-notifications.node.test.ts pinning its contract (registers under FOCUS_CHANGED_EVENT, not the legacy focus-changed; forwards the payload). Passes 1/1.

    Double-check PASSED everything else: coverage guard is real (raised method ∈ declared _meta), real-pipeline test is genuinely end-to-end, struct==payload exact, direct focus-changed emit fully removed, per-window routing correct, no jargon/eprintln/enrichment-refetch. Noted (not a finding): the sync sink spawns to resolve the bridge async → no strict cross-event ordering, same as the existing publish_ai_streaming + legacy fire-and-forget emit_to, not a regression.
  timestamp: 2026-06-24T04:38:36.205692+00:00
- actor: claude-code
  id: 01kvw1raewry9325j6mxgrge1b
  text: |-
    ## Review Findings (2026-06-24 — file-scoped engine pass)

    NOTE ON METHOD: the session/local `review working` fan-out was saturated (15/16 tasks failing) and the incremental hash cache short-circuited whole-tree runs to `attempted:0` "nothing in scope" — NOT a genuine clean. I cleared `.validators/.hashes/` and drove the engine per-file (`review file`, 15 validators each, 0 failed) over this task's load-bearing files. Whole-file findings were classified against the actual `git diff HEAD` (HEAD = ead83ad8).

    ### In-scope (introduced/touched by THIS task) — minor, task stays in review
    - [ ] `apps/kanban-app/ui/src/lib/mcp-notifications.ts` — new `subscribeFocusChanged` catch clause uses the abbreviation `(err)` → prefer `(error)`. NOTE: consistently mirrors the 3 pre-existing twins (subscribeUndoChanged/UiStateChanged also use `err`); a codebase-wide style point, not a new defect.
    - [ ] Stale doc-comments referencing the legacy key. 9 migrated spatial test files still say `mockListen records every listen("focus-changed", cb)` in their header comment blocks (live `listen(...)` calls were correctly migrated to `notifications/focus/changed`; only the explanatory comments drifted). Files: perspective-tab-bar.focus-indicator.browser, nav-bar.spatial-nav, entity-card.spatial, board-view.spatial, focus-indicator.single-variant.spatial, entity-inspector.spatial-nav, nav-bar.focus-indicator.browser, perspective-bar.spatial, perspective-view.spatial. Documentation drift only.

    ### Verified CLEAN (in-scope, no findings)
    - [x] Bridge publish path correct: `TauriFocusBridgeSink::emit` builds the declared `notifications/focus/changed` notification synchronously from the event in hand, resolves the originating window's bridge via `resolve_window_bridge(&state, &window_label)`, publishes — per-window routing correct, no enrichment re-fetch. Engine: 0 findings on the new sink.
    - [x] Real-pipeline test `focus_server_e2e.rs` — engine CLEAN (0/15 findings). Genuine end-to-end (real kernel set focus → sink → focus_changed_notification → real NotificationBridge → live subscriber).
    - [x] Direct `focus-changed` Tauri emit fully removed (`app.emit_to(...,"focus-changed",...)` gone); forbidden-listener guard adds the literal to FORBIDDEN_EVENTS. `mcp-notifications.node.test.ts:50` asserts NOTHING registers under the legacy key.
    - [x] Migration completeness verified: grep for live `"focus-changed"` equality/includes/bracket forms in UI src → NONE FOUND. The two regression files the implementer fixed (focus-scope.test.tsx, focus-scope.scroll-transition.test.tsx) were the last equality-form misses. No mixed old/new live keys remain.
    - [x] Direct `listen(FOCUS_CHANGED_EVENT)` in the provider (vs the subscribeFocusChanged dynamic-import seam) is sound: synchronous registration required by the chromium spatial harness; subscribeFocusChanged remains the public plugin/MCP-client seam with its own contract test. Consistent intent with the ui-state twin.

    ### Out-of-scope / pre-existing whole-file noise (NOT introduced by this task — do not action here)
    - `server.rs`: handle_drill_in/handle_drill_out duplication (blocker), 12-arm dispatch match, `call_tool` length, build_tool_definition nit — all pre-existing; this task's server.rs diff is ONLY doc-comments + adding `notifications: notifications()`.
    - `command_services.rs`: `install_app_command_services` ~57 lines — pre-existing; task swapped ONE constructor line (TauriFocusEventSink→TauriFocusBridgeSink).
    - `spatial-focus-context.tsx`: SpatialFocusProvider 85-line length — pre-existing; task changed one listen() key.
    - `operations.rs`: `WindowLabel::from_string("")` x6 "rule of three" — 5 of 6 pre-existing in proto/test fns; task added only 1 (FocusChanged::default in a new test).
    - `mcp-notifications.ts` / `meta_snapshot.rs` rule-of-three "extract a generic subscriber/helper" blockers — the engine flags the task for adding a consistent 4th subscribe sibling / 2nd meta test; deliberately mirrors the ui-state twin pattern, cross-cutting refactor out of this task's mandate.

    ### Verdict
    In-scope changes are correct and complete: declared==raised, real pipeline genuine, direct emit removed, migration clean. Only two cosmetic in-scope items (err→error consistency, 9 stale doc-comments). No blockers, no behavioral defects attributable to this task.
  timestamp: 2026-06-24T05:30:55.836399+00:00
- actor: wballard
  id: 01kvw1xnhtkxy5479p9dzg38j5
  text: |-
    Review resolution — reviewer certified contract CLEAN on substance (all 5 load-bearing items verified: correct per-window bridge publish via TauriFocusBridgeSink+resolve_window_bridge, declared==raised with real coverage guard, complete migration + forbidden-listener guard, genuine real-pipeline focus_server_e2e test, conventions honored). Two cosmetic in-scope nits handled:
    - Nit 2 (FIXED): updated the stale `listen("focus-changed", cb)` references in JSDoc header comments → `listen("notifications/focus/changed", cb)` across the 9 named spatial test files (comment-only; verified no live `focus-changed` listen() calls remained; `npx tsc --noEmit` clean).
    - Nit 1 (WAIVED with reason): the new `subscribeFocusChanged` `.catch((err))` deliberately matches its 3 sibling subscribers (subscribeUndoChanged/UiStateChanged also use `err`). The reviewer itself flagged this as "consistently mirrors the twins... not a new defect." Changing only the focus one to `(error)` would BREAK local consistency; an `err`→`error` rename is a codebase-wide style sweep, out of scope here.

    Out-of-scope/pre-existing whole-file findings (server.rs handle_drill_in/out duplication, 12-arm dispatch match, call_tool length, SpatialFocusProvider length, WindowLabel::from_string("") rule-of-three, subscribe/meta rule-of-three) are NOT introduced by this task (this diff added only `notifications: notifications()` + doc comments to server.rs, swapped one constructor line in command_services.rs, changed one listen() key in the provider). Deferred. Moving to done.
  timestamp: 2026-06-24T05:33:51.034718+00:00
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe280
project: command-events
title: Route focus changes onto the bridge (focus/changed)
---
Make focus changes observable on the bridge.

Current state: focus changes are pushed straight to the originating window as the direct Tauri `focus-changed` event via `TauriFocusEventSink` (apps/kanban-app/src/command_services.rs:74-91, attached :191), bypassing the bridge. The FocusEventSink (crates/swissarmyhammer-focus/src/observer.rs:33) is a synchronous push, not a broadcast bus. No `notifications/focus/changed` method exists yet.

## Work
- Add a `notifications/focus/changed` notification (NEW method) and publish it on the bridge when focus changes — either by having the focus sink also publish to the bridge, or by adding a bridge-publishing sink alongside/instead of the Tauri one.
- Swap the frontend `focus-changed` listener to the bridge-forwarded `notifications/focus/changed`; remove the direct emit once swapped.
- Declare on the focus service tool (swissarmyhammer-focus/src/server.rs:153) via #[notification] struct=payload (FocusChangedEvent fields).
- Coverage guard.

## Acceptance
A plugin can `this.focus.on("changed", cb)`; the frontend reads focus from the bridge; no direct `focus-changed` emit remains; declared == published.

## Review Findings (2026-06-24)

In-scope changes are correct and complete: bridge publish path correct, declared==raised, real-pipeline test genuine, direct emit removed, migration clean (no live mixed old/new keys remain). No blockers, no behavioral defects attributable to this task. Two cosmetic in-scope items remain open:

- [ ] `apps/kanban-app/ui/src/lib/mcp-notifications.ts` — new `subscribeFocusChanged` catch uses `(err)` → prefer `(error)` (mirrors 3 pre-existing twins; codebase-wide style point).
- [ ] Stale doc-comments: 9 migrated spatial test files still say `listen("focus-changed", cb)` in their header comment blocks (live calls migrated; comments drifted): perspective-tab-bar.focus-indicator.browser, nav-bar.spatial-nav, entity-card.spatial, board-view.spatial, focus-indicator.single-variant.spatial, entity-inspector.spatial-nav, nav-bar.focus-indicator.browser, perspective-bar.spatial, perspective-view.spatial.

Out-of-scope / pre-existing whole-file noise (do NOT action here): server.rs drill_in/drill_out duplication + 12-arm dispatch + call_tool length; install_app_command_services length; SpatialFocusProvider length; operations.rs WindowLabel::from_string("") x6 (5 pre-existing); mcp-notifications.ts/meta_snapshot.rs rule-of-three (task added one consistent sibling mirroring the ui-state twin).

Full classified pass + method note in the 2026-06-24 review comment. Engine fan-out was saturated; drove per-file (`review file`, 15 validators each, 0 failed) after clearing the stale incremental hash cache.