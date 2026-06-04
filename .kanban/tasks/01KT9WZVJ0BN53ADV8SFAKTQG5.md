---
assignees:
- claude-code
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: todo
position_ordinal: b980
project: command-events
title: Route entity/view/perspective + undo changes onto the bridge (store/changed, store/undo_changed)
---
Make entity/view/perspective data changes and undo-stack changes observable on the MCP bridge (so plugins can subscribe), and make the frontend consume them from the bridge instead of direct Tauri events.

Current state (inventory): the kanban `spawn_notification_fanin` (crates/swissarmyhammer-kanban/src/notify_fanin.rs) already translates EntityEvent/ViewEvent/PerspectiveEvent → `notifications/store/changed` and StackState → `notifications/store/undo_changed`, but it is TEST-ONLY (callers only in command-service integration tests). The running app instead emits direct Tauri `entity-created/removed/field-changed/attachment-changed` via watcher `run_bridge` (apps/kanban-app/src/watcher.rs:174-189, spawned state.rs:481), and reads undo flags via post-command sync.

## Work
- Wire `spawn_notification_fanin` into apps/kanban-app bootstrap so both methods publish on the bridge in the running app.
- Swap the frontend to the bridge-forwarded notifications (the per-window forwarder already re-emits each bridge notification as a Tauri event named by its method — commands.rs:2593): `entity-*` listeners → `notifications/store/changed`; undo UI → `notifications/store/undo_changed`. Reuse the existing entity/field reload reducer; keep txn batching.
- Remove the now-redundant watcher `run_bridge` direct `entity-*` emits once the frontend is swapped.
- Declare both on the `store` service tool (this.store.on("changed") / .on("undo_changed")) via #[notification] struct=payload (mechanism card 01KT9JV0N1). Owner/publisher split: declared on `store`, published by the kanban fan-in — the coverage guard is namespace-based (every `notifications/store/*` published must be declared on the store tool, and vice versa).
- Coverage guard test.

## Acceptance
A plugin can `this.store.on("changed", cb)` / `.on("undo_changed", cb)` and receive real entity/view/perspective/undo changes; the frontend re-renders from the bridge; no direct `entity-*` emits remain; declared == published.