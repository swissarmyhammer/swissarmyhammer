---
assignees:
- claude-code
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: todo
position_ordinal: ba80
project: command-events
title: Route UI-state changes onto the bridge (ui_state/changed)
---
Make ephemeral UI-state changes observable on the bridge and consumed by the frontend from the bridge.

Current state: the app emits the direct Tauri `ui-state-changed` event (apps/kanban-app/src/commands.rs:1889 `emit_ui_state_change_if_needed`; watcher.rs:877 perspective-filter recompute) with `kind` discriminators (scope_chain, palette_open, keymap_mode, inspector_stack, active_view, active_perspective, app_mode, inspector_width, perspective_switch, board_switch, board_close). The bridge plane `notifications/ui_state/changed` (notify.rs:296 constructor) has NO publisher.

## Work
- Publish `notifications/ui_state/changed` on the bridge from the ui-state service when UI state mutates (carry the same `kind`/window/key/value payload).
- Swap the frontend `ui-state-changed` listener to the bridge-forwarded `notifications/ui_state/changed`; remove the direct emits (commands.rs:1889, watcher.rs:877) once swapped.
- Declare on the ui-state service tool (swissarmyhammer-ui-state/src/service.rs:106) via #[notification] struct=payload. The `kind` set should be captured (enum or documented value space) so the declared params describe it.
- Coverage guard.

## Acceptance
A plugin can `this.ui_state.on("changed", cb)`; the frontend reads UI-state changes from the bridge; no direct `ui-state-changed` emit remains; declared == published.