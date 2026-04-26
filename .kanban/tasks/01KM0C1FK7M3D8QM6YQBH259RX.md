---
assignees:
- claude-code
depends_on:
- 01KM0C0X4KT5C8CVW71VWRE9ZK
position_column: done
position_ordinal: ffffffffffb580
title: Implement OS-level drag initiation from Rust
---
## What
Create a new Tauri command `start_os_drag` that initiates an OS-level drag operation using `tauri-plugin-drag` with `DragItem::Data`. This is what makes the card ghost appear at the OS level and follow the cursor across windows.

**Files:**
- `kanban-app/src/commands.rs` — new `start_os_drag` command
- `kanban-app/src/main.rs` — register the new command in `.invoke_handler()`

**Approach:**
When the frontend detects a task drag starting (in `handleDragStart`), it calls this command with:
- `task_id` — the entity being dragged
- `task_fields` — serialized fields for the data provider
- `window_label` — which window to initiate the drag from

The command:
1. Gets the window handle from the label
2. Calls the existing `start_drag_session()` logic to set up the Tauri event session
3. Creates a `DragItem::Data` with:
   - `provider`: closure that serializes task data as JSON bytes when asked for type `\"dev.swissarmyhammer.task\"`
   - `types`: `vec![\"dev.swissarmyhammer.task\".to_string()]`
4. Creates a `drag::Image::Raw` for the drag preview (start with a simple placeholder icon — full card rendering comes in a later card)
5. Calls `drag::start_drag()` with the window's raw handle

**Important:** `DragItem::Data` only works on macOS. The `DataProvider` is `Box<dyn Fn(&str) -> Option<Vec<u8>>>` — when the drop target requests data for our custom UTI, we return the JSON-serialized task.

**Note on drag::start_drag():** This is a blocking call that doesn't return until the drag completes or is cancelled. It needs to run on a background thread or be carefully managed to not block the Tauri async runtime.

## Acceptance Criteria
- [ ] `start_os_drag` command exists and compiles
- [ ] Calling it initiates a visible OS drag ghost on macOS
- [ ] The drag ghost follows the cursor across window boundaries
- [ ] The Tauri event session (`drag-session-active`) is also emitted
- [ ] The command doesn't block the UI thread

## Tests
- [ ] `cargo nextest run` — no regressions
- [ ] Manual test: trigger `start_os_drag` — OS drag ghost appears and crosses windows
- [ ] Manual test: press Escape during drag — session cancels cleanly