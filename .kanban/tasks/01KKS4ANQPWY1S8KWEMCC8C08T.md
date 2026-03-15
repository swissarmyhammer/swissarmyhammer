---
assignees:
- claude-code
position_column: done
position_ordinal: fffffd80
title: 'Quick Capture: wire up provider stack + entity event listeners'
---
## What
The quick capture window currently has a minimal provider stack (`SchemaProvider → EntityStoreProvider(entities={}) → KeymapProvider`) with an **empty** entity store and **zero** Tauri event listeners. This means:
- Board names never update when changed on disk or in the main window
- Board list never refreshes except on window focus
- `useFieldUpdate()` returns NO_OP (board rename silently fails)
- `BoardSelector` gets no `boardEntity` prop (can't display live name)

### Approach
Add the minimum providers needed for quick capture to work properly:
1. **FieldUpdateProvider** — enables board rename via `useFieldUpdate`
2. **Entity loading** — load board entities via `list_entities({entityType: "board"})` or extract from `get_board_data`
3. **Tauri event listeners** — listen for `entity-field-changed` and `board-changed` events to update board names/list dynamically
4. **Pass `boardEntity`** to `BoardSelector` so it can display and edit the live name

### Files
- `kanban-app/ui/src/App.tsx` (QuickCaptureApp function, lines 478-493) — add FieldUpdateProvider, entity loading
- `kanban-app/ui/src/components/quick-capture.tsx` — add event listeners, pass boardEntity to BoardSelector

## Acceptance Criteria
- [ ] Changing board name in main app → quick capture shows updated name without refocusing
- [ ] Changing board name on disk → quick capture shows updated name
- [ ] Board rename via EditableMarkdown in quick capture's BoardSelector works and persists
- [ ] Adding/removing boards in main app → quick capture dropdown updates
- [ ] Board list refreshes on window focus (existing behavior preserved)

## Tests
- [ ] Manual test: Rename board in main window → open quick capture → name is updated
- [ ] Manual test: In quick capture, rename board via EditableMarkdown → name persists in main app
- [ ] Manual test: Open new board in main app → quick capture dropdown shows it