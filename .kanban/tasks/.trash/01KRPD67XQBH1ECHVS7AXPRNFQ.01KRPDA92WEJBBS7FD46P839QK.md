---
assignees:
- claude-code
depends_on:
- 01KRPD2X34N6AT0J4QRMTP9QSX
position_column: todo
position_ordinal: '8580'
project: cli-in-app
title: '[Deferred] Windows: register the bundled kanban.exe on PATH via the NSIS installer'
---
## What
DEFERRED — do not start until the Windows app build is enabled. The Windows build in `.github/workflows/release-app.yml` is currently a commented-out placeholder (`build-windows`), so this task cannot be CI-verified yet.

The sidecar task already makes `kanban.exe` ship inside the Windows bundle automatically (`externalBin` is cross-platform). What remains is exposing it on the Windows `PATH`:
- Configure the Tauri NSIS installer (`bundle.windows.nsis` in `apps/kanban-app/tauri.conf.json`, plus an NSIS installer hook template if needed) to append the install directory — or a dedicated `bin` directory containing `kanban.exe` — to the system or user `PATH` environment variable on install, and remove it on uninstall.
- Decide system vs. user PATH (user PATH avoids requiring elevation; matches the macOS "just happens" intent).
- The macOS launch-time self-install (`src/cli_install.rs`) is macOS-specific; Windows relies on the installer doing the PATH registration, so no Windows code path is needed in `cli_install.rs` — guard that module to macOS (`#[cfg(target_os = "macos")]`).

Files to modify (when un-deferred):
- `apps/kanban-app/tauri.conf.json` — NSIS PATH configuration / installer hooks.
- Possibly a custom NSIS template under `apps/kanban-app/`.
- `.github/workflows/release-app.yml` — un-comment and complete the `build-windows` job; run `verify-app-bundle.sh` equivalent for the Windows artifact.

## Acceptance Criteria
- [ ] Installing the Windows package puts `kanban` on `PATH` for a new terminal session.
- [ ] Uninstalling removes the PATH entry.
- [ ] `cli_install.rs` is `#[cfg(target_os = "macos")]`-gated and the app still builds for Windows.

## Tests
- [ ] Windows CI smoke test (added when `build-windows` is enabled): after install, a fresh shell resolves `kanban --version` with exit 0. Until Windows runners are enabled this task stays in the backlog — it is intentionally not started, so the "automated tests required" rule is satisfied by the future CI smoke test rather than manual verification now.

## Workflow
- Deferred. Leave in the backlog. When picked up, use `/tdd` for any Rust `#[cfg]` gating changes; the NSIS/installer behavior is verified by the Windows CI smoke test.