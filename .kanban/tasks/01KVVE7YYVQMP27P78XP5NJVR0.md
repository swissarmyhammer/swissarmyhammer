---
comments:
- actor: wballard
  id: 01kvvewscz969gm53e9fh5dd1n
  text: |-
    Implemented via TDD. Added `#[tokio::test] async fn test_auto_open_board_restores_all_prior_open_boards` to the `#[cfg(test)] mod tests` block in apps/kanban-app/src/state.rs. It creates two valid boards in two TempDirs via `create_board_at`, seeds both `.kanban` paths via `state.ui_state.add_open_board(...)`, drives the production entry point `state.auto_open_board().await` (NOT `restore_persisted_boards()` directly), then asserts `state.boards.read().await.len() == 2` and that `open_boards()` still contains both seeded paths (none pruned).

    RED proof: temporarily commented out `self.restore_persisted_boards().await;` in `auto_open_board` -> test FAILED with "found 1" instead of 2 (CWD/MRU fallback only opens one board), then reverted. Production code in auto_open_board is unchanged (state.rs diff is 52 insertions / 0 deletions, purely the new test).

    Incidental blocker fix: apps/kanban-app/src/command_services.rs had a pre-existing merge artifact -- a duplicate `applies_to: None,` field in 4 test struct literals, which broke compilation of the whole kanban-app test crate ("field `applies_to` specified more than once"). Removed the 4 duplicate lines (4 deletions / 0 insertions) so the test crate compiles. CommandMetadata in swissarmyhammer-command-service/src/types.rs declares `applies_to` exactly once.

    Verification: `cargo nextest run --package kanban-app test_auto_open_board_restores_all_prior_open_boards` -> 1 passed. `command_services::` tests -> 3 passed. `cargo fmt` clean. `cargo clippy -p kanban-app --tests` has NO warnings in my touched files (the only clippy errors are pre-existing, in committed dependency crate swissarmyhammer-focus, outside this task's scope). double-check agent verdict: PASS.
  timestamp: 2026-06-24T00:01:19.263682+00:00
- actor: wballard
  id: 01kvvey7yfwedrjyg1qt0ef2zn
  text: 'Implement landed in review. Added `#[tokio::test] test_auto_open_board_restores_all_prior_open_boards` to apps/kanban-app/src/state.rs (drives production `auto_open_board()` with 2 seeded boards, asserts both load + neither pruned). RED-proof confirmed by commenting out `restore_persisted_boards().await` → test failed (found 1, expected 2), then reverted. Also removed a duplicate `applies_to: None` field from 4 test struct literals in command_services.rs (a pull/merge artifact that broke `kanban-app` test-crate compilation with E0062). Next: /test then /review.'
  timestamp: 2026-06-24T00:02:06.927427+00:00
- actor: wballard
  id: 01kvvfw72qds56ydvd9wa3j2q0
  text: 'Test step GREEN: full `cargo nextest run --package kanban-app` → 187 passed, 1 skipped. Along the way fixed merge-fallout test breakage (all same root cause: the merges added `open_board` Defense-1 validation requiring `boards/board.yaml`, but several test helpers/tests opened bare temp dirs): (1) plugins.rs `open_temp_board` + `open_temp_board_seeded` now call `seed_board(...)` before open; (2) state.rs `test_open_board_serves_full_sah_mcp_toolset` now seeds via `create_board_at` then adds a column+task over MCP (the old in-test `init board` call conflicted with the seeded board). Tester also removed a duplicate `FILE_COMMANDS` decl in builtin/plugins/file-commands/index.ts and duplicate `applies_to` fields in command_services.rs. fmt clean; no new clippy warnings in touched files (the lone kanban-app clippy warning is pre-existing in menu.rs:1388). Next: /review.'
  timestamp: 2026-06-24T00:18:29.079212+00:00
- actor: wballard
  id: 01kvvgs2y2215xgm7sx18hhrgq
  text: |-
    Review resolution:
    - IN-SCOPE finding FIXED: `builtin/plugins/file-commands/index.ts` stale `windowD` comment refs (lines 37–40) → updated to `this.window.window.board.*` to match the live dispatch doc comment.
    - OUT-OF-SCOPE findings DEFERRED to new task ^rhm33fg: the review engine's blocker + warning (`write_user/project_command_plugin` duplication) and 4 nits (hardcoded poll-timeout constants) all target PRE-EXISTING `apps/kanban-app/src/plugins.rs` code NOT in this diff (the reviewer's own driver note confirms this). They are valid standing cleanups, now tracked separately.

    Additional merge-fallout compile repairs made while driving to green (the earlier in-session merges/pull renamed `UIState`→`UiState` but left stale refs; CI-blocking, unrelated to the task but had to compile to verify):
    - crates/swissarmyhammer-command-service/tests/integration/full_baseline_e2e.rs + builtin_ai_commands_e2e.rs: `UIState`→`UiState`.
    - crates/swissarmyhammer-kanban/tests/dynamic_sources_headless.rs: `UIState::new()`→`UiState::new()`.

    Verification (cargo nextest; `cargo test` is forbidden here):
    - `cargo check --workspace --tests` → clean (all merge-fallout compile breaks fixed).
    - `cargo nextest run -p kanban-app` → 187 passed / 1 skipped.
    - `cargo nextest run -p swissarmyhammer-kanban` → 1415 passed / 0 failed.
    - `cargo nextest run -p swissarmyhammer-command-service builtin_file_commands builtin_app_shell` → 4 passed (confirms the tester's file-commands/index.ts rewrite is good).
    - fmt clean; no new clippy warnings in touched files.

    Deliverable verified (RED→GREEN proven). Moving to done.
  timestamp: 2026-06-24T00:34:15.106502+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe080
title: 'Regression guard: prior-open boards load on app start via auto_open_board (multi-board startup)'
---
## What

"Boards not loading on app start" is a recurring regression. The startup board-restore path is **`AppState::auto_open_board`** (`apps/kanban-app/src/state.rs:1235`), invoked once at launch from the production entry point **`apps/kanban-app/src/main.rs:137`** (`tauri::async_runtime::block_on(state.auto_open_board())`). `auto_open_board` reads the persisted open-board list (`self.ui_state.open_boards()`), runs `restore_persisted_boards()` then `restore_window_boards()`, registers each board into `self.boards`, and only falls back to CWD discovery if nothing was restored.

**The test gap:** the existing tests in the `state.rs` `#[cfg(test)] mod tests` (`test_restore_keeps_valid_board_in_config`, `test_restore_prunes_malformed_board_from_config`, `test_restore_keeps_board_on_transient_open_failure`) only drive the lower-level helper `restore_persisted_boards()` directly, and only ever with a **single** board. Nothing exercises the real production entry point `auto_open_board()`, and nothing covers **multiple** persisted boards. A regression in `auto_open_board` itself (the deep-link guard, the call ordering, or the `self.boards.is_empty()` early-return) — or in multi-board restore — would slip through every existing test. That is the exact surface that keeps regressing.

Add a real-pipeline regression test that drives the production `auto_open_board()` entry point with multiple valid persisted boards and asserts they all load.

- File to modify: `apps/kanban-app/src/state.rs` (add a test in the existing `#[cfg(test)] mod tests`; reuse the existing `AppState::new_for_test()`, `create_board_at(parent, name)`, and `state.ui_state.add_open_board(path)` helpers already used by `test_restore_keeps_valid_board_in_config`).
- Seed two distinct valid boards in two separate `TempDir`s, register both paths via `add_open_board`, then call `state.auto_open_board().await` (NOT `restore_persisted_boards()` directly).
- Assert the real registry `state.boards.read().await` contains both boards, and `state.ui_state.open_boards()` still lists both (none pruned).

### Subtasks
- [ ] Add `#[tokio::test] async fn test_auto_open_board_restores_all_prior_open_boards` to the `state.rs` test module.
- [ ] Create two valid boards in two temp dirs with `create_board_at`, seed both `.kanban` paths via `state.ui_state.add_open_board(...)`.
- [ ] Drive the production entry point `state.auto_open_board().await`, then assert both boards are registered in `state.boards.read().await` (len == 2) and both remain in `state.ui_state.open_boards()`.

## Acceptance Criteria
- [ ] A new `#[tokio::test]` drives the production entry point `AppState::auto_open_board()` — not `restore_persisted_boards()` directly.
- [ ] With 2 valid boards seeded into `ui_state.open_boards()`, after `auto_open_board()` the live registry `state.boards.read().await` contains exactly 2 boards (every prior-open board loaded).
- [ ] After restore, `state.ui_state.open_boards()` still lists both seeded paths (no valid board was pruned).
- [ ] The test does not depend on process CWD — seeded boards short-circuit CWD discovery; if any CWD coupling surfaces, isolate with the repo's existing `CurrentDirGuard` / `serial_test` pattern rather than changing production code.

## Tests
- [ ] Add `test_auto_open_board_restores_all_prior_open_boards` in `apps/kanban-app/src/state.rs`.
- [ ] Run `cargo test -p kanban-app test_auto_open_board_restores_all_prior_open_boards` → the new test passes.
- [ ] Confirm it is a genuine regression guard: temporarily stub `auto_open_board` to skip `restore_persisted_boards()` (or short-circuit before restore) and verify the new test FAILS (RED), then revert.

## Workflow
- Use `/tdd` — write the failing test first, watch it fail (RED), then ensure the production path makes it pass (GREEN). Since this is a coverage-gap guard on existing behavior, prove RED by breaking the restore call as above before reverting. #regression #test

## Review Findings (2026-06-23 18:19)

### Blockers
- [ ] `apps/kanban-app/src/plugins.rs:1047` — `write_user_command_plugin` and `write_project_command_plugin` are near-duplicates: both create a plugin directory, write identical TypeScript plugin code to `index.ts`, and differ only in how the plugins directory is resolved and error message strings. The nearly-identical code should be extracted into a single parameterized function. Consolidate into a single `fn write_command_plugin(plugins_dir: &std::path::Path, id: &str, command_id: &str)` function that both call sites invoke with the appropriate plugins directory. This eliminates drift risk and maintenance burden.

### Warnings
- [ ] `apps/kanban-app/src/plugins.rs:1680` — `write_project_command_plugin` and `write_user_command_plugin` are parallel code paths with nearly identical implementations that differ only in how the root plugins directory is sourced and error message prefixes. These should be unified into a single function parameterized by the root directory, expressing the variation as data rather than maintaining duplicate code paths in lockstep. Extract a single function `fn write_command_plugin(plugins_root: &Path, id: &str, command_id: &str)` and call it with the computed/passed root path from both locations, eliminating the duplicate template code.
- [ ] `builtin/plugins/file-commands/index.ts:112` — Comment references deleted variable `windowD` that no longer exists in the code. The refactoring renamed `windowD` to `window` (line 298), but the backend routing documentation at lines 112–114 still mentions the old variable name, creating a mismatch between documented and actual implementation. Update lines 112–114 to reference `window` instead of `windowD`: change `(windowD.window.window.board.switch)` to `(window.window.window.board.switch)`, etc.

### Nits
- [ ] `apps/kanban-app/src/plugins.rs:305` — Hardcoded timeout 20ms for event loop polling should be a named constant. This value configures the sleep interval in `wait_for_available`. Extract to a module-level constant: `const EVENT_POLL_MS: u64 = 20;`.
- [ ] `apps/kanban-app/src/plugins.rs:340` — Hardcoded timeout 100ms for polling sleep interval should be a named constant. This value configures the wait duration in polling helper functions. Extract to a module-level constant: `const POLL_INTERVAL_MS: u64 = 100;`.
- [ ] `apps/kanban-app/src/plugins.rs:375` — Hardcoded timeout 300ms for OS file watcher registration settlement should be a named constant. This value appears multiple times and configures polling test behavior. Extract to a module-level constant: `const OS_WATCHER_SETTLE_MS: u64 = 300;`.
- [ ] `apps/kanban-app/src/plugins.rs:540` — Hardcoded timeout 50ms should be a named constant. This configures the sleep interval during weak handle upgrade polling. Extract to a module-level constant or reuse the polling interval constant.

### Reviewer driver note (scope context, not an engine finding)
The engine scanned whole files, not just the changed hunks. Verified against `git diff`: the **only** working-tree edits to `apps/kanban-app/src/plugins.rs` are two `seed_board(...)` insertions (+8/-0) in the test helpers `open_temp_board` / `open_temp_board_seeded`. The blocker and all four nits above target **pre-existing** `plugins.rs` code (`write_*_command_plugin`, hardcoded poll timeouts) that is **not part of this diff** — they are valid standing cleanups but out of scope for this task. The `windowD`-comment warning is real and in a file this diff touched (`builtin/plugins/file-commands/index.ts`), though the live stale references are at the `// file.*Board → window …` comment block (lines ~37–40), not line 112; this diff itself only removed the duplicate `FILE_COMMANDS` declaration. No engine finding implicates the new regression test or the seeding repairs themselves — those are faithful to each test's original intent and `open_board`'s Defense-1 `boards/board.yaml` requirement is correct product behavior the helpers must satisfy (not a product bug).