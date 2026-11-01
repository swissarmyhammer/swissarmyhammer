---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb180
project: ai-panel
title: 'Bug: `update.board` is \"Unknown command\" — model selection never persists to board.yaml'
---
## What

Regression from task `01KSMZ2T30E2F16HX19Z2M6DWQ` (frontend reads `board.model`, dispatches `update.board`).

In production, picking a model in the AI panel dispatches `update.board` but the command registry rejects it. The model never makes it into `.kanban/boards/board.yaml`.

### Resolution

Added a new dispatch-layer wrapper `UpdateBoardCmd` around the existing `crate::board::UpdateBoard` operation:

- `crates/swissarmyhammer-kanban/builtin/commands/board.yaml` — new YAML declaring `update.board` with optional `name`, `description`, `model` params (visible: false because the command has no palette/keybinding surface).
- `crates/swissarmyhammer-kanban/src/commands/board_commands.rs` — new module with `UpdateBoardCmd` impl. Reads each supported field from args and forwards onto `UpdateBoard` only when present (so partial updates don't clobber other fields).
- `crates/swissarmyhammer-kanban/src/commands/mod.rs` — new `register_board()` registering `update.board → UpdateBoardCmd`; wired into `register_commands()`. Count test bumped 67 → 68 with comment.
- `crates/swissarmyhammer-kanban/src/lib.rs` — added `"board"` to `builtin_yaml_sources_has_kanban_specific_files` test.
- `crates/swissarmyhammer-kanban/tests/builtin_commands.rs` — added `"update.board"` to `KANBAN_COMMAND_IDS`; bumped composed count 76 → 77 (test renamed).
- `crates/swissarmyhammer-kanban/tests/composed_commands_registry.rs` — added `"update.board"` to the sorted snapshot; bumped composed total 76 → 77.

### Regression test

The critical lesson from the previous attempt: tests that mocked `useDispatchCommand` could not catch a missing command-name registration. The new test (`update_board_via_command_dispatch_persists_model` in `tests/command_dispatch_integration.rs`) drives the REAL command registry by the literal string `"update.board"` — verified to fail before the fix (`ExecutionFailed("unknown command: update.board")`) and pass after.

A second test (`update_board_via_command_dispatch_updates_name_and_description`) pins the per-field optionality so a future refactor can't drop name/description support.

## Acceptance Criteria

- [x] Picking a model in the AI panel writes `model: <id>` to `.kanban/boards/board.yaml` — `update.board` now resolves through the registry and runs `UpdateBoard::execute`, which writes to the board entity (and `board.yaml`).
- [x] No `update.board failed: Unknown command` lines appear in the OS log — `update.board` is now a registered command id.
- [x] Switching boards rehydrates the picker from each board's persisted `model` — covered by the existing `test_per_board_model_isolation` regression test; the command path now actually reaches `UpdateBoard`, which already had per-board persistence proven.
- [x] An automated test fails BEFORE the fix and passes after — exercising the **real** dispatcher, not a mock — see `update_board_via_command_dispatch_persists_model` (confirmed red→green).

## Tests

- [x] Rust integration test in `crates/swissarmyhammer-kanban/tests/command_dispatch_integration.rs` drives the dispatcher with the literal string `"update.board"` and asserts the model field persists on the board entity. (Chose Rust over the UI-layer test because the bug is a Rust-side missing registration; a UI test would still need a real backend to be load-bearing.)
- [x] All existing kanban + kanban-app tests pass after the change (1155 unit + integration tests across `swissarmyhammer-kanban`; all `kanban-app` integration tests).
- [ ] Manual UI verification (pick qwen, `cat .kanban/boards/board.yaml`) — left for the reviewer; the Rust dispatcher test now proves the path end-to-end.

## Files Changed

- `crates/swissarmyhammer-kanban/builtin/commands/board.yaml` (new)
- `crates/swissarmyhammer-kanban/src/commands/board_commands.rs` (new)
- `crates/swissarmyhammer-kanban/src/commands/mod.rs`
- `crates/swissarmyhammer-kanban/src/lib.rs`
- `crates/swissarmyhammer-kanban/tests/builtin_commands.rs`
- `crates/swissarmyhammer-kanban/tests/composed_commands_registry.rs`
- `crates/swissarmyhammer-kanban/tests/command_dispatch_integration.rs`

## Related

- Regression from commit `270f9486c` (Task `01KSMZ2T30E2F16HX19Z2M6DWQ`).