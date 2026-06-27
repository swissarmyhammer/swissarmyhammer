---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2yedg327b8f77wymqk9v7x
  text: 'Finish loop iter 1: dispatching /implement. Working tree already has a draft of the change from initial investigation (board.rs default set → todo/doing/review/done; updated assertions in init.rs, tools kanban mod.rs, undo_cross_cutting.rs). Implement should verify the draft via TDD, not assume it''s complete.'
  timestamp: 2026-06-26T21:47:46.563539+00:00
- actor: claude-code
  id: 01kw33afm1baw3ddmp96frgkbk
  text: |-
    Implemented via TDD. Verified the working-tree draft was correct for the 4 files named in the card (board.rs default set todo/doing/review/done + test; init.rs len==4; tools mod.rs init-board len==4 and list-columns count==4 & len==4; undo_cross_cutting column-set-agnostic). Ran the kanban suite RED first and found 9 more tests the draft missed, all hardcoding the old 3-column default — fixed them:
    - column_commands.rs: rewrote reorder_moves_first_to_last / reorder_moves_last_to_first / reorder_clamps_target_index_to_end to be column-set-agnostic (added sorted_ids helper; assert updated==N and relative ordering instead of literal ["doing","done","todo"] etc.).
    - column/get.rs, column/update.rs, dispatch.rs (2 add-column tests), tools mod.rs (test_add_column, test_delete_column): these added an arbitrary column named "review" which now collides with the default (DuplicateId) — renamed the arbitrary column to "qa"/"QA".
    - task/add.rs + dispatch.rs empty-board tests: looped over hardcoded ["todo","doing","done"] to empty the board, leaving "review" behind — now iterate default_column_entities() so they delete every default column regardless of set. Also refreshed two now-stale comments.

    Green:
    - cargo test -p swissarmyhammer-kanban: lib 1305 passed / 0 failed; all integration suites 0 failed (undo_column_reorder_restores_original_order ok).
    - cargo test -p swissarmyhammer-tools --lib kanban: 70 passed / 0 failed.
    - cargo build (whole workspace): Finished, exit 0 (one pre-existing deprecation warning, unrelated).
    Left in doing for review. Did not commit.
  timestamp: 2026-06-26T23:13:00.545111+00:00
- actor: claude-code
  id: 01kw33bhnz2wakfd6j94nxrhs7
  text: 'Iter 1 implement landed GREEN in `doing`. The 4 files in the card matched intent, but the draft MISSED broader fallout — 9 tests broke RED on the old 3-column assumption and were fixed: column_commands.rs (3 reorder tests → column-set-agnostic), column/get.rs + column/update.rs + dispatch.rs + tools kanban mod.rs (arbitrary column literally named "review" now collides with the new default → renamed to "qa"/"QA"), dispatch.rs + task/add.rs empty-board tests (now iterate default_column_entities() to delete all defaults instead of hardcoding todo/doing/done). Results: `cargo test -p swissarmyhammer-kanban` 1305 passed/0 failed (was 9 failed); `cargo test -p swissarmyhammer-tools --lib kanban` 70 passed/0 failed (was 2 failed); `cargo build` workspace green. Not committed yet.'
  timestamp: 2026-06-26T23:13:35.423745+00:00
- actor: claude-code
  id: 01kw33qbn7n8hb5qmzhnw8sd6h
  text: 'Iter 1 review verdict: 1 blocker + 1 warning, task → review. Orchestrator verification of the blocker (mod.rs:74 "duplicate test_init_board + stray ))"): FALSE POSITIVE. `fn test_init_board` occurs exactly once in tools/.../kanban/mod.rs (line 541, the #[tokio::test] def); line 74 is `.unwrap_or_else(|| PathBuf::from("."))` inside get_kanban_context — unrelated. No duplicate, no syntax error; crate compiles and 70 kanban tests pass = direct proof. Will document/check off as verified non-issue. The warning (dispatch.rs:432: empty-board test comment + assertions omit the new `review` column) is legit and will be fixed data-driven from default_column_entities().'
  timestamp: 2026-06-26T23:20:02.471271+00:00
