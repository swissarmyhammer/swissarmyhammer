---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9a80
title: '[Medium] Menu rebuild on every window focus event may cause flicker'
---
**File**: `kanban-app/src/main.rs` line 238\n\n**Issue**: `WindowEvent::Focused(true)` calls `crate::menu::rebuild_menu(window.app_handle())` which completely tears down and rebuilds the native menu bar. This fires on every window focus change (e.g., switching between the app and another app, or between two app windows). Full menu rebuilds on macOS can cause visible flicker in the menu bar.\n\n**Severity**: Medium (performance/UX)\n**Layer**: Performance\n\n**Fix**: Consider only updating the Window menu checkmarks on focus change (like `handle_menu_event` does for `window.focus:*` clicks) rather than rebuilding the entire menu. Full rebuilds should be reserved for structural changes (new/closed windows, keymap mode change)."