---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffdf80
title: Hot reload only restores one window when two were open
---
Had two windows with two boards open. After hot reload, only one window was restored. The Tauri window-state plugin restores window geometry but the second webview window may not survive Vite HMR. Need to verify window persistence across hot reloads and potentially re-create missing windows.