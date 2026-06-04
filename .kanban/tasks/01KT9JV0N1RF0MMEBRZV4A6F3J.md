---
assignees:
- claude-code
depends_on:
- 01KT9JTDE3EX2BQNQ4F3HMZYTP
position_column: todo
position_ordinal: b780
project: plugin-arch
title: Declare emitted notifications on owning MCP services (commands, store, ui_state)
---
Populate the notification vocabulary by decorating the services that own each notification, using the #[notification] attribute + notifications() slice + operation_tool!{ notifications: ... } from the macro card. This is what makes concrete event names show up in each service's `_meta` for the SDK.

Owners (verified map of who constructs/publishes each McpNotification):
- **swissarmyhammer-command-service** (`service.rs:285` build_tool_definition): declare `notifications/commands/executed` (LIVE publisher — BridgeActionSink, bootstrap.rs:154) and `notifications/commands/changed` (no publisher yet; declare anyway as the contract).
- **swissarmyhammer-kanban** (owns notify_fanin.rs): declare `notifications/store/changed` and `notifications/store/undo_changed`. NOTE these are only published by `spawn_notification_fanin`, which is test-spawned and NOT app-wired today — declaration is still correct (vocabulary ≠ live publisher). The kanban operation tool is built in swissarmyhammer-kanban/src/schema.rs (KANBAN_OPERATIONS).
- **swissarmyhammer-ui-state** (`service.rs:106`): declare `notifications/ui_state/changed` (no publisher yet).
- `notifications/tools/list_changed` has NO service owner (it's a host/registry concern) — skip here; revisit if/when the host publishes it.

## Per declaration
- Payload struct fields mirror the corresponding `McpNotification` constructor's params (notify.rs): executed → {id, ctx, result}; store_changed → {store, item, op, changes?}; undo_changed → {can_undo, can_redo, undo_label?, redo_label?}; ui_state_changed → {window?, key, value}. Fields give codegen a typed callback param.
- DRIFT RISK: the declared struct duplicates the imperative constructor's shape. Acceptable for now; a future card may unify so the declared struct IS the publish payload (single source of truth). Note it in code comments.

## Tests
- For each touched service, `tools/list` carries `io.swissarmyhammer/notifications` with the expected event keys + methods.

## Acceptance
The commands, kanban, and ui_state operation tools advertise their notifications in `_meta`; `this.commands.on(...)` etc. can be resolved against real declarations.