---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffdc80
title: Hot reload does not restore multiple windows and board selection
---
When the app hot reloads during development, the multiple window state and per-window board selection is lost. Need to restore window-to-board mapping on reload.