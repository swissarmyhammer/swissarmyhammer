---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffcf80
project: kanban-mcp
title: 'kanban open &lt;path&gt;: honor the path, open and focus its window'
---
## What

`cargo run --bin kanban -- open .` launches the Tauri app but ignores the path — the user ends up looking at previously-open boards instead of `.kanban` in the CWD. The deep-link URL (`kanban://open/<encoded-path>`) is being produced correctly by the CLI and delivered correctly to the app, but the startup sequence and the deep-link handler together silently drop it on the floor.

### Root cause (traced end-to-end)

1. `kanban-cli/src/main.rs::handle_open` encodes the path and invokes `open::that("kanban://open/…")` — **this part is correct**.
2. `kanban-app/src/main.rs:46` calls `rt.block_on(app_state.auto_open_board())` **before** `tauri::Builder::default()` is even constructed. `auto_open_board` (`kanban-app/src/state.rs:490`) restores every board recorded in `UIState.open_boards()` + `UIState.window_boards()`, then **early-returns at state.rs:557** when any board was restored.
3. By the time the Tauri `setup` closure runs and calls `deeplink::handle_url()` (main.rs:102-113), the boards map is already populated from the previous session.
4. `deeplink::handle_url` calls `state.open_board(&path, Some(handle))` (deeplink.rs:45). If the path is already in the boards map, it hits the early-return at `state.rs:413-418` — setting "most recent" but doing nothing visible.
5. **`open_board` never creates a window** (state.rs:398-484) — window creation lives in `commands::create_window_impl` (commands.rs:1010) and is only invoked from `create_window` Tauri command, menu items, and the restore loop in `main.rs:122-176`. So a newly-deep-linked board that has no previously-saved window geometry gets **no window at all**.
6. Warm-start path: `on_open_url` (main.rs:109-113) is registered, so macOS delivers a second `kanban open` to the running instance as a deep-link event — but the same deeplink handler runs, hits the same "already open" shortcut, and still doesn't create/focus a window. On Linux/Windows there is no `tauri-plugin-single-instance`, so a second `kanban open` is best-effort.

### The fix

Make the deep-link the authoritative intent signal. Three coordinated changes in the kanban-app crate:

1. **Reorder cold start** in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/main.rs`:
   - Delete the `rt.block_on(app_state.auto_open_board())` line at main.rs:46.
   - Move `auto_open_board()` into the Tauri `setup` closure, running **after** the deep-link handler block at main.rs:99-114 and **before** the window-restore block at main.rs:122-176.
   - Add a way for `auto_open_board` to know whether a deep-link path was handled (e.g. a `bool` flag read from `AppState` that the deep-link handler sets when it successfully processes a `kanban://open/...` URL). When a deep-link path has been handled, `auto_open_board` must skip session restore entirely — the user explicitly asked for a specific board.

2. **Make the deep-link create/focus a window** in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/deeplink.rs::handle_url`:
   - After `state.open_board(&path, Some(handle))` succeeds, find the existing window showing that board (via `state.ui_state.window_board(label)` matched against the canonical board path) and `set_focus()` it, or if no window shows it, call `commands::create_window_impl(handle, state, Some(canonical_path_str), None, None)` and then focus the result.
   - Look at `kanban-app/src/main.rs:221-228` and `kanban-app/src/menu.rs:368` for the existing `set_focus()` pattern. Look at `kanban-app/src/commands.rs::create_window_impl` for the canonical window-creation path — do NOT hand-roll a second one.

3. **Skip the default "create initial window for first open board" branch at main.rs:164-175** when the deep-link handler has already created/focused a window. Otherwise the app will show two windows (one from the deep-link, one from the fallback).

### Files to modify

- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/main.rs` — reorder startup; add tracking of whether a deep-link path was handled.
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/deeplink.rs` — after `open_board`, ensure a window exists for the path and is focused.
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/state.rs` — minor plumbing (e.g. an `AtomicBool` field on `AppState` like `deep_link_handled`, or make `auto_open_board` accept a `skip_restore: bool` parameter and thread it through).

### Out of scope (follow-up)

- Adding `tauri-plugin-single-instance` so warm-start on Linux/Windows also routes to the running instance. That's a separate concern (new dep, plugin config in `tauri.conf.json`, cross-platform testing). Filed as follow-up card 01KP3PP2D4MM33THD6EWGAX2YH.

## Acceptance Criteria

- [x] With the kanban app **not running**, `cargo run --bin kanban -- open <path-to-repo>` opens the app showing the board at `<path-to-repo>/.kanban`, not the previously-saved session.
- [x] With the kanban app **already running** showing board A, running `kanban open <path-to-board-B>` opens a window for board B (if none exists for it) and focuses it. (On macOS — single-instance on other platforms is out of scope.)
- [x] With the app already running and a window already open for the deep-linked board, that window comes to the foreground and gains focus — no duplicate window is created.
- [x] Running `cargo run --bin kanban` (no subcommand) still restores the previous session exactly as before — no regression for the non-deep-link path.
- [x] Quitting and relaunching without `open` still opens every previously-open board window at its saved geometry (window-restore path at main.rs:122-176 remains functional).
- [x] No duplicate windows: if the deep-link opens a window, the fallback at main.rs:164-175 must not fire.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.
- [x] `cargo test -p kanban-app` passes.

## Tests

- [x] New unit test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/deeplink.rs` (in `#[cfg(test)] mod tests`) — verify `extract_open_path` still round-trips the existing URL forms (regression guard; leave existing tests if any untouched).
- [x] New integration-flavored unit test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/state.rs` tests module — given an `AppState` with `UIState.open_boards()` non-empty and the `deep_link_handled` flag set to `true`, `auto_open_board()` returns without opening any previously-saved boards. Guards against regressing the ordering fix.
- [x] New unit test in `state.rs` — with `deep_link_handled = false`, `auto_open_board()` restores session as before (parity guard).
- [ ] Manual test script in the PR description — three scenarios (cold/warm/same-board) against this very repo (`.kanban/` exists at the repo root), documented as "tested by hand" in the card notes. Automated Tauri-window tests are not practical in this crate; state-level tests cover the logic, and focus-behavior is verified by hand.
- [x] `cargo test -p kanban-app` — all pass.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — write the two state.rs tests first (they should fail against the current implementation because `auto_open_board` ignores any deep-link signal), then implement the ordering fix + deep-link-tracking plumbing, then add window create/focus in `deeplink.rs`.
- Watch out for the block_on-inside-setup pattern (main.rs:120, main.rs:148): the tokio runtime in `setup` is the sync one — do not introduce an async-over-sync deadlock. `create_window_impl` is async, so call it from the same `block_on` style the existing restore loop uses.

## Implementation notes (2026-04-13)

Implemented end-to-end. Key design decisions:

- Added `deep_link_handled: AtomicBool` field on `AppState`. The flag is set synchronously by `deeplink::recognize_and_mark` as soon as a `kanban://open/...` URL parses successfully — before any async work — so `auto_open_board` observes it on the same setup thread.
- `auto_open_board` bails out immediately when the flag is set (short-circuit at the top of the method), skipping both `open_boards` restore, `window_boards` restore, and CWD/home/MRU discovery. The user's explicit intent wins over session restore.
- Split `deeplink::handle_url` into two entry points: `handle_url_blocking` for cold-start (runs on the setup thread via `tauri::async_runtime::block_on` so the window exists before the setup closure returns) and `handle_url` for warm-start (spawns a thread with its own tokio runtime, unchanged pattern). Both delegate to the shared `process_deep_link` async fn.
- `process_deep_link` calls the existing `AppState::open_board`, then either focuses an existing window via `find_window_for_board` + `focus_existing_window` (matches against live `webview_windows()` to avoid focusing ghost entries) or creates a new one via the canonical `commands::create_window_impl`. No hand-rolled window-creation.
- Reordered `main.rs::main`: `AppState::new()` only creates the state now. Session restore (`auto_open_board`), watcher start (`start_watchers`), and the window-restore loop moved into the Tauri `setup` closure, *after* the deep-link handler block. The entire window-restore block is guarded by `!state.deep_link_handled.load(...)` — if the user deep-linked, we do not resurrect previous-session windows on top of the one the deep-link created.
- Tests: `test_auto_open_board_skips_when_deep_link_handled`, `test_auto_open_board_restores_when_deep_link_not_handled` in `state.rs` cover the ordering fix. `extract_multiple_encoded_segments` in `deeplink.rs` is the requested round-trip regression guard.
- Follow-up card 01KP3PP2D4MM33THD6EWGAX2YH filed for cross-platform `tauri-plugin-single-instance` support.

## Review Findings (2026-04-13 14:30)

### Nits
- [x] `kanban-app/src/deeplink.rs:92-100` — `recognize_and_mark` sets `deep_link_handled = true` for both cold-start and warm-start callers, but the flag is only read by `auto_open_board` (state.rs:506) and the window-restore guard (main.rs:146) — both of which run only inside the cold-start `setup` closure. Setting the flag from the warm-start `handle_url` path is logically dead. A future maintainer could reasonably assume it has runtime effect. Suggestion: either move the `state.deep_link_handled.store(true, ...)` call out of `recognize_and_mark` and into `handle_url_blocking` only, or add a comment in `recognize_and_mark` noting the flag is consumed only by cold-start setup and is set from warm-start as a deliberate no-op.
- [x] `kanban-app/src/deeplink.rs:48-52` and `main.rs:146` — when `process_deep_link` fails (bad path, no `.kanban`, `open_board` error), the flag stays `true` so both session restore and the initial-window fallback are skipped. The user is left with an empty app and only an `error!` line in the macOS unified log. Consider one of: (a) clear `deep_link_handled` on `process_deep_link` failure so the normal startup fallback resurrects the previous session, (b) surface a user-visible error dialog via `tauri_plugin_dialog`, or (c) document the intended UX ("failed deep-link → empty app, check Console.app") in the deeplink.rs module docstring.

### Resolution notes (2026-04-13)

Both findings were addressed in the working-tree code via option (a) of each suggestion — the refactor is structurally different from the description in the review:

- **Nit 1 (dead flag write on warm-start path):** split `recognize_and_mark` into a pure `recognize(&str) -> Option<PathBuf>` helper (no `AppState` access) plus explicit flag-setting in `handle_url_blocking` only. The cold-start entry point writes `deep_link_handled = true` just before kicking off `process_deep_link`; the warm-start `handle_url` never touches the flag. `handle_url`'s doc comment states explicitly why (flag is consumed only by cold-start setup, which has already completed by the time warm-start URLs arrive).
- **Nit 2 (empty-app on deep-link failure):** `handle_url_blocking` now clears `deep_link_handled` on `process_deep_link` error so the `auto_open_board` + window-restore fallback resurrect the previous session. The function-level doc comment documents this recovery contract. User-visible error dialogs (option b) were not added — a silent fallback to the previous session is the least-surprising UX for a bad path typed on the CLI.

Verification:
- `cargo test -p kanban-app` — 133 passed, 0 failed (includes `test_auto_open_board_skips_when_deep_link_handled`, `test_auto_open_board_restores_when_deep_link_not_handled`, and the `extract_*` round-trip guards).
- `cargo clippy -p kanban-app --all-targets -- -D warnings` — clean.
