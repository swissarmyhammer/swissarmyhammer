---
assignees:
- claude-code
depends_on:
- 01KRPD2X34N6AT0J4QRMTP9QSX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffea80
project: cli-in-app
title: Auto-install the bundled kanban CLI onto PATH at app launch
---
## What
When `Kanban.app` launches, silently and idempotently ensure the bundled `kanban` CLI (shipped by the sidecar task at `Contents/MacOS/kanban`) is reachable on the user's `PATH`. No menu item, no click — it just happens. Runs every launch and self-heals stale/missing links.

Files to create/modify:
- `apps/kanban-app/src/cli_install.rs` (new module) with these pure, testable functions:
  - `fn resolve_bundled_cli(current_exe: &Path) -> Option<PathBuf>` — given the running app executable path, return the sibling `kanban` (`current_exe.parent()/"kanban"`), or `None` if absent.
  - `fn already_installed(target_dir: &Path, bundled: &Path) -> bool` — true if `kanban` already resolves on PATH to a symlink pointing into ANY `Kanban.app/Contents/MacOS/kanban` (covers the Homebrew-cask case where `brew` already made the link — do nothing, never prompt).
  - `fn install_cli_symlink(bundled: &Path, target_dir: &Path) -> io::Result<InstallOutcome>` — create/repair a `kanban` symlink in `target_dir` pointing at `bundled`. Idempotent: no-op if the link already points at `bundled`; repair if it points elsewhere into a Kanban bundle; leave a non-Kanban real file untouched and return `InstallOutcome::Skipped`.
  - `fn pick_target_dir() -> TargetDir` — choose a directory that is both user-writable AND on the default PATH. Prefer the Homebrew bin dir (`brew --prefix`/bin via `which brew`, e.g. `/opt/homebrew/bin` or `/usr/local/bin`) when writable; else `/usr/local/bin`. Return whether the chosen dir needs privilege escalation.
- `apps/kanban-app/src/main.rs` — add `mod cli_install;` and call a `cli_install::run()` entry point from `setup_app` (or just after, before GUI is interactive). Run it on a background thread (`std::thread::spawn`) so it never blocks startup. Log outcome via `tracing` (os_log) — never `eprintln!`.

Privilege handling (macOS):
- If the chosen dir is user-writable: symlink silently. This is the common Homebrew case — fully silent.
- If the only viable dir is root-owned `/usr/local/bin`: perform the symlink via a single `osascript -e 'do shell script "ln -sf ... " with administrator privileges'`, but ONLY once — guard with a marker file (e.g. `~/Library/Application Support/com.swissarmyhammer.kanban/.cli-install-attempted`) so a user who declines the prompt is not nagged on every launch. A later launch still self-heals silently if a writable dir becomes available.
- This is std/`osascript` only — no Tauri JS command, no new capability/permission entry in `capabilities/default.json`.

## Acceptance Criteria
- [x] On app launch with the app installed in `/Applications`, `kanban` becomes resolvable on PATH (via symlink into a PATH dir) pointing at the bundle's `Contents/MacOS/kanban`.
- [x] Idempotent: a second launch makes no change and issues no prompt.
- [x] When the Homebrew cask already created the `kanban` link, launch is a silent no-op (`already_installed` short-circuits).
- [x] A pre-existing non-Kanban `kanban` on PATH is never overwritten.
- [x] Startup is not blocked — the work runs off the main thread.

## Tests
- [x] New `apps/kanban-app/tests/cli_install.rs` unit/integration tests using `tempfile`:
  - `install_cli_symlink` into an empty temp dir creates a valid symlink to the bundled path.
  - Re-running `install_cli_symlink` is a no-op (outcome `AlreadyCurrent`), link unchanged.
  - A stale symlink pointing at a different `…/Kanban.app/Contents/MacOS/kanban` is repaired to the current bundle.
  - A pre-existing real (non-symlink) `kanban` file is left intact, outcome `Skipped`.
  - `resolve_bundled_cli` returns the sibling path when present and `None` when absent.
  - `already_installed` returns true for a temp PATH dir whose `kanban` link targets a `Kanban.app/Contents/MacOS/kanban`-shaped path, false otherwise.
- [x] Test command: `cargo test -p kanban-app --test cli_install` — passes.
- [x] The privilege-escalation (`osascript`) branch is isolated behind `pick_target_dir`'s writability result and is not unit-tested; document this in a code comment.

## Workflow
- Use `/tdd` — write the `cli_install.rs` tests first against the function signatures above, then implement the module and wire it into `main.rs`.

## Implementation Notes
- `cli_install` is a module of the binary crate (no lib target). The integration test compiles it standalone via `#[path = "../src/cli_install.rs"]` — the same independent-compilation pattern `build.rs` files use across this workspace. `#[allow(dead_code)]` is scoped to that test's `mod` import only (the launch-time `run`/`spawn`/escalation helpers are exercised by the binary, not the pure-function tests); the binary build itself stays strict.
- The entry point wired into `setup_app` is `cli_install::spawn()`, which detaches `run()` onto a background `std::thread`.
- `is_writable` probes empirically by creating/removing a temp file rather than doing ownership/mode arithmetic, which is unreliable across ACLs.
- One added test beyond the listed set: `already_installed` returns false for a `kanban` symlink that points outside any Kanban bundle.