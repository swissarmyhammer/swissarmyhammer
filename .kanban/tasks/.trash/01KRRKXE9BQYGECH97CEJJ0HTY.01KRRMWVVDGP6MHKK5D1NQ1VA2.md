---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8780'
project: ai-panel
title: Permission handling + AI session Tauri command surface
---
## What
Handle ACP permission requests and expose the AI session to the webview as Tauri commands.

- Permission: the ACP `session/request_permission` handler (registered in `ai/client.rs`) forwards the request as a `ai://permission/{window_label}` Tauri event `{ requestId, toolName, args }`, then awaits the user's decision and returns it as the handler's `RequestPermissionResponse` return value (the reply IS the return value — not a separate message).
- Permission policy: an `always-ask` / `auto-approve-reads` / `auto-approve-all` setting; `auto-approve-reads` answers `kanban` read ops without a prompt.
- Tauri commands in `apps/kanban-app/src/commands.rs`, registered in `apps/kanban-app/src/main.rs` `generate_handler!`:
  - `ai_start_session(window_label, model_id) -> { sessionId }`
  - `ai_send_prompt(window_label, text) -> ()` (streams via events)
  - `ai_cancel_prompt(window_label) -> ()` — ACP `cancel` notification + `CancellationToken`
  - `ai_respond_permission(window_label, request_id, decision) -> ()`
  - `ai_close_session(window_label) -> ()`
- Teardown: drop the window's `AiSession` on window close and on board close (hook the existing window/board close paths).

Spec: `ideas/kanban/ai_panel.md` — Phase 2 "Permission requests", "Cancellation & teardown", Wire Protocol.

## Acceptance Criteria
- [ ] `session/request_permission` round-trips: agent request -> `ai://permission` event -> `ai_respond_permission` -> handler returns the decision.
- [ ] `auto-approve-reads` policy answers `kanban` read ops without emitting a permission event.
- [ ] All five `ai_*` commands are registered and functional; `ai_cancel_prompt` issues the ACP `cancel`.
- [ ] Closing a window or board drops that window's `AiSession`.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Integration test: agent triggers `session/request_permission`; assert the `ai://permission` event fires and that `ai_respond_permission` resolves the handler with the decision.
- [ ] Test the permission policy: `auto-approve-reads` resolves a read op with no event.
- [ ] Test `ai_cancel_prompt` sends the ACP `cancel` and the prompt ends.
- [ ] Test session teardown on window close.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the permission round-trip test first.