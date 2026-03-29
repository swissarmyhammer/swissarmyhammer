---
assignees:
- claude-code
depends_on:
- 01KM85W10KVXQ4F4JVJCXXPN3A
position_column: done
position_ordinal: ffffffffffdf80
title: Migrate drag_session and context_menu_ids into UIState
---
## What

`drag_session` and `context_menu_ids` are transient state in AppState. They don't persist but they should live in UIState for consistency (single state owner).

### Changes
- Move `drag_session: Option<DragSession>` into UIState (not persisted)
- Move `context_menu_ids: HashSet<String>` into UIState (not persisted)
- Add `#[serde(skip)]` to these fields so they don't pollute the YAML
- UIState methods: `start_drag()`, `cancel_drag()`, `complete_drag()`, `set_context_menu_ids()`
- `complete_drag_session` should dispatch `task.move` / `column.reorder` through dispatch_command instead of handling the move inline
- Remove these fields from AppState
- Update all Tauri commands that reference these fields to go through UIState

## Acceptance Criteria
- [ ] `drag_session` and `context_menu_ids` removed from AppState
- [ ] Drag-and-drop still works (start, cancel, complete)
- [ ] Context menus still work
- [ ] `complete_drag_session` routes entity moves through dispatch_command

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes