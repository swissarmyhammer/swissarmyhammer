---
assignees:
- claude-code
depends_on:
- 01KN24NAX41PVGAZ927NKZZ95X
position_column: done
position_ordinal: ffffffffffffffffa280
title: 'Bug: command palette opens in wrong window (stale scope chain)'
---
## What

When you have multiple windows and press Cmd+Shift+P, the command palette opens in whichever window last had a click interaction — not the currently focused window.

**Root cause**: `CommandPaletteCmd` (`swissarmyhammer-kanban/src/commands/app_commands.rs:119`) calls `ctx.window_label_from_scope().unwrap_or("main")` to determine which window to set `palette_open` on. But the scope chain is stale — the frontend does not re-dispatch `ui.setFocus` when a window gains OS focus (only on entity clicks). So `window_label_from_scope()` returns the label of the last window where the user clicked a card/tag/etc., not the currently focused window.

The frontend side is correct: `AppShell` reads `paletteOpen` from `uiState.windows[getCurrentWindow().label]?.palette_open`. Each window reads its own state. The bug is the backend setting the wrong window's state.

**This is fixed by the parent card** (re-dispatch `ui.setFocus` on window focus in `entity-focus-context.tsx`). Once the scope chain updates on window focus, `window_label_from_scope()` returns the correct window label. Same fix also affects `PaletteOpenCmd`, `PaletteCloseCmd`, `SearchPaletteCmd`, `DismissCmd`, and any other command using `window_label_from_scope()`.

### No code changes needed beyond the parent card

This card exists to document the symptom and verify the fix. After the parent card is implemented, this should be verified and closed.

## Acceptance Criteria
- [ ] With 2+ windows open, Cmd+Shift+P opens the palette in the currently focused window
- [ ] Escape closes the palette in the current window only
- [ ] `app.search` (Cmd+F) opens search palette in the correct window

## Tests
- [ ] Manual: open two windows, click in Window A, Alt-Tab to Window B, press Cmd+Shift+P → palette appears in Window B (not A)
- [ ] Manual: open palette in Window B, Alt-Tab to Window A, press Cmd+Shift+P → palette appears in Window A (Window B's palette unaffected)