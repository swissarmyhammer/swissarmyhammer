---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffac80
title: Board switch leaves stale perspective filter, hiding new board's cards until user toggles
---
## What

When the user switches the active board (or opens a different one), columns appear empty until they toggle a perspective tab or change the view kind. The new board's cards become visible only after that manual toggle.

**Root cause.** `SwitchBoardCmd` in `crates/swissarmyhammer-kanban/src/commands/file_commands.rs` only updates `windows[label].board_path` (via `UIState::set_window_board`). It does NOT reset the per-window `active_perspective_id` or the transient `filtered_task_ids` slot. Result:

1. Stale `active_perspective_id` from the prior board persists. It points to a perspective id that does not belong to the new board.
2. Stale `filtered_task_ids` persists and contains task IDs from the prior board, none of which exist in the new board, so every column filters out to empty.
3. `useAutoSelectActivePerspective` in `apps/kanban-app/ui/src/lib/perspective-context.tsx` does have a repair path that fires when the stored id is no longer in the loaded perspective list — but it only triggers after the perspective list re-fetch completes, and even when it does fire, the user sees a stale-empty render in between.
4. Toggling a perspective tab dispatches `perspective.switch`, which atomically recomputes `filtered_task_ids` against the new board, so the cards reappear. Changing the view kind similarly causes `useAutoSelectActivePerspective` to re-run and dispatch `perspective.switch` for the new view kind.

**Approach.** Reset the per-window perspective state inside the board-switch boundary so the frontend never observes a (new board, old perspective id, old filtered ids) tuple.

- In `crates/swissarmyhammer-commands/src/ui_state.rs`, extend `UIState::set_window_board` (or add a sibling `switch_window_board`) so that when the new `path` differs from the previous `board_path`, it also clears `windows[label].active_perspective_id` and `windows[label].filtered_task_ids` (set to `None` / cleared) under the same write lock, then a single `try_save()`.
- Confirm `SwitchBoardCmd::execute` in `crates/swissarmyhammer-kanban/src/commands/file_commands.rs` uses the updated path so a switch produces one consistent `UIStateChange`.
- Verify on the frontend that with `active_perspective_id` cleared, `useAutoSelectActivePerspective` (`apps/kanban-app/ui/src/lib/perspective-context.tsx`) picks `matching[0].id` for the new board's view kind and dispatches `perspective.switch`, recomputing `filtered_task_ids` against the new board. No frontend logic change should be required if the backend reset is correct, but if a transient empty render still slips through (e.g., perspective list lags board data), gate the column render on `filtered_task_ids !== undefined` in the same context.

Keep `useSwitchBoardHandler` in `apps/kanban-app/ui/src/components/window-container.tsx` as-is — it already clears `board` and `entitiesByType` eagerly; the backend reset complements that.

## Acceptance Criteria

- [x] Switching boards via `file.switchBoard` clears `windows[label].active_perspective_id` and `filtered_task_ids` for the affected window when the new board path differs from the previous one.
- [x] After a board switch, the new board's cards render in their columns without the user needing to toggle a perspective tab or change view kind.
- [x] Switching back to the previous board still shows that board's cards (the reset must not strand the user with no perspective; auto-select repair path picks the new board's default perspective).
- [x] No regression in single-board first-boot perspective auto-create / auto-select flow.

## Tests

- [x] Backend unit test in `crates/swissarmyhammer-commands/src/ui_state.rs`: extend the existing `set_window_board_and_window_board_round_trip` style tests with a case that seeds `active_perspective_id = "p-old"` and `filtered_task_ids = Some(vec!["t1"])` for `"main"`, calls `set_window_board("main", "/boards/new")`, and asserts both fields are cleared. Add a companion test that calling `set_window_board` with the *same* path does NOT clear the perspective state (idempotent no-op).
- [x] Backend integration test in `crates/swissarmyhammer-kanban/src/commands/file_commands.rs` (or its tests module): drive `SwitchBoardCmd::execute` with a `CommandContext` whose `UIState` has a pre-existing `active_perspective_id` for `"main"`; after execute, read back via `UIState::window_state("main")` and assert `active_perspective_id` is empty and `filtered_task_ids` is `None`.
- [x] Frontend regression test in `apps/kanban-app/ui/src/components/window-container.test.tsx`: simulate `handleSwitchBoard` flow with a stubbed UIState transition where the new board's `active_perspective_id` arrives empty and `perspectives` list is repopulated; assert `perspective.switch` is dispatched for the new board's first matching perspective and that no render frame surfaces a filter referencing stale task IDs.
- [x] Run `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban` and `pnpm --filter kanban-app test` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.