---
assignees:
- claude-code
position_column: todo
position_ordinal: f680
title: A malformed board dir (.kanban without boards/) blanks the whole window — open must reject or self-heal it, refresh must degrade gracefully
---
## What

LIVE INCIDENT (2026-06-12): the app's main window stopped loading boards entirely. Root cause chain:
1. A stray `.kanban` directory appeared at `apps/kanban-app/.kanban` (created 05:07 by an agent/test run with the wrong cwd — it had actors/definitions/entities/perspectives/views/undo_stack but **no `boards/` dir**, hence no board entity).
2. The app (or an op) OPENED that path as a board; it got persisted into UIState `open_boards` + bound to a window via `window_boards`.
3. On every launch, `restore_persisted_boards` (apps/kanban-app/src/state.rs) restored it — the prune only removes entries whose path is NOT a dir; a malformed-but-existing dir passes.
4. The window bound to it called `refreshBoards` → `get_board_data` → `entity not found: board/board` → the whole Promise.all fails → boardData null → blank window ("boards are not loading").
5. Bonus: the perspectives reconciler dutifully minted a Default perspective INTO the malformed board (it ran at board open before anything validated the board).

Immediate mitigation applied by hand: deleted `apps/kanban-app/.kanban`; restart prunes the stale entry.

## Required outcome (defense in depth — all four)

1. **Open validates**: opening a board dir that has no `boards/` board entity must either self-heal (create the board entity the way `init board` does — if the dir is otherwise a plausible board) or REJECT the open with a clear error — never open into a half-board. Decide which (probably: reject for auto-restore paths; the explicit `init board` op is how you create new ones).
2. **Restore prunes malformed boards**: `restore_persisted_boards` must drop (and remove from config, warn-logged) entries that fail to open — today a failed open logs a warning but leaves the entry persisted, so every launch re-fails.
3. **Frontend degrades gracefully**: one board failing `get_board_data` must not blank the window — the refresh should surface an error state for THAT board (and ideally fall back to another open board) instead of swallowing into a null board forever. Split the Promise.all so entity-list failures and board-data failures don't take each other down.
4. **Board-open ordering**: the perspectives reconciler (and any other open-time side effects) must run only AFTER the board is validated — no minting state into malformed dirs.

ALSO INVESTIGATE: what created/opened `apps/kanban-app/.kanban` at 05:07 — if an agent test run with cwd=apps/kanban-app caused a kanban context to init a directory, that creation path is too eager (a `.kanban` should be created by explicit `init board`, not by ambient context construction — cf. session-cwd-for-tools memory). If found, fix the eager creation too.

## Acceptance Criteria
- [ ] Auto-restore of a malformed board dir: entry pruned from config with a warn log; app starts with the remaining valid boards; window falls back sanely
- [ ] Explicit open of a malformed dir: clear error (or documented self-heal), never a persisted half-open
- [ ] One bad board cannot blank a window bound to it (error state + recovery path visible)
- [ ] Reconciler/side-effects run only post-validation
- [ ] Root cause of the 05:07 creation identified and closed (or documented as external)

## Tests
- [ ] Rust: restore_persisted_boards with a malformed dir → pruned + warn (red first); open_board on malformed dir → rejected/self-healed per the decision
- [ ] Rust: reconciler does not run for rejected opens
- [ ] vitest: refreshBoards with get_board_data failing → error state, not silent null; other boards still load
- [ ] Crate-scoped suites green

## Constraints
- NO whole-workspace cargo build/clippy; no kanban-app crate compile (write app-crate tests to compile on next rebuild; note it). Never touch .kanban/actors/wballard.jsonl.

## Workflow
- /tdd.