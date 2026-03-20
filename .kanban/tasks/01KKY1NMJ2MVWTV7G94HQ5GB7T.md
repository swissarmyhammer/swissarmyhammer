---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffe880
title: Restored windows forget screen position on hot reload
---
Windows are restoring on hot reload and boards are preserved, but the window x/y screen position is lost. The create_window call creates windows with default position instead of restoring saved position. The tauri-plugin-window-state should handle this for windows it knows about, but dynamically created windows may not get their state restored.