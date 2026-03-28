---
assignees:
- claude-code
depends_on:
- 01KM85VAEJM77M03VP1P444AS1
position_column: done
position_ordinal: ffffffffffda80
title: Migrate recent_boards into UIState
---
## What

`recent_boards` lives in AppConfig. The `get_recent_boards` Tauri command reads it directly. MRU updates happen as side effects of `open_board`.

### Changes
- Move `recent_boards: Vec<RecentBoard>` into UIState
- UIState.open_board() updates the MRU list as a side effect (already the pattern)
- Remove `get_recent_boards` Tauri command — read from `useUIState().recentBoards` instead
- Remove `recent_boards` from AppConfig
- Frontend: menu bar Open Recent reads from UIState

## Acceptance Criteria
- [ ] Recent boards persist via UIState
- [ ] `get_recent_boards` Tauri command removed
- [ ] Open Recent menu still works

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes