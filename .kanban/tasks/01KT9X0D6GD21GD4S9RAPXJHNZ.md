---
assignees:
- claude-code
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: todo
position_ordinal: bc80
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