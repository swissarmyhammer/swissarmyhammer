---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8580
project: command-cutover
title: Wire board-management callbacks in kanban-app; remove Tauri handlers
---
Wire list_open_boards/get_board_data callbacks in build_apphandle_shells to existing projection logic; make commands.rs projection helpers pub(crate); remove list_open_boards + get_board_data Tauri handlers + generate_handler! entries; update SpyAppShell.

## Review Verification (2026-06-10)

Substance fully landed, under the window-service design (see sibling card 01KT7QA0YH9G0809XWM2FNSXY0 / commit 3cb02a0c9 for the full design finding — the board-management reads live on the `window` server, not the app server). Verified at HEAD:

- [x] Callbacks wired — `build_apphandle_shells` (apps/kanban-app/src/main.rs) builds `ListOpenBoardsFn`/`GetBoardDataFn` closures that run `commands::list_open_boards_impl` / `get_board_data_impl` on the confinement runtime and passes them into `TauriWindowShell::new` (window shell, not app shell, per the documented decision).
- [x] Projection helpers `pub(crate)` — `commands::list_open_boards_impl` and `commands::get_board_data_impl` in apps/kanban-app/src/commands.rs.
- [x] Legacy Tauri handlers removed — no `#[tauri::command]` `list_open_boards`/`get_board_data` fns remain; `generate_handler![]` in main.rs lists neither. Frontend reaches the reads only through `ui/src/lib/window-mcp.ts` (`callMcpTool("window", "list open boards" / "get board data")`); the `no-direct-invoke` guardrail test enforces no direct `invoke()` of the legacy commands. Remaining `invoke("list_open_boards"/"get_board_data")` strings in ui are test-mock translators only.
- [x] Spy shell updated — under the shell split this landed on `SpyWindowShell` (apps/kanban-app/src/state.rs), which implements `list_open_boards`/`get_board_data`; `SpyAppShell` correctly shrank to quit/about/help.
- [x] Tests — `cargo nextest run -p swissarmyhammer-window-service`: 33/33 passed, including `board_reads_e2e` 3/3.