---
assignees:
- claude-code
depends_on:
- 01KS36VTN9K8C41P20SJ2WQA6X
position_column: todo
position_ordinal: 9f80
project: command-backends
title: '`window` server: board-file lifecycle (switch/close/new/open + OS dialog)'
---
## What

Add the board-file lifecycle operations to the `window` MCP server (same server as the window/OS-file-ops task — this task adds the board-lifecycle op group). These back the `file.*` commands: `SwitchBoard`, `CloseBoard`, `NewBoard`, `OpenBoard`.

REALITY CHECK (verified against `apps/kanban-app/src/`): there is NO `commands/file_commands.rs`. The relevant code is:
- `new_board_dialog`, `open_board_dialog` Tauri handlers in the single `apps/kanban-app/src/commands.rs` (these show the OS file dialogs).
- Board open/close are methods on `AppState` (`apps/kanban-app/src/state.rs:601`, `:877`), not file-command handlers.
So `NewBoard`/`OpenBoard` port the existing dialog handlers (incl. the OS file-open/new dialog), while `SwitchBoard`/`CloseBoard` wrap the existing `AppState` methods. Port the actual file/window I/O into the `window` server; this is partly relocation (dialogs) and partly wrapping existing AppState methods.

Files:
- `crates/swissarmyhammer-window-service/src/operations.rs` — add `#[operation]` structs: `SwitchBoard`, `CloseBoard`, `NewBoard`, `OpenBoard` (OpenBoard shows the OS file-open dialog; NewBoard creates a board file via the existing dialog path).
- `crates/swissarmyhammer-window-service/src/service.rs` — board lifecycle methods on `WindowService`, delegating to the relocated dialog logic + `AppState` board open/close.
- `apps/kanban-app/src/commands.rs` — the `new_board_dialog`/`open_board_dialog` logic moves into the window service (left dead at cut-over).

## Acceptance Criteria
- [ ] `SwitchBoard`/`CloseBoard`/`NewBoard`/`OpenBoard` reachable on the `window` server
- [ ] `OpenBoard` shows the OS file-open dialog (porting `open_board_dialog`); `NewBoard` creates a board (porting `new_board_dialog`)
- [ ] `SwitchBoard`/`CloseBoard` drive the existing `AppState` board open/close (`state.rs:601`/`:877`) with no behavior change
- [ ] `_meta` tree includes the four board ops

## Tests
- [ ] `crates/swissarmyhammer-window-service/tests/integration/board_lifecycle_e2e.rs` — `NewBoard` creates a board file (assert on disk); `OpenBoard` with an injected/mocked picker opens a board; `SwitchBoard`/`CloseBoard` change the active board state. Use an injectable picker shim so the dialog path is testable without manual interaction (define the shim seam).
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-window-service` passes

## Workflow
- Use `/tdd`

Depends on the `window` window/OS-ops task (same server crate). Prerequisite for the file-commands plugin.