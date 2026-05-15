---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff780
title: '[High] Keybinding conflict: file.newBoard and window.new both bind Mod+Shift+N'
---
**Files**: `swissarmyhammer-commands/builtin/commands/file.yaml` line 20, `swissarmyhammer-commands/builtin/commands/ui.yaml` lines 44-46\n\n**Issue**: Both `file.newBoard` (New Board) and `window.new` (New Window) declare `cua: Mod+Shift+N`. Tauri's native menu will only bind one of them, making the other unreachable from the keyboard. Which one wins depends on menu insertion order — non-deterministic from the user's perspective.\n\n**Severity**: High (functionality)\n**Layer**: Functionality/Correctness\n\n**Fix**: Assign a distinct keybinding to one of the two commands. Common convention: `Mod+Shift+N` for New Window, `Mod+Shift+B` or similar for New Board."