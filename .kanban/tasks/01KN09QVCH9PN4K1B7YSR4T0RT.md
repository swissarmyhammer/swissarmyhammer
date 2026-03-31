---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8980
title: 'window.focus commands: dynamic commands for open windows, drawn in Window menu'
---
## What

Open windows are not listed in the Window menu and there's no command to focus a window. The current approach hard-codes window items directly in the menu builder (`menu.rs:171-191`) bypassing the command system entirely. This means they don't appear in the palette either.

Fix: make `window.focus:{label}` a proper dynamic command — generated in `commands_for_scope` like `view.switch:*` and `board.switch:*`, with `menu` placement on the Window menu. The menu builder draws them like any other command. The palette shows them too.

### Changes

**1. Add `WindowInfo` to `DynamicSources`** (`swissarmyhammer-kanban/src/scope_commands.rs`):
- New struct `WindowInfo { label: String, title: String, focused: bool }`
- Add `windows: Vec<WindowInfo>` to `DynamicSources`

**2. Generate `window.focus:{label}` commands** (`swissarmyhammer-kanban/src/scope_commands.rs`):
- In the dynamic commands section (step 3), for each window generate a `ResolvedCommand`:
  - `id: "window.focus:{label}"`
  - `name: "{title}"` (or use template `"{{entity.display_name}}"`)
  - `menu_name: Some("{title}")` — same as name, no "Switch to" prefix
  - `group: "window"`
  - `context_menu: false`
- These appear in both the palette and menu

**3. Populate `WindowInfo` in `list_commands_for_scope`** (`kanban-app/src/commands.rs`):
- Read `app.webview_windows()` to get visible windows with titles
- Build `Vec<WindowInfo>` and set on `DynamicSources.windows`

**4. Handle `window.focus:*` dispatch** (`kanban-app/src/commands.rs`):
- In `dispatch_command_internal`, intercept `window.focus:*` pattern (like `view.switch:*` and `board.switch:*`)
- Extract label, call `app.get_webview_window(label)` → `unminimize()` + `set_focus()`

**5. Remove hardcoded window items from menu builder** (`kanban-app/src/menu.rs:171-191`):
- Delete the entire "Open windows with a checkmark" block
- The Window menu's window list will now come from the command system via `menu` placement on the generated commands

**6. Add `menu` placement to generated window commands** (`swissarmyhammer-kanban/src/scope_commands.rs`):
- Set `menu: Some(MenuPlacement { path: ["Window"], group: 10, order: 0 })` on the generated `window.focus:*` commands
- Wait — `ResolvedCommand` doesn't have a `menu` field. The menu builder reads from `CommandDef` in the registry, not from `ResolvedCommand`. So either:
  - (a) Add `menu` to `ResolvedCommand` and have the menu builder also draw from resolved dynamic commands, OR
  - (b) Keep the window items in the menu builder but generate them from `DynamicSources.windows` instead of raw `app.webview_windows()` — the menu builder receives the dynamic sources

Option (b) is simpler: pass `DynamicSources` to `build_menu_from_commands` so it can draw window items from the same data that generates palette commands. The menu builder handles the Window menu section, the command system handles the palette.

## Acceptance Criteria
- [x] Window menu lists all open windows by title
- [x] Clicking a window in the menu brings it to the front
- [x] Palette shows window focus commands (e.g. "SwissArmyHammer")
- [x] `window.focus:*` dispatch works from both menu and palette
- [x] Menu rebuilds when windows are created or destroyed
- [x] `cargo nextest run` passes

## Tests
- [x] `scope_commands.rs` — test: window.focus commands generated from DynamicSources.windows
- [x] `scope_commands.rs` — test: window commands have correct names and IDs
- [x] `commands.rs` — test: `window.focus:*` dispatch interception works
- [x] `cargo nextest run -p swissarmyhammer-kanban` passes
- [x] `cargo nextest run -p kanban-app` passes