---
depends_on: ''
position_column: done
position_ordinal: b680
title: 'UI smoke test: verify board loads and data reload works!'
---
Launch the Tauri app and verify the full data flow: initial load, mutation via commands, and event-driven refresh.

## Data Reload Architecture

```
User action (palette, keybinding, context menu, drag-drop)
  → dispatchCommand(cmd) in command-scope.tsx
    → invoke("dispatch_command", { cmd, target, args }) to Rust
      → commands.rs: looks up Command impl, builds CommandContext, executes
      → if undoable: app.emit("board-changed", ())
  → App.tsx listens for "board-changed" event
    → calls refresh() which does parallel fetches:
        get_board_data  → board, columns, swimlanes, tags (with counts)
        list_open_boards → sidebar board list
        list_entities(task) → all tasks (with enrichment)
    → sets React state: setBoard, setTaskEntities, setTagEntities
    → React re-renders board view
```

## Key event sources
- `dispatch_command` (commands.rs:558-561): emits `board-changed` for undoable commands
- `handle_menu_event` (menu.rs:261): emits `board-changed` for native menu actions
- `field-update-context.tsx`: field edits dispatch `entity.update_field` → Rust emits event
- Drag-drop in `board-view.tsx`: dispatches `task.move` → Rust emits event

## Checklist

- [x] Board name visible in nav
- [ ] Columns render (todo, doing, done)
- [ ] Tasks visible in columns
- [ ] Add task via palette → task appears without manual refresh
- [ ] Edit task title in inspector → title updates on board
- [ ] Drag task between columns → position persists after reload
- [ ] Delete task → card disappears
- [ ] Switch keymap in palette → Settings menu radio updates
- [ ] Escape closes inspector panel
- [ ] Escape closes palette (vim: normal-mode Escape closes)
- [ ] Check browser console for [keybindings], [dispatch] debug logs
- [ ] Check Rust logs for dispatch_command tracing output
- [ ] No console errors

#test #sample #smoke-test #reload