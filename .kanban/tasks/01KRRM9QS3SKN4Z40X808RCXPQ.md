---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffef80
project: cli-in-app
title: Fix already_installed to verify the symlink points at the running app's CLI
---
## What
`already_installed` in `apps/kanban-app/src/cli_install.rs` decides whether `run()` skips the launch-time CLI self-install. It takes a `bundled: &Path` parameter and **ignores it** (`let _ = bundled;`). It only checks whether the existing `kanban` entry is a symlink *shaped like* some `Kanban.app/Contents/MacOS/kanban` (via `is_inside_kanban_bundle`) — NOT whether it is the symlink we manage, pointing at the CLI of the *currently running* app.

The check must be **"is this our symlink"**, not "is there a `kanban`." Two concrete ways the current check is wrong:
- A symlink into a stale/moved/different `Kanban.app` → currently `true` (wrongly skips; the link is never repaired because `run()` short-circuits before `install_cli_symlink`'s `Repaired` branch or the escalation re-link).
- A *real file* named `kanban` — e.g. a `cargo install`-ed `kanban` binary that happens to sit in a target dir — must never be mistaken for our install. (`already_installed` already returns `false` for a non-symlink via `read_link` erroring, but this must be explicit and test-covered, since "check for the symlink, not just for `kanban`" is the whole point.)

Fix: `already_installed` returns `true` **only** when `target_dir/kanban` is a symlink whose target equals exactly `bundled` (the running app's CLI — "what we intend"). Every other case — missing, a real non-symlink file, a symlink to an unrelated target, or a symlink into a *different* Kanban bundle — returns `false`, so `run()` proceeds to install/repair.

Changes to `apps/kanban-app/src/cli_install.rs`:
- Rewrite `already_installed(target_dir: &Path, bundled: &Path) -> bool`: `read_link(target_dir.join(CLI_NAME))`, return `true` iff the resolved link target equals `bundled`. Use the `bundled` parameter — delete the `let _ = bundled;` discard.
- Update the docstring: it currently says `bundled` is "Unused for the bundle-shape check" — now false. Document the exact-match semantics ("our symlink, pointing at the running app's bundle").
- Re-state the Homebrew-cask note accurately: a cask links `<brew bin>/kanban -> /Applications/Kanban.app/Contents/MacOS/kanban`; when the app runs from `/Applications`, `bundled` equals that path, so exact-match still returns `true` and the app still does not fight the cask.
- `is_inside_kanban_bundle` stays in use by `install_cli_symlink`'s `Repaired` branch — do NOT delete it; `already_installed` simply stops calling it.

## Acceptance Criteria
- [x] `already_installed` returns `true` only when `target_dir/kanban` is a symlink pointing exactly at `bundled`.
- [x] A symlink into a *different* `Kanban.app` bundle (≠ `bundled`) now returns `false` — previously `true`.
- [x] A real (non-symlink) `kanban` file in the target dir — e.g. a `cargo install`-ed binary — returns `false`.
- [x] A missing entry, and a symlink to an unrelated target, return `false`.
- [x] The `bundled` parameter is used; `let _ = bundled;` is gone; the docstring is corrected.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` is clean.

## Tests
- [x] In `apps/kanban-app/tests/cli_install.rs`, update the existing `already_installed` test(s) to exact-match semantics and add coverage:
  - symlink whose target == `bundled` → `true`.
  - symlink into a *different* `…/Kanban.app/Contents/MacOS/kanban` path (≠ `bundled`) → `false` (regression: returned `true` before the fix).
  - a real, non-symlink file named `kanban` in the target dir → `false` (the cargo-installed-binary case).
  - no entry → `false`; a symlink to an unrelated non-Kanban target → `false`.
- [x] Test command: `cargo test -p kanban-app --test cli_install` — passes.
- [x] `cargo test -p kanban-app` — full crate suite stays green.

## Workflow
- Use `/tdd` — write the new/updated `already_installed` tests first (the different-bundle → false and real-file → false cases especially), watch them fail against the current `let _ = bundled;` implementation, then rewrite `already_installed` to make them pass.