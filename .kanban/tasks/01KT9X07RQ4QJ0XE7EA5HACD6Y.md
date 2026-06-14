---
assignees:
- claude-code
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: todo
position_ordinal: bb80
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