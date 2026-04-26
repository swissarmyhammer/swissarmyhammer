---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff9880
title: Opening a board switches the first window instead of the focused window
---
When opening a board, the main window always switches to it because it follows the backend's is_active flag. The focused window should be the one that switches. Need to scope board-changed handling to only follow active board in the window that is focused.