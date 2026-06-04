---
assignees:
- claude-code
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: todo
position_ordinal: be80
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

## Acceptance
A plugin can `this.window.on("created"/"focused"/"closed", cb)` and observe window lifecycle (silent today); declared == published. Decide whether geometry changes are worth emitting (probably not — too frequent).