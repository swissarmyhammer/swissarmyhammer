---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff980
title: Set window title from board display name
---
## What

Every window is titled \"SwissArmyHammer\" (hardcoded at `kanban-app/src/commands.rs:652` and `:735`). The title should reflect the board loaded in that window, using the board's display name from `KanbanContext::name()` (the path stem above `.kanban`, e.g. \"swissarmyhammer-kanban\"). This also makes the Window menu's `window.focus:*` items useful — they'll show distinguishable names instead of identical \"SwissArmyHammer\" entries.

### Changes

**1. Set title on board switch** (`kanban-app/src/commands.rs:1004-1025`):
- After the `BoardSwitch` result handler opens the board and sets `window_board`, call `set_title` on the window:
- Get the `BoardHandle` from `state.boards`, call `handle.ctx.name()` for the display name
- `app.get_webview_window(label)?.set_title(&format!(\"SwissArmyHammer — {}\", ctx_name))`
- The format: \"SwissArmyHammer — project-name\" (app name + board context name)

**2. Set title on new window creation** (`kanban-app/src/commands.rs:651-658`):
- After `create_new_window` builds the window and the board path is known, set the title from the board handle's `ctx.name()`
- Fallback to just \"SwissArmyHammer\" if no board is assigned

**3. Set title on window restore** (`kanban-app/src/commands.rs:734-751`):
- Same pattern — after restoring a window with its board, set the title from the board handle

**4. Update title on board close** (`kanban-app/src/commands.rs` — `BoardClose` handler):
- When a board is closed in a window, reset the title to just \"SwissArmyHammer\"

## Acceptance Criteria
- [x] Window title shows \"SwissArmyHammer — project-name\" when a board is loaded
- [x] Window title shows just \"SwissArmyHammer\" when no board is loaded
- [x] Switching boards updates the window title
- [x] Window menu shows distinguishable window names
- [x] `cargo nextest run` passes

## Tests
- [x] `context.rs` — existing `name()` test covers the display name derivation
- [x] `cargo nextest run -p kanban-app` passes
- [x] `cargo nextest run -p swissarmyhammer-kanban` passes