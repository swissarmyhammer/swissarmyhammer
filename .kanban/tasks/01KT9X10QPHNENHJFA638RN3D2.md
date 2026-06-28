---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvwtrhgbrkkdvag3rce7gvpe
  text: |-
    Picked up; moved to doing. Studied sibling commit 4fc64bab (board lifecycle on same window-service).

    NAMESPACE DECISION: option (b) — mirror the board family exactly with two-segment short events window.created / window.focused / window.closed (full methods notifications/window/created|focused|closed). This is what the committed board code's own comment anticipated ("sibling raw OS-window lifecycle (window.created/window.focused/window.closed)"). The _meta tree is flat-keyed by SHORT event with last-insert-wins, so bare `closed` would collide with board.closed; window.closed vs board.closed are distinct. Chose (b) over the bare `created/focused/closed` from the acceptance text BECAUSE the bare form would be safe today but the explicit form matches the existing board sibling's pattern and keeps both families self-documenting in _meta. Coverage guards assert ALL SIX leaves survive distinct.

    GEOMETRY: SKIPPED (no moved/resized events). Move/Resize fire continuously during a drag — far too chatty for a lifecycle signal, and no plugin consumer needs per-pixel motion. Documented in the WindowCreated module comment in operations.rs.

    window-service layer done: added WindowCreated/WindowFocused/WindowClosed #[notification] structs (payload = {label, board_path: Option<String>}), window_created/focused/closed_notification publish helpers, added to WINDOW_NOTIFICATIONS slice, re-exported from lib.rs. Tests: window_notifications_declare_method_and_event, window_notifications_build_label_and_board_path_payload, extended coverage guard published_lifecycle_methods_are_declared (all 6 families), NEW meta_keeps_board_and_window_families_distinct collision guard, + meta_snapshot integration test window_tool_meta_advertises_window_lifecycle_notifications (production list_tools path, asserts all 6 leaves). All 6 unit notification tests GREEN.
  timestamp: 2026-06-24T12:47:57.451543+00:00
- actor: claude-code
  id: 01kvwvh0nt91jep7f8emnsk4tq
  text: |-
    All publish sites wired + adversarial review applied.

    Publish sites (3): created → commands.rs create_window_impl (single creation path: user-new AND startup-restore both route here; window→board mapping is set just before, so resolve_window_bridge resolves the new window's board bridge). focused → main.rs on_window_focused. closed → main.rs on_window_close_requested. focused/closed run from synchronous OS WindowEvent handlers, so they spawn the async publish (clone AppHandle, re-resolve AppState).

    DOUBLE-CHECK found + FIXED a real High bug: the close handler originally spawned the publish then SYNCHRONOUSLY aborted the per-window forwarder via unbind_window_forwarder — the abort wins the race and window.closed is dropped (the board sibling avoided this by publishing inline/awaited before teardown). Fix: on_window_close_requested now captures board_path, removes the UiState entry synchronously (anti-resurrection), then runs ONE spawned task that does publish_window_lifecycle(...).await THEN unbind_window_forwarder — awaited-publish-strictly-before-teardown. Added regression test state.rs::window_close_publishes_before_forwarder_teardown mirroring that ordering (subscriber live during publish receives it despite the following unbind). Re-ran double-check: PASS.

    Helper rename: publish_board_lifecycle → publish_window_lifecycle (generalized, both board.* and window.* families route through it — it's just resolve_window_bridge + publish; updated the one menu.rs caller).

    FRONTEND: SKIPPED — grep found zero existing UI window-lifecycle listeners; primary consumer is plugins (which subscribe to the bridge, not the webview), so the UI needs nothing.

    Verification (all fresh, green): cargo nextest -p swissarmyhammer-window-service 41/41; -p kanban-app (excl ai_panel_e2e) 231/231 incl both new state.rs tests; cargo fmt clean; clippy on touched files (operations.rs, lib.rs, main.rs, commands.rs, menu.rs, state.rs) clean — the 4 remaining workspace clippy warnings are all pre-existing in untouched code (shell.rs commit 443eba291, swissarmyhammer-focus, menu.rs:1406 commit 884739f10).
  timestamp: 2026-06-24T13:01:19.418757+00:00
- actor: wballard
  id: 01kvwwdczebb5z8z2zegz4wsvb
  text: |-
    Review resolution — reviewer certified all 5 load-bearing items PASS (namespace collision-free via real guard `meta_keeps_board_and_window_families_distinct` asserting all 6 distinct leaves; close-ordering correct — `on_window_close_requested` awaits `publish_window_lifecycle(...)` strictly before `unbind_window_forwarder`, and `created`/`focused` have no analogous teardown race; declared==raised; publish sites correct; conventions honored incl. documented geometry-skip + frontend-skip).

    In-scope BLOCKER FIXED: the three new `window_created/focused/closed_notification` helpers (identical shape: `label: impl Into<String>, board_path: Option<String>` → struct → `from_declared`) are now generated by a single `window_lifecycle_helper!` macro instead of three verbatim copies. The `board_*` helpers stay explicit (different signature — path-only, committed in 4fc64bab; out of this task's scope). Re-verified: window-service 41/41, kanban-app compiles clean, fmt clean, no clippy warnings in operations.rs.

    Out-of-scope/pre-existing (reviewer-classified, not in this diff): main.rs `"quick-capture"` literal (warning); state.rs:3049/3050/3208/3212 drag-session timeouts / MCP-probe sleeps (4 nits) — pre-existing code below this task's additive test hunk.

    Noted (reviewer caveat, not a filed finding): the `window_close_publishes_before_forwarder_teardown` regression test proves "awaited publish reaches a live subscriber" but isn't a strict red-on-swap guard of the abort-wins-the-race mode (unbind on an unbound label is a no-op + the test uses a bare bridge.subscribe(), no real forwarder). The production fix itself is correct (double-check PASS); a hard red-on-swap guard would need a real forwarder in the test — left as a possible future hardening. Moving to done.
  timestamp: 2026-06-24T13:16:49.518154+00:00
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe580
project: command-events
title: Raise window lifecycle as bridge events (window created/focused/closed) — NEW events
---
Make OS window lifecycle observable. This is the user's "window change" question — today these are SILENT (no event on any channel), so this card CREATES the events, it doesn't migrate them.

Current state: the OS window-event handler `handle_window_event` (apps/kanban-app/src/main.rs:440-514) only mutates in-memory state — `Focused(true)` → on_window_focused (:482), Moved/Resized → on_window_geometry_changed (:460), CloseRequested → on_window_close_requested (:493), Destroyed → on_window_destroyed (:508). Window creation (crates/swissarmyhammer-window-service/src/shell.rs:446 open_new_window) returns a value and emits nothing.

## Work
- Add `notifications/window/created|focused|closed` (NEW methods; geometry moved/resized optional — likely too chatty, decide) and publish on the bridge from the window-event handler + window-creation path. Payload: window label, board_path.
- Frontend: add listeners if the UI needs them (optional — primary consumer is plugins).
- Declare on the window service tool via #[notification] struct=payload.
- Coverage guard.

## Sibling card (keep separate — decided 2026-06-06)
**Board** lifecycle (opened/switched/closed) is the related-but-distinct event family — `01KT9X0SB17R3TRKT419A01TM7`. Kept separate on purpose: window lifecycle = raw OS window events; board lifecycle = board-file ↔ window association. Coordinate the window-service event-name namespace so `created/focused/closed` and `board.opened/...` don't collide and share one declaration pattern.

## Acceptance
A plugin can `this.window.on("created"/"focused"/"closed", cb)` and observe window lifecycle (silent today); declared == published. Decide whether geometry changes are worth emitting (probably not — too frequent).

## Review Findings (2026-06-24 07:02)

Scope: working tree vs HEAD (`4fc64bab`). Reviewed this task's diff only. Load-bearing items all PASS: (1) namespace — `created` published inline-awaited in `create_window_impl`, collision guard `meta_keeps_board_and_window_families_distinct` genuinely asserts all 6 distinct `board.*`/`window.*` leaves with correct methods via production `generate_notifications_meta`; (2) close-ordering — `publish_window_lifecycle(...).await` runs strictly before `unbind_window_forwarder` on one spawned task, no spawn-then-sync-abort race; `created`/`focused` have no analogous drop race; regression test `window_close_publishes_before_forwarder_teardown` mirrors production ordering with a live subscriber; (3) declared==raised across all 3 new methods via production-path tests; (4) publish sites correct (created@create_window_impl, focused@on_window_focused, closed@on_window_close_requested) with correct board_path resolution; (5) conventions — thin events, tracing not eprintln!, geometry-skip + frontend-skip documented.

### Blockers (in-scope)
- [ ] `crates/swissarmyhammer-window-service/src/operations.rs:371` — Three window notification helper functions (window_created_notification, window_focused_notification, window_closed_notification) are near-verbatim duplicates, differing only in the struct type being instantiated. The function bodies are identical: instantiate the struct, then call McpNotification::from_declared with identical arguments. This creates two maintenance burdens: (1) a fix to the pattern requires touching three locations; (2) the three functions will diverge if one is updated and the others are not. Create a generic helper function or macro that generates all three, parameterized by the struct type. For example: a macro `window_notification!(Created, Focused, Closed)` that generates each function, or a single generic function using a trait bound if Rust's type system permits.

### Warnings (out-of-scope / pre-existing — NOT in this task's diff)
- [ ] `apps/kanban-app/src/main.rs:157` — Hardcoded repeated `"quick-capture"` window-label literal. PRE-EXISTING: this task's main.rs diff does not touch these occurrences (grep of the HEAD diff finds no `quick-capture` additions). Disregard for this card.

### Nits (out-of-scope / pre-existing — NOT in this task's diff)
- [ ] `apps/kanban-app/src/state.rs:3049` — `31_000` drag-session-timeout test literal. PRE-EXISTING: this task adds only one additive hunk at state.rs:2812 (the two new window-lifecycle tests); these lines are unrelated drag-session code below the hunk. Disregard.
- [ ] `apps/kanban-app/src/state.rs:3050` — `30_000` drag-session-timeout literal. PRE-EXISTING (same as above). Disregard.
- [ ] `apps/kanban-app/src/state.rs:3208` — `500`ms async-shutdown grace sleep. PRE-EXISTING MCP-probe test code, not in this task's diff. Disregard.
- [ ] `apps/kanban-app/src/state.rs:3212` — `2`s MCP-server-probe HTTP timeout. PRE-EXISTING, not in this task's diff. Disregard.