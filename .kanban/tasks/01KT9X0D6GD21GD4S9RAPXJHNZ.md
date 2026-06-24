---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvw266s20ratjq3b2adyjcfj
  text: |-
    Picked up. Research done. Stale refs corrected vs the 2026-06-04 description:
    - Direct emitters now live in apps/kanban-app/src/commands.rs handle_drag_start/handle_drag_cancel/handle_drag_complete (emit drag-session-active/cancelled/completed), invoked from handle_drag_events in apply_post_command_side_effects.
    - Drag state machine: crates/swissarmyhammer-ui-state/src/state.rs start_drag/cancel_drag/take_drag + DragSession/DragSource. Operations DragStart/DragCancel/DragComplete in operations.rs.
    - Frontend listeners: apps/kanban-app/ui/src/lib/drag-session-context.tsx useDragSessionEvents (listen drag-session-active/cancelled/completed).

    Event names chosen (mirroring the third-instance convention; declaring service is ui_state, same as the ai_streaming twin): notifications/ui_state/drag_started, /drag_cancelled, /drag_completed. Plugins subscribe this.ui_state.on("drag_started"/"drag_cancelled"/"drag_completed"). Payloads carry exactly what the direct events carry: started = full DragStart session wire shape; cancelled = {session_id}; completed = {session_id, success}. Bridge publish per-window via the existing resolve_window_bridge sink (like publish_ui_state_changed_to). Frontend swaps to subscribe* bridge listeners in mcp-notifications.ts; old drag-session-* added to no-tauri-change-listeners guard.
  timestamp: 2026-06-24T05:38:30.818141+00:00
- actor: claude-code
  id: 01kvwnrc6s66t6wx4jrfrskfyd
  text: 'Resumed. The full implementation was already present in the working tree (uncommitted) from a prior session: Rust ui-state crate (DragStarted/DragCancelled/DragCompleted #[notification] structs + 3 publish helpers in operations.rs, re-exported from lib.rs), kanban-app commands.rs (handle_drag_start/cancel/complete publish on the bridge via publish_drag_to_all_windows; no direct Tauri drag-session-* emits remain in Rust src), frontend (drag-session-context.tsx + mcp-notifications.ts subscribeDrag* seams), coverage guard (drag_emitted_methods_are_declared) + real-pipeline test (drag_transitions_publish_on_the_bridge) in operations.rs, meta_snapshot integration test, no-tauri-change-listeners guard lists all 3 legacy names, mcp-notifications.node.test.ts. Now running full verification before driving to review.'
  timestamp: 2026-06-24T11:20:29.145944+00:00
- actor: claude-code
  id: 01kvwpd46ezrnhee815p92swp4
  text: |-
    Verification complete + adversarial sign-off PASS.

    really-done evidence (all fresh, green):
    - cargo nextest run -p swissarmyhammer-ui-state: 161 passed, 0 skipped
    - cargo nextest run -p kanban-app -E 'not test(ai_panel_e2e)': 188 passed, 3 skipped (ai_panel_e2e GPU)
    - cargo fmt: applied clean
    - cargo clippy -p swissarmyhammer-ui-state --tests: 0 warnings. kanban-app touched file (commands.rs) clean; the lone kanban-app warning is menu.rs:1388 (pre-existing doc_lazy_continuation, not drag-related, 0 net additions from me).
    - npx tsc --noEmit: exit 0
    - vitest (3 affected files: mcp-notifications.node.test.ts, no-tauri-change-listeners.node.test.ts, drag-session-context.test.tsx): 21 passed

    double-check adversarial review: first pass REVISE (Finding 1: real-pipeline test decoupled mutation from notification — built drag_started from a json! literal restating the local session var, so start_drag could be a no-op and still pass). Fixed: drag_transitions_publish_on_the_bridge now reads the session back via ui_state.drag_session().expect(...) and builds the wire payload from the read-back `stored` session (session_id/source_board_path/source_window_label/task_id/copy_mode/started_at_ms/from); cancelled uses stored.session_id. Verified RED->GREEN myself: stubbing start_drag to a no-op makes the test FAIL at "start_drag stored the session"; restored => pass (the first GREEN run was the known stale-rlib gotcha; touch+rerun showed the real RED). Also hardened drag_started_notification to tracing::warn! on a malformed-session deserialization instead of silently publishing an empty payload. Re-run double-check: PASS.

    Final event names: notifications/ui_state/drag_started | drag_cancelled | drag_completed (plugins subscribe this.ui_state.on("drag_started"|"drag_cancelled"|"drag_completed")). No direct Tauri drag-session-* emit remains in Rust src; all three legacy names in the no-tauri-change-listeners guard. Moving to review.
  timestamp: 2026-06-24T11:31:49.070773+00:00