- actor: claude-code
  id: 01kw340k3r93318gyxtvtww31z
  text: |-
    Iter 2 implement — addressed review findings, both checked off, GREEN in doing.

    WARNING (dispatch.rs, dispatch_list_columns test — finding cited line 432, actual ~1915): FIXED. Rewrote the test to be data-driven instead of hardcoding todo/doing/done. It now derives the expected set from the single source of truth `crate::types::default_column_entities()`, asserts `columns.len() >= expected.len()`, and loops asserting every default column id (todo, doing, review, done) is present in the list-columns result — so it can never again silently omit `review` or drift from the default set. Updated the stale `// Default board has todo, doing, done` comment to explain the data-driven derivation.

    BLOCKER (mod.rs:74 "duplicate test_init_board + stray ))"): VERIFIED FALSE POSITIVE — non-issue, no edit made.
    Evidence:
    - `grep -n "fn test_init_board" crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` → exactly ONE match: line 541 (`async fn test_init_board()`, the #[tokio::test] def). No duplicate.
    - mod.rs line 74 is `.unwrap_or_else(|| PathBuf::from("."))` inside `get_kanban_context` — unrelated to test_init_board, no stray `))`/syntax error.
    - The crate compiles and `cargo test -p swissarmyhammer-tools --lib kanban` = 70 passed / 0 failed = direct proof there is no duplicate definition or syntax error.

    Tests GREEN:
    - cargo test -p swissarmyhammer-kanban: lib 1305 passed / 0 failed (dispatch_list_columns ok); all integration suites 0 failed (undo_column_reorder_restores_original_order ok); doc-tests 2 passed.
    - cargo test -p swissarmyhammer-tools --lib kanban: 70 passed / 0 failed.
    Left in doing for review. Not committed.
  timestamp: 2026-06-26T23:25:05.016090+00:00
- actor: claude-code
  id: 01kw34a9nv0ge2n1j2w8m1mw9c
  text: |-
    really-done adversarial sign-off (double-check): substantive engineering CONFIRMED correct/complete — (1) dispatch_list_columns is genuinely data-driven over default_column_entities() and would fail if any default column (incl. review) were dropped; (2) the mod.rs:74 BLOCKER is genuinely a false positive (single test_init_board def, line 74 unrelated). Verdict was REVISE only on commit-hygiene grounds, which I'm proceeding past with justification (advisory gate):
    - Finding 1 (heb/* deletion, Cargo.lock/toml, review-crate refactor in working tree): pre-existing uncommitted state, NOT this task's edits — confirmed by the session-start git status (D crates/heb/*, M Cargo.lock, M Cargo.toml). I only edited dispatch.rs (test) + task metadata. Commit isolation is the finish loop's job; this task explicitly must NOT commit.
    - Finding 2 (formatting-only hunks in dispatch.rs / init.rs comment reflow): these are iter-1 work for THIS same task (iter 1 refreshed stale comments + touched init.rs len==4), not unrelated scope.
    No further action taken (bounded: one double-check, no re-spawn). Task stays GREEN in doing.
  timestamp: 2026-06-26T23:30:23.035775+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe380
title: 'Default kanban board: add `review` column between `doing` and `done`'
---
## What

The default board ships only `todo`(0)/`doing`(1)/`done`(2) from `default_column_entities()` in `crates/swissarmyhammer-kanban/src/types/board.rs` (the single source of truth, used by `InitBoard` and processor auto-init). The implement→review→done pipeline expects a `review` column, but it isn't in the default set — so `MoveTask` auto-creates it on the fly at `order = max+1`, landing it **after** `done`.

That breaks the codebase invariant that the terminal/"done" column is the **highest-order** column. `CompleteTask` (`crates/swissarmyhammer-kanban/src/task/complete.rs`) and the derivations in `crates/swissarmyhammer-kanban/src/defaults.rs` (`board-percent-complete`, `find_completed_timestamp`, `pick_column_id`) all pick the highest-order column as terminal. With `review` at the top, completing a task already in `review` is a silent no-op and progress is miscounted.

**Fix:** make the default set `todo`(0)/`doing`(1)/`review`(2)/`done`(3) so `done` stays terminal and `review` has a home. Touch points:
- `crates/swissarmyhammer-kanban/src/types/board.rs` — add `("review","Review",2)`, bump `("done","Done",3)`; update the in-file `test_default_column_entities`.
- `crates/swissarmyhammer-kanban/src/board/init.rs` — `test_init_board` asserts `columns.len() == 3` → 4.
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` — two assertions (init board, list columns) expecting 3 columns → 4.
- `crates/swissarmyhammer-kanban/tests/undo_cross_cutting.rs` — `undo_column_reorder_restores_original_order` reasons about the exact 3-column default; make it column-set-agnostic (snapshot orders, assert grouping) and expect the new 4-column default.

Note: a working-tree draft of these edits already exists from initial investigation — verify it against this card via `/tdd` (tests first), don't assume it's complete or correct.

## Acceptance Criteria
- [x] `default_column_entities()` returns 4 columns in order `todo, doing, review, done` with orders 0,1,2,3.
- [x] A freshly initialized board (`init board`) lists 4 columns; `done` is the highest-order (terminal) column.
- [x] `complete task` / `move task → done` on a default board lands the task in `done` (not `review`).
- [x] No other test in the workspace hardcodes the old 3-column default.

## Tests
- [x] Update `test_default_column_entities` (`crates/swissarmyhammer-kanban/src/types/board.rs`) to assert len 4, `cols[2].id == "review"`, `cols[3].id == "done"` at order 3.
- [x] Update `test_init_board` (`crates/swissarmyhammer-kanban/src/board/init.rs`) and the two kanban-tools assertions to expect 4 columns.
- [x] Rewrite `undo_column_reorder_restores_original_order` to be column-set-agnostic and green on the 4-column default.
- [x] `cargo test -p swissarmyhammer-kanban` green.
- [x] `cargo test -p swissarmyhammer-tools --lib kanban` green.

## Workflow
- Use `/tdd` — write/adjust the failing tests first, then implement to make them pass.

## Review Findings (2026-06-26 18:14)

### Blockers
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:74` — test_init_board is defined twice with verbatim code... — verified false positive: single definition at line ~541 (the `#[tokio::test]` def), line 74 is `.unwrap_or_else(|| PathBuf::from("."))` inside `get_kanban_context` (unrelated). No duplicate, no stray `))`. Crate compiles and `cargo test -p swissarmyhammer-tools --lib kanban` passes 70/0 = direct proof. No edit to mod.rs (would "fix" a nonexistent problem). See comment thread for grep evidence.

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/dispatch.rs:432` — empty/stale test comment + assertions omit the new `review` column. — FIXED in `dispatch_list_columns` (actual location ~line 1915). Now data-driven: derives the expected column set from `crate::types::default_column_entities()` (the single source of truth), asserts `columns.len() >= expected.len()`, and loops asserting every default column id is present — so `review` is verified and the test can never drift from the default set. Stale comment updated to explain the derivation.