---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffc280
title: Backend drag session state and Tauri commands
---
## What
Add a lightweight drag session to AppState so the backend can coordinate cross-window drags. Three new Tauri commands orchestrate the lifecycle.

**Files:**
- `kanban-app/src/state.rs` — add `DragSession` struct + field on `AppState`
- `kanban-app/src/commands.rs` — add `start_drag_session`, `cancel_drag_session`, `complete_drag_session` commands
- `kanban-app/src/main.rs` — register new commands in `invoke_handler`

**DragSession struct:**
- `entity_type: String`, `entity_id: String`
- `source_board_path: PathBuf`, `source_window_label: String`
- `entity_snapshot: serde_json::Value` (full entity for rendering in target window)
- `modifier_keys: Option<ModifierState>` (track Alt for copy mode)
- `started_at: Instant` (30s auto-expire)

**Commands:**
- `start_drag_session` — reads entity from source board, stores snapshot, emits `drag-session-started` to all windows
- `cancel_drag_session` — clears session, emits `drag-session-cancelled`
- `complete_drag_session(target_board_path, target_column, before_id?, after_id?, copy?)` — if same board: delegate to existing `task.move`. If cross-board: delegate to `task.transfer` or `task.copy`. Clears session, emits `drag-session-completed`

**Events emitted (via `app.emit()`):**
- `drag-session-started { entity_type, entity_id, source_board_path, source_window_label, entity_snapshot }`
- `drag-session-cancelled {}`
- `drag-session-completed { entity_type, entity_id, target_board_path, target_column }`

## Acceptance Criteria
- [ ] DragSession struct exists on AppState behind RwLock
- [ ] start_drag_session reads entity, stores snapshot, emits event
- [ ] cancel_drag_session clears and emits
- [ ] complete_drag_session routes to move vs transfer, clears and emits
- [ ] 30-second stale session auto-cancel on next start_drag_session call
- [ ] All three commands registered in main.rs invoke_handler

## Tests
- [ ] Unit test: start then cancel clears session
- [ ] Unit test: start then complete same-board delegates to task.move
- [ ] Unit test: stale session replaced by new start
- [ ] `cargo nextest run -p kanban-app`