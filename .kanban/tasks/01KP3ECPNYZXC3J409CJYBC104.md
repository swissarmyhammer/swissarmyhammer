---
assignees:
- claude-code
position_column: todo
position_ordinal: b580
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

- Adding `tauri-plugin-single-instance` so warm-start on Linux/Windows also routes to the running instance. That's a separate concern (new dep, plugin config in `tauri.conf.json`, cross-platform testing). File as a follow-up card.

## Acceptance Criteria

- [ ] With the kanban app **not running**, `cargo run --bin kanban -- open <path-to-repo>` opens the app showing the board at `<path-to-repo>/.kanban`, not the previously-saved session.
- [ ] With the kanban app **already running** showing board A, running `kanban open <path-to-board-B>` opens a window for board B (if none exists for it) and focuses it. (On macOS — single-instance on other platforms is out of scope.)
- [ ] With the app already running and a window already open for the deep-linked board, that window comes to the foreground and gains focus — no duplicate window is created.
- [ ] Running `cargo run --bin kanban` (no subcommand) still restores the previous session exactly as before — no regression for the non-deep-link path.
- [ ] Quitting and relaunching without `open` still opens every previously-open board window at its saved geometry (window-restore path at main.rs:122-176 remains functional).
- [ ] No duplicate windows: if the deep-link opens a window, the fallback at main.rs:164-175 must not fire.
- [ ] `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.
- [ ] `cargo test -p kanban-app` passes.

## Tests

- [ ] New unit test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/deeplink.rs` (in `#[cfg(test)] mod tests`) — verify `extract_open_path` still round-trips the existing URL forms (regression guard; leave existing tests if any untouched).
- [ ] New integration-flavored unit test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-cli/kanban-app/src/state.rs` tests module — given an `AppState` with `UIState.open_boards()` non-empty and the `deep_link_handled` flag set to `true`, `auto_open_board()` returns without opening any previously-saved boards. Guards against regressing the ordering fix.
- [ ] New unit test in `state.rs` — with `deep_link_handled = false`, `auto_open_board()` restores session as before (parity guard).
- [ ] Manual test script in the PR description — three scenarios (cold/warm/same-board) against this very repo (`.kanban/` exists at the repo root), documented as "tested by hand" in the card notes. Automated Tauri-window tests are not practical in this crate; state-level tests cover the logic, and focus-behavior is verified by hand.
- [ ] `cargo test -p kanban-app` — all pass.
- [ ] `cargo clippy -p kanban-app --all-targets -- -D warnings` — clean.

## Workflow

- Use `/tdd` — write the two state.rs tests first (they should fail against the current implementation because `auto_open_board` ignores any deep-link signal), then implement the ordering fix + deep-link-tracking plumbing, then add window create/focus in `deeplink.rs`.
- Watch out for the block_on-inside-setup pattern (main.rs:120, main.rs:148): the tokio runtime in `setup` is the sync one — do not introduce an async-over-sync deadlock. `create_window_impl` is async, so call it from the same `block_on` style the existing restore loop uses.
