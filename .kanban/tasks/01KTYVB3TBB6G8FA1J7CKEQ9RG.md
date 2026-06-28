---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffad80
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
- [x] Auto-restore of a malformed board dir: entry pruned from config with a warn log; app starts with the remaining valid boards; window falls back sanely
- [x] Explicit open of a malformed dir: clear error (or documented self-heal), never a persisted half-open
- [x] One bad board cannot blank a window bound to it (error state + recovery path visible)
- [x] Reconciler/side-effects run only post-validation
- [x] Root cause of the 05:07 creation identified and closed (or documented as external)

## Tests
- [x] Rust: restore_persisted_boards with a malformed dir → pruned + warn (red first); open_board on malformed dir → rejected/self-healed per the decision
- [x] Rust: reconciler does not run for rejected opens
- [x] vitest: refreshBoards with get_board_data failing → error state, not silent null; other boards still load
- [x] Crate-scoped suites green

## Constraints
- NO whole-workspace cargo build/clippy; no kanban-app crate compile (write app-crate tests to compile on next rebuild; note it). Never touch .kanban/actors/wballard.jsonl.

## Workflow
- /tdd.

## Review Findings (2026-06-13 15:55)

Verification re-run fresh this session: kanban `1297/1297` PASS, vitest `38/38` PASS (refresh + window-container + rust-engine-container), `tsc --noEmit` exit 0. Red-green probe confirmed: reverting the `board_entity_exists` guard in `KanbanContext::open` makes `open_of_malformed_board_does_not_mint_perspectives` fail (left:1 minted, right:0 expected); guard restored → green. No stray `.kanban` created under `apps/` during the run.

All four defenses verified in source:
- Defense 1 — `state.rs` `open_board_with` calls `KanbanContext::board_entity_exists(&kanban_path)` BEFORE `BoardHandle::open_with` (which runs `KanbanContext::open` + reconciler); rejection error is actionable.
- Defense 2 — `restore_persisted_boards` now prunes via `remove_open_board` on open FAILURE (see warning below re: error discrimination).
- Defense 3 — `refresh.ts` splits `get_board_data` from the 3 `list_entities`, gates the entity build on `bd`, surfaces `boardError: string|null`; `window-container.tsx` `recoverFromBoardErrorIfNeeded` adopts a healthy fallback board.
- Defense 4 — reconciler guarded on `board_entity_exists` inside `KanbanContext::open`; genuine second layer for direct callers (agent-cwd case). Single-source predicate: `is_initialized()` also delegates to it.

### Warnings
- [x] `apps/kanban-app/src/state.rs` `restore_persisted_boards` — the prune fires on ANY `open_board` error, not specifically the malformed-structure rejection. `BoardHandle::open_with` (production path) binds a TCP port for the per-board MCP server and constructs an FSEvents watcher; a transient failure there (port contention, momentary I/O error, lock contention) on a *valid* persisted board would permanently drop it from `open_boards` config — the "a real board that momentarily fails to open shouldn't be forgotten" hazard the card itself flagged. Severity is a warning, not a blocker, because (a) the card's acceptance criterion says "entries that fail to open" → prune (the impl follows it literally), and (b) the board survives in MRU `touch_recent` history and re-adds to `open_boards` on any subsequent manual open (`register_open_board` → `add_open_board`), so it is recoverable, not lost data. Suggested hardening (follow-up, not required for this card): distinguish the structural rejection (the `board_entity_exists` `Err`) from downstream open errors and prune only on the former, leaving transient failures persisted for retry.
  - RESOLVED (2026-06-13): `open_board_with` now returns a typed `OpenBoardError { Malformed(String), Transient(String) }`. Defense 1 constructs `Malformed`; every other error (path resolution, `BoardHandle::open_with` port-bind / FSEvents / IO / lock) flows in via `From<String>` as `Transient`. The two public wrappers (`open_board`, `open_board_for_test`) flatten back to `String` so all existing callers are unchanged. `restore_persisted_boards` now calls `open_board_with` directly and branches on `OpenBoardError::is_malformed()`: PRUNE only the malformed-structure rejection; KEEP (warn-log, retry next launch) on transient. Covered by new state.rs tests `test_open_board_error_discriminates_malformed_from_transient`, `test_open_board_error_from_string_is_transient`, `test_restore_keeps_board_on_transient_open_failure` (WRITTEN-NOT-RUN — kanban-app crate intentionally not compiled per constraints).

### Nits
- [x] `apps/kanban-app/src/state.rs` (state.rs test module: `test_open_malformed_board_is_rejected`, `test_restore_prunes_malformed_board_from_config`, `test_restore_keeps_valid_board_in_config`) — these exercise the real production `open_board_for_test`/`restore_persisted_boards` paths and are structurally sound by inspection, but remain UNRUN this cycle (kanban-app crate intentionally not compiled per constraints). The load-bearing core predicate (`board_entity_exists` gating the reconciler) IS covered by a RUN test (`crates/swissarmyhammer-kanban/tests/malformed_board_open.rs`, with a verified red-green), so the residual risk is confined to the app-side WIRING (open_board_with calling the check; restore pruning on failure), not the predicate. Confirm these three go green on the next CI/rebuild that compiles the kanban-app crate.
  - CONFIRMED/NOTED (2026-06-13): these three tests — plus the three new discrimination tests added for the warning fix — remain WRITTEN-NOT-RUN this cycle by constraint (kanban-app crate not compiled). They are expected to go GREEN on the next CI/rebuild that compiles the kanban-app crate. The run-tested predicate layer (`board_entity_exists`, `crates/swissarmyhammer-kanban/tests/malformed_board_open.rs`) is GREEN this session (1297/1297). Residual unrun-coverage is confined to the app-side WIRING and the new `OpenBoardError` malformed-vs-transient discrimination, both of which live only in the kanban-app crate and cannot be run without compiling it.

### Notes (no action)
- 05:07 root cause documented as external/agent-cwd (a direct `KanbanContext::open` with cwd=apps/kanban-app, no app path produces it); the broad `create_dir_all` in `KanbanContext::open` was deliberately left unchanged due to blast radius. This matches the agent-cwd-stray-kanban / session-cwd-for-tools project memory and is a reasonable call — Defense 4 (the reconciler guard) neutralizes the harmful side effect even on that direct-construction path, so the eager `create_dir_all` no longer mints board state into a malformed dir.