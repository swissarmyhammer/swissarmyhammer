---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8780
title: Move palette_open/palette_mode to per-window state — palette opens in both windows
---
## What

Pressing `:` opens the command palette in ALL windows, not just the focused one. Root cause: `palette_open` and `palette_mode` are global fields on `UIStateInner` (lines 114-117 in `swissarmyhammer-commands/src/ui_state.rs`). When any window dispatches `app.command`, it sets the global `palette_open = true`, the `ui-state-changed` event broadcasts to all windows, and all windows see `palette_open: true` and render the palette.

The fix: move `palette_open` and `palette_mode` into `WindowState` (per-window), just like `inspector_stack` and `active_view_id`. The `CommandPaletteCmd` uses `ctx.window_label_from_scope()` (from the root `window:*` moniker in the scope chain) to target only the invoking window.

### Backend changes (`swissarmyhammer-commands/src/ui_state.rs`)

1. **Move fields to `WindowState`** (line 38):
   - Add `palette_open: bool` (transient, `#[serde(skip)]`)
   - Add `palette_mode: String` (transient, `#[serde(skip)]`)
2. **Remove from `UIStateInner`** (lines 112-117): remove global `palette_open` and `palette_mode`
3. **Update `set_palette_open()`** — takes `window_label: &str`, modifies `windows[label].palette_open`
4. **Update `set_palette_open_with_mode()`** — takes `window_label: &str`, modifies `windows[label]`
5. **Update `palette_open()`** — takes `window_label: &str`, reads from `windows[label]`
6. **Update `palette_mode()`** — takes `window_label: &str`, reads from `windows[label]`
7. **Update `to_json()`** — remove global `palette_open`/`palette_mode`, include them in each window's JSON object
8. **Update tests** — `set_palette_open_toggles` etc. need a window_label arg

### Backend command changes

9. **`app_commands.rs` — `CommandPaletteCmd`/`SearchPaletteCmd`**: use `ctx.window_label_from_scope().unwrap_or(\"main\")` to get the window label, pass it to `set_palette_open_with_mode(window_label, ...)`
10. **`app_commands.rs` — `DismissCmd`**: use `window_label_from_scope()` for the palette close check too (line 166: `ui.palette_open()` → `ui.palette_open(window_label)`)

### Frontend changes

11. **`ui-state-context.tsx`** — move `palette_open` and `palette_mode` from `UIStateSnapshot` into `WindowStateSnapshot`
12. **`app-shell.tsx`** — read `palette_open`/`palette_mode` from `uiState.windows?.[WINDOW_LABEL]` instead of top-level `uiState`

## Acceptance Criteria
- [ ] Pressing `:` opens the palette in the focused window only
- [ ] The other window is unaffected
- [ ] Dismiss (`Escape`) closes the palette only in the invoking window
- [ ] `cargo nextest run` passes
- [ ] `cd kanban-app && npx vitest run` passes (no new failures)

## Tests
- [ ] `ui_state.rs` — test: `set_palette_open` with window label targets only that window
- [ ] `ui_state.rs` — test: `palette_open(\"main\")` returns false when `palette_open(\"secondary\")` is true
- [ ] `app_commands.rs` — test: `CommandPaletteCmd` with scope chain `[\"window:secondary\"]` sets palette_open only on secondary window
- [ ] `cargo nextest run -p swissarmyhammer-commands` passes
- [ ] `cargo nextest run -p kanban-app` passes