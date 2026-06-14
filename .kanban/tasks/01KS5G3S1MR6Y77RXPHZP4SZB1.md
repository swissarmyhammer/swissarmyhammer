---
assignees:
- claude-code
depends_on:
- 01KS5G3AKZXDN7K6YR415E0V4K
- 01KS5F8THM5EQMKFSF6GFAE55C
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffcd80
project: command-events
title: 'Frontend: subscribe to MCP change notifications (reuse reducer + add txn batching)'
---
## What

Migrate the kanban webview from Tauri-specific change events to the MCP notification surface, so the UI is just another MCP client (same stream agents receive). The existing entity/field reload reducer is solid — **keep it; only change its input source** from Tauri `listen("entity-field-changed", …)` to the MCP `notifications/store/changed` subscription. Add transaction batching so a command's N changes apply as one atomic re-render.

Files (`apps/kanban-app/ui/src/...`):
- Data layer — subscribe to the MCP notification planes and route into the **existing reducer**:
  - `notifications/store/changed { store, item, op, changes?, txn, origin }` → apply field patches (entities) or reload item (views/perspectives) using today's reducer logic, unchanged
  - `notifications/store/undo_changed` → Undo/Redo control state
  - `notifications/ui_state/changed` → UI state
  - `notifications/commands/changed` → palette/menu refresh (already wired in the dispatch task)
- **Txn batching**: buffer `store/changed` events by `txn` and flush as one atomic state update per transaction, so a multi-write command (or an undo of one) re-renders once, not N times. A short coalescing window keyed on `txn`; flush on txn change or a microtask tick.
- Remove the Tauri `listen("entity-field-changed"|"board-changed"|"attachment-changed"|...)` call sites
- `apps/kanban-app/src/commands.rs` — remove the `app.emit(...)` change-event calls (handled in the cut-over Tauri-migration task if any remain)

Memory `metadata-driven-ui`, `frontend-logging` apply. The reducer that applies thin patches does NOT change — only the source (MCP) and the batching wrapper.

## Acceptance Criteria
- [ ] The webview receives all change planes over MCP, not Tauri `app.emit`
- [ ] The existing entity/field reducer is reused unchanged; only its input source changed
- [ ] A command's N `store/changed` events sharing a `txn` apply as ONE atomic re-render (txn batching)
- [ ] Undo/Redo controls reflect `notifications/store/undo_changed`
- [ ] No React code path still depends on `listen("entity-field-changed")` and friends

## Tests
- [ ] `apps/kanban-app/ui/src/.../mcp-notifications.test.tsx` — emit `store/changed` with `{field,value}`; assert the board patches identically to the old Tauri path (reducer reused); emit `op:"removed"` → card removed
- [ ] `apps/kanban-app/ui/src/.../txn-batching.test.tsx` — emit 3 `store/changed` sharing one `txn`; assert exactly one render flush; a 4th with a new `txn` flushes separately
- [ ] `apps/kanban-app/ui/src/.../undo-controls.test.tsx` — emit `undo_changed { can_undo:false, can_redo:true }`; assert Undo disabled, Redo enabled
- [ ] grep test: no `listen("entity-field-changed"|"board-changed"|...)` remains
- [ ] `npm test --prefix apps/kanban-app/ui` passes

## Workflow
- Use `/tdd` — write the txn-batching test first; it pins the atomic-per-command-render contract while proving the reducer is reused.

Depends on the MCP notification surface + change-propagation tasks.