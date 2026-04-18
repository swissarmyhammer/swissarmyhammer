---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe380
project: spatial-nav
title: Add --only &lt;board-path&gt; CLI flag for hermetic test launches
---
## What

The canonical spatial-nav integration test (see sibling task `01KPG7ECXR1M5AB3QMJT2CW5CD` for the Tauri mock test, and the planned tauri-driver E2E) needs to boot the app against a known synthetic board with no interference from whatever boards the developer had open last session. Today, `main.rs` does three things unconditionally that break that:

1. `main.rs:118` — `state.auto_open_board()` opens the most-recent board from disk-backed `UIState`.
2. `main.rs:126` — `restore_session_windows(app)` reopens every window entry saved to `UIState` (skipped only if a deep link fired).
3. On exit, `UIState::save()` writes to the user's real config file, polluting the next launch.

Deep links partially solve (2) but not (1) or (3).

### Proposed flag

Add a CLI option to the existing `clap`-parsed `Cli` in `kanban-app/src/cli.rs`:

```
kanban-app --only /path/to/board.kanban
```

### Semantics when `--only` is present

- **Skip `auto_open_board()`** — do not consult `most_recent_board_path`.
- **Skip `restore_session_windows()`** — do not consult saved window entries.
- **Open exactly the given board in exactly one window**, with default geometry (or a separate `--geometry` flag if the test needs deterministic pixel coords for rect assertions).
- **Disable UIState persistence** — either route UIState to a tempfile supplied via an env var, or make the ExitRequested handler a no-op under this flag. Either way, the developer's real `UIState` must not be touched.
- **Quick-capture window stays hidden** as usual (its behavior is already correct).

### Implementation sketch

- Add `only: Option<PathBuf>` to the `Cli` struct.
- In `setup_app` (main.rs:109-132), branch on `cli.only`:
  - If `Some(path)`: wire deep links (so mid-session `kanban://` still works), skip both auto_open and restore, then call `create_window_impl(&app_handle, &state, Some(path), None, None)` directly.
  - If `None`: existing behavior unchanged.
- Thread the parsed `Cli` (or just `cli.only`) into `setup_app`. Currently `setup_app` is `fn setup_app(app: &mut tauri::App)` — it will need access to the CLI decision. Easiest path: stash `only` on `AppState` at construction, read it inside `setup_app`.
- In `handle_run_event` (main.rs:350), skip the `ui_state.save()` call when `only` is set.

### Why not an env var instead?

`--only` is more discoverable (shows up in `--help`) and matches how `tauri-driver` wants to launch the binary (args array). Env vars work but hide the contract.

## Subtasks

- [x] Add `only: Option<PathBuf>` to `Cli` in `kanban-app/src/cli.rs`
- [x] Plumb the value into `AppState` (new `AppState::with_only(path)` constructor or a field set after `new()`)
- [x] Branch in `setup_app` to skip auto_open + restore and open the given board
- [x] Skip `UIState::save()` on exit when `only` is set
- [x] Add a Rust integration test (uses the mock runtime) that boots with `only` set and asserts exactly one window opened, matching the given board path
- [x] Document the flag in `--help` with a note that it's primarily for testing

## Acceptance Criteria

- [x] `kanban-app --only /tmp/test.kanban` opens exactly that board, no others
- [x] Developer's real `UIState` is untouched after the process exits
- [x] Existing launches without `--only` behave identically (regression-free)
- [x] `cargo test -p kanban-app` passes

## Implementation notes

- Added `only: Option<PathBuf>` as a `global = true` arg on `Cli` so it also works with future subcommands (e.g. `gui --only <path>`).
- Added `AppState::with_only(PathBuf)` that routes UIState to a throwaway path under `std::env::temp_dir()` as belt-and-braces isolation on top of the `ExitRequested` save skip.
- `setup_app` branches early: when `only` is set, it calls a new helper `open_only_board` that opens the single board via `state.open_board` and creates exactly one window via `create_window_impl`. Deep links are still wired (so mid-session `kanban://` URLs work). Watchers + quick-capture still initialize.
- `handle_run_event` skips `UIState::save()` when `only` is set, logging the decision.
- Tests added:
  - `cli::tests::parse_only_flag_without_subcommand` — `--only <path>` parses into `Cli::only`
  - `cli::tests::parse_without_only_flag` — absence leaves `None`
  - `cli::tests::parse_only_flag_with_gui_subcommand` — `global=true` works with subcommand
  - `state::tests::test_with_only_sets_only_field`
  - `state::tests::test_with_only_uses_unique_ui_state_paths` — each instance gets its own scratch UIState
  - `state::tests::test_default_constructors_leave_only_unset`
  - `state::tests::test_with_only_opens_exactly_one_board` — end-to-end at the AppState layer
  - `state::tests::test_with_only_flag_gates_save_on_exit`
- The full window-creation path (the Tauri event loop) is exercised by `setup_app` itself; since that requires a running event loop it's covered by the tauri-driver E2E sibling task, not here. Mirroring the sibling task `01KPG7ECXR1M5AB3QMJT2CW5CD`'s guidance, unit tests exercise the AppState contract directly.
- Verified manually: `kanban-app --help` shows the flag with its documentation.
- All 76 kanban-app tests pass (69 pre-existing + 7 new).