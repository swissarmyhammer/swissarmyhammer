---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8680
project: command-cutover
title: Add board-management ops + AppShell seam methods to app-service
---
Add ListOpenBoards (no params) and GetBoardData (optional board_path) operations to swissarmyhammer-app-service; extend AppShell trait with list_open_boards()/get_board_data() returning serde_json::Value via injected Fn callbacks (mirror WindowShell pattern); wire dispatch in service.rs; update SpyShell, app_e2e + meta_snapshot tests.

## Review Findings (2026-06-10 12:05)

### Warnings
- [ ] Card superseded — re-scope or archive. The board-management reads were deliberately placed on the **window** service, not app-service, in commit `3cb02a0c9` ("feat(window): list_open_boards + get_board_data as window MCP ops"); they were never in app-service at any point in history. The placement decision is documented in `crates/swissarmyhammer-app-service/src/operations.rs` (module doc: "The multi-board management reads (`list open boards` / `get board data`) live on the `window` server, not here: that server already owns the full open/close/new/switch board lifecycle, so the read counterparts belong alongside it."). Every deliverable this card describes exists at HEAD in the window-service equivalent form: `ListOpenBoards`/`GetBoardData` op structs registered in `crates/swissarmyhammer-window-service/src/operations.rs`; `WindowShell` trait methods `list_open_boards()`/`get_board_data()` backed by injected `ListOpenBoardsFn`/`GetBoardDataFn` callbacks in `src/shell.rs`; dispatch wired in `src/service.rs` (`handle_list_open_boards`/`handle_get_board_data`); recording test shell in `tests/integration/common.rs`; dedicated e2e coverage in `tests/integration/board_reads_e2e.rs` (3 tests, all passing); real callbacks wired in `apps/kanban-app/src/main.rs`. Verified fresh: `cargo nextest run -p swissarmyhammer-app-service` 6/6 passed, `cargo nextest run -p swissarmyhammer-window-service` 33/33 passed. **No code work remains.** Recommend the owner either archive this card or re-title it to record the window-service outcome and move it to done — as written, its app-service scope will never be implemented.