- actor: wballard
  id: 01kvwqx99cwzh2yqtw5gx92w13
  text: |-
    Review resolution — reviewer certified the correctness contract SOLID (all 3 transitions publish per-window on real state-machine transitions; declared==raised for all three via from_declared with a real coverage guard; frontend fully swapped + all 3 legacy names in the forbidden-listener guard; genuine real-pipeline test with RED→GREEN read-back coupling; conventions honored). 0 blockers. The engine's 5 "missing doc comment" nits were FALSE (misattributed — structs have full docs).

    Handled the in-scope warnings:
    - FIXED: drag-session-context.test.tsx restated the DRAG_*_EVENT literals → now imports DRAG_STARTED_EVENT/DRAG_CANCELLED_EVENT/DRAG_COMPLETED_EVENT from mcp-notifications.ts (single source of truth).
    - FIXED (real flake found while verifying): the test was intermittently failing (~1/4 runs) — root cause: the provider subscribed via the `subscribeDrag*` seams, which resolve `listen` through a dynamic `import("@tauri-apps/api/event")` (a macrotask hop that registers the handler a tick late and races the emitted event in the chromium harness). Switched `useDragSessionEvents` to the statically-imported `listen()` (synchronous registration), mirroring the exact fix the focus twin ^5hacd6y applied; `subscribeDrag*` stays the public plugin/MCP-client seam. Verified 8/8 consecutive green runs (was flaky before).
    - WAIVED: `catch (err)`/`catch (e)` → `(error)` — mirrors the existing sibling subscribers; a codebase-wide style sweep, out of scope.
    - DEFERRED (optional polish, non-blocking, mirror established patterns): handle_drag_start/cancel near-duplication; test.each parameterization of the 3 near-verbatim subscribeDrag tests.

    Verified: `npx tsc --noEmit` clean; drag-session-context.test.tsx 16/16 (×8 green); mcp-notifications.node + no-tauri-change-listeners.node 5/5. Rust unchanged since implement verification (ui-state 161, kanban-app 188). Moving to done.
  timestamp: 2026-06-24T11:58:07.148147+00:00
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe380
project: command-events
title: Route drag lifecycle onto the bridge (drag started/cancelled/completed)
---
Make the cross-window drag lifecycle observable on the bridge.

Current state: the app emits direct Tauri `drag-session-active` (commands.rs:1724), `drag-session-cancelled` (commands.rs:1733), `drag-session-completed` (commands.rs:1773) from the DragStart/DragCancel/DragComplete result envelopes. The drag state machine lives in the ui-state service. No bridge notifications exist.

## Work
- Add `notifications/drag/started|cancelled|completed` (NEW methods) and publish on the bridge when the drag state machine transitions.
- Swap the frontend `drag-session-*` listeners to the bridge-forwarded methods; remove the direct emits once swapped.
- Declare on the ui-state service tool (the drag state machine owner) via #[notification] struct=payload.
- Coverage guard.

## Acceptance
A plugin can subscribe to drag lifecycle via `this.ui_state.on("drag.started"/...)` (final event names TBD by the implementer); frontend reads drag from the bridge; no direct `drag-session-*` emit remains; declared == published.

## Review Findings (2026-06-24 05:32)

### Warnings
- [ ] `apps/kanban-app/src/commands.rs` — `handle_drag_start` and `handle_drag_cancel` are near-identical: each extracts a notification via a different builder (`drag_started_note` vs `drag_cancelled_note`), then calls `publish_drag_to_all_windows`. Consider a shared `handle_drag_notification(app, state, result, extractor)` parameterized by the extractor fn. (In-scope, low priority — the two-line bodies are arguably clearer left explicit.)
- [ ] `apps/kanban-app/ui/src/lib/drag-session-context.test.tsx:88` — The literal event-name constants (`DRAG_STARTED`/`DRAG_CANCELLED`/`DRAG_COMPLETED`) are redefined locally, but identical constants are already exported from `mcp-notifications.ts` (`DRAG_*_EVENT`). Import them from the shared module instead of restating the literals. (In-scope.)
- [ ] `apps/kanban-app/ui/src/lib/drag-session-context.tsx` — Catch clauses use abbreviated `catch (e)`; project convention is `catch (error)`. Affects the drag-session hooks. (In-scope.)
- [ ] `apps/kanban-app/ui/src/lib/mcp-notifications.node.test.ts:77` — The three `subscribeDrag* targets the bridge event…` tests are near-verbatim copies differing only in subscribe fn, event constant, and legacy string. Parameterize with `test.each`/`describe.each` over `[(subscribeDragStarted, DRAG_STARTED_EVENT, "drag-session-active"), …]`. (In-scope.)
- [ ] `apps/kanban-app/ui/src/lib/mcp-notifications.ts` — Catch clauses use abbreviated `catch (err)`; project convention is `catch (error)`. (In-scope — touches the new `subscribeDrag*` seams region.)

### Nits
- [ ] `crates/swissarmyhammer-ui-state/src/operations.rs` — Engine flagged `DragStarted`/`DragCancelled`/`DragCompleted` structs and `drag_started_notification`/`drag_cancelled_notification` as lacking struct/fn doc comments. Reviewer verification: FALSE — all five have full doc comments in the working tree; the engine's line refs point at unrelated `ShowPalette`/`ShowSearch`/`Dismiss` (also documented). Treat as already-satisfied / misattributed; no action needed.

### Reviewer notes (correctness contract — all confirmed against the diff)
- [x] All three transitions (start/cancel/complete) publish `notifications/ui_state/drag_*` per-window via `publish_drag_to_all_windows` (dedups by distinct bridge, mirrors `publish_ui_state_changed_to`), gated on the real `DragStart`/`DragCancel`/`DragComplete` envelope — fires on the real transition, not spuriously.
- [x] Declared == raised: helpers use `from_declared(payload.method(), …)` so the wire method derives from `#[notification]`; coverage guard `drag_emitted_methods_are_declared` asserts all three published methods appear in `_meta`.
- [x] Frontend swap complete: `drag-session-context.tsx` consumes the `subscribeDrag*` bridge seams; no `listen("drag-session-*")` in Rust or TS production src; no `.emit("drag-session-*")` in Rust. The forbidden-listener guard (`no-tauri-change-listeners.node.test.ts`) lists all three legacy names in `FORBIDDEN_EVENTS`, compiled into the scanned regex (real enforcement).
- [x] Real-pipeline test `drag_transitions_publish_on_the_bridge` is genuine: `start_drag` → read back via `drag_session()` (None if it were a no-op) → `DragStarted` payload → real `NotificationBridge` → live subscriber; cancel path coupled via `cancel_drag()` + `is_none()`. Implementer's RED→GREEN claim verified plausible (read-back coupling makes a no-op mutation fail).
- [x] Conventions: thin payloads (session_id / {session_id} / {session_id, success}), `tracing::warn!` on malformed-session deserialization (not eprintln!), single-source `from_declared`. No internal jargon in plugin-facing strings.

Pre-existing / out-of-scope (disregarded): `kanban-app::ai_panel_e2e` (GPU), `file_notes_e2e`/`example_layering_e2e` (CWD isolation), `menu.rs:1388` clippy doc_lazy_continuation.