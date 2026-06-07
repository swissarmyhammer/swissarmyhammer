---
assignees:
- claude-code
position_column: review
position_ordinal: '8380'
project: ui-command-cleanup
title: Card F1 — Host→UI request/reply channel (correlation-id, lock-safe)
---
## What
Add a HOST→UI request/reply mechanism so host-side code (plugins, services) can ASK the webview a question and AWAIT the answer. Today host→UI is fire-and-forget only (`apps/kanban-app/src/commands.rs::spawn_window_forwarder` ~line 2725 and `command_services.rs::TauriFocusEventSink::emit` use `emit_to`); the host registers NO listener for UI-emitted events, so there is no reply path. This card builds the generic primitive that Card F2 (focus geometry queries) rides on.

Design (from exploration):
- Host side (`apps/kanban-app/src/commands.rs` or a new `ui_request.rs`): a registry `Mutex<HashMap<RequestId, oneshot::Sender<serde_json::Value>>>`. An async API `request_from_ui(window_label, kind, params) -> Result<Value>` that: generates a RequestId, inserts a oneshot sender, `emit_to(window_label, "ui/request", {request_id, kind, params})`, then awaits the receiver with a TIMEOUT.
- New `#[tauri::command] ui_request_reply({ request_id, result })` (registered in the generate_handler! list): looks up the sender by id and fires it.
- UI side (e.g. `apps/kanban-app/ui/src/lib/ui-request-responder.ts`): a `listen("ui/request", …)` handler + a registry mapping `kind -> responder fn`; on a request it calls the responder, then `invoke("ui_request_reply", { request_id, result })`. Provide `registerUiResponder(kind, fn)`.
- CRITICAL deadlock discipline: the host MUST NOT hold the `AppState` / spatial `Mutex`es across the `.await`. Build the request, DROP all locks, await the reply, then re-acquire. Document and test this.
- Include a timeout + error if the UI never replies (window closed, no responder).

## Note (2026-06-06)
F1 itself looks correct in the working tree (`apps/kanban-app/src/ui_request.rs`). The downstream nav rewrite that rides on F1 broke arrow-key navigation — but the root cause is the **focus source-of-truth** seam in F2, NOT this channel. See regression card `01KTESYQ49JYJB2YT1WXYKK0W4` (F1 is fine; flagged here only for traceability).

## Acceptance Criteria
- [ ] Host code can issue a typed request to a specific window and receive the UI responder's reply value, correlated by request id.
- [ ] Concurrent in-flight requests correlate independently (no cross-talk).
- [ ] A request times out cleanly if no reply arrives; returns an error, no leaked sender.
- [ ] No `AppState`/spatial lock is held across the await (deadlock-safe) — asserted by test or a documented lock-drop seam.

## Tests
- [ ] Rust integration/unit test: drive `request_from_ui` and simulate the reply by invoking the `ui_request_reply` handler with the matching request_id; assert the awaited value matches. Add a second concurrent request to prove id correlation, and a timeout case.
- [ ] (If feasible) a test asserting locks are released before the await (e.g. a second command can proceed while a request is in flight).
- [ ] Tests fail before the channel exists (RED), pass after.

## Workflow
- Use `/tdd` — failing test first. Automated tests only.