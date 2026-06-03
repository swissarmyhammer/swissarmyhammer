---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
project: command-cutover
title: Add board-management ops + AppShell seam methods to app-service
---
Add ListOpenBoards (no params) and GetBoardData (optional board_path) operations to swissarmyhammer-app-service; extend AppShell trait with list_open_boards()/get_board_data() returning serde_json::Value via injected Fn callbacks (mirror WindowShell pattern); wire dispatch in service.rs; update SpyShell, app_e2e + meta_snapshot tests.