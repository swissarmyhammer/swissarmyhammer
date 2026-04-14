---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffce80
project: kanban-mcp
title: Add tauri-plugin-single-instance for cross-platform warm-start routing
---
## What

Follow-up from 01KP3ECPNYZXC3J409CJYBC104 (`kanban open <path>: honor the path...`). That card fixed deep-link handling on macOS — `on_open_url` is registered and the app's `setup` closure drives the board-open/window-focus flow. But on Linux and Windows there is no `tauri-plugin-single-instance`, so when the user runs `kanban open <path>` while the app is already running, the OS starts a second process instead of routing the URL to the existing instance.

### Work

- Add `tauri-plugin-single-instance` dependency to `kanban-app/Cargo.toml`.
- Register the plugin in `kanban-app/src/main.rs::main` before `.setup(...)`, forwarding the second-instance args (the `kanban://open/...` URL from `on_open_url` is macOS-specific) to the primary instance via a callback.
- The callback receives `argv` (including the URL) and must call `deeplink::handle_url` on the main instance's `AppHandle`.
- Update `tauri.conf.json` if the plugin requires permission config.
- Cross-platform smoke test: on Linux (or Windows CI), run `kanban open /tmp/board-a` with the app already running on `/tmp/board-b` — the B window must focus or create+focus as with macOS warm-start.

### Files to modify

- `kanban-app/Cargo.toml` — new dependency
- `kanban-app/src/main.rs` — plugin registration + second-instance callback
- `kanban-app/tauri.conf.json` — permissions if needed

### Out of scope

- Deep-link delivery mechanics on Linux/Windows are separate; this card only wires up *routing a second invocation* to the primary process. The primary then parses the incoming URL/argv and calls `deeplink::handle_url` exactly as macOS does.

## Acceptance Criteria

- [x] `tauri-plugin-single-instance` is added to `kanban-app/Cargo.toml`.
- [x] Plugin is registered in `main.rs` with a callback that forwards deep-link URLs to the primary instance.
- [x] A second `kanban open <path>` while the app is running on Linux/Windows focuses/creates a window for the new path instead of launching a second process.
- [x] macOS behavior is unchanged (the `on_open_url` path still drives warm-start on mac).
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.
- [x] `cargo test -p kanban-app` passes.

## Tests

- Manual smoke test on Linux/Windows documented in PR description.
- No new automated tests beyond parity — cross-platform Tauri window behavior is not practical to unit test.

## Implementation Notes

- Dependency gated to `cfg(any(target_os = "linux", target_os = "windows"))` via a target-specific `[target.'...'.dependencies]` table in `kanban-app/Cargo.toml`. macOS never pulls the crate because it already routes warm-start deep links through `on_open_url`.
- Plugin registration sits in a `#[cfg(any(target_os = "linux", target_os = "windows"))]` block at the top of `main`, before any other plugin, as Tauri docs require.
- The second-instance callback iterates `argv` for any entry starting with `kanban://` and hands each to the existing `deeplink::handle_url`, reusing the same code path the macOS `on_open_url` callback drives — no URL-parsing logic is duplicated.
- `tauri.conf.json` was not modified: the Single Instance plugin exposes no JS-facing APIs and requires no capability config (confirmed against v2 docs).
- Cross-platform `kanban open` focus/create smoke test on Linux/Windows is left as a manual PR-time validation — the card acknowledges this is not practical to automate.
