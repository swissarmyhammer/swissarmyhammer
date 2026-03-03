---
title: Build React app shell with nav bar and board switcher
position:
  column: done
  ordinal: a5
---
Create the React app shell — the visual frame of the application with a top nav bar.

Files:
- ui/src/App.tsx — root component. On mount: invoke("get_board") + invoke("list_open_boards") + invoke("get_recent_boards"). Renders NavBar + main content placeholder.
- ui/src/components/nav-bar.tsx — fixed top bar with:
  - Board switcher dropdown (shadcn DropdownMenu):
    - "Open" section: currently loaded boards, click to switch (invoke set_active_board)
    - "Recent" section: from MRU config, click to reopen (invoke open_board)
    - Separator, then "Open Board..." triggers Tauri folder dialog (invoke open_board with selected path)
    - Active board has check mark
  - Active board name as dropdown trigger label
  - Summary badges: total tasks, ready count, blocked count (from GetBoard summary)
- ui/src/types/kanban.ts — TypeScript types mirroring Rust types: Board, Column, Swimlane, Tag, BoardSummary, Task, Position, OpenBoard, RecentBoard

Depends on: frontend scaffold + Tauri commands.
Verify: cargo tauri dev launches window, nav bar shows board name + summary badges, dropdown lists open boards.