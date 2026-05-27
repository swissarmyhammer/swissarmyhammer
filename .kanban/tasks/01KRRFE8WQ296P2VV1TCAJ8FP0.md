---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffee80
project: cli-in-app
title: Remove the CLI-install marker file — gate self-install on the symlink alone
---
## What
The launch-time CLI self-install (`apps/kanban-app/src/cli_install.rs`) currently gates (re)install on two pieces of state: the `kanban` symlink AND a `.cli-install-attempted` marker file. The marker is written by `install_with_escalation` *regardless of outcome* — including on a successful install — so once any privileged install succeeds, the marker is permanent and the symlink can never be re-created if it is later deleted. That breaks the module's documented self-healing promise and is confusing (two gates).

Decision (confirmed with the user): gate self-install on the **symlink alone**. Remove the marker concept entirely. If the `kanban` symlink is missing, the app attempts to install it on launch — every launch until it succeeds, including the escalation/password path. There is no "remember the attempt" state.

Changes to `apps/kanban-app/src/cli_install.rs`:
- Delete the `ESCALATION_MARKER` constant and the `escalation_marker_path()` function (both currently `#[cfg(target_os = "macos")]`).
- In `install_with_escalation` (`#[cfg(target_os = "macos")]`): remove the marker-exists early-return guard at the top and the marker-write block at the end. The function now simply builds the AppleScript via `build_install_applescript` and runs it in-process via `NSAppleScript` every time it is called.
- `run()` already calls `install_with_escalation` only when `already_installed()` is false (symlink absent, or not pointing into a Kanban bundle) — so after the marker removal the symlink is the sole gate. Confirm no other marker references remain.
- Remove any imports left unused by the removal (e.g. a `dirs` import, if `escalation_marker_path` was its only use in this file) so `cargo clippy` stays clean.
- Update the module-level doc comment and the `install_with_escalation` doc comment: they currently describe the one-shot marker / "not nagged on every launch" behavior. Replace with an accurate description — self-install is gated solely on the symlink; a missing symlink is (re)installed on the next launch.

Docs to correct (they now describe behavior that no longer exists):
- `apps/kanban-app/README.md` — the self-install section describing a "single one-time admin prompt".
- `apps/kanban-cli/README.md` — the line about the "single one-time admin prompt that may appear".
Both should instead say: on a drag-installed app, if the `kanban` CLI is not on PATH, the app offers to install it on launch (an explanatory dialog, then the admin prompt when a root-owned directory is the only target); it will offer again on a later launch if the CLI is still not linked.

## Acceptance Criteria
- [x] `ESCALATION_MARKER` and `escalation_marker_path()` are gone; no `.cli-install-attempted` reference remains anywhere in `cli_install.rs`.
- [x] `install_with_escalation` no longer reads or writes any marker; it builds and runs the AppleScript unconditionally when called.
- [x] Whether self-install runs is determined solely by `already_installed()` (the symlink): with the symlink absent, relaunching the app triggers a fresh install attempt; with a valid symlink present, it is still a silent no-op.
- [x] `apps/kanban-app/README.md` and `apps/kanban-cli/README.md` no longer claim a "one-time" prompt; they describe the symlink-gated retry behavior accurately.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` is clean (no unused imports left by the removal).

## Tests
- [x] `cargo test -p kanban-app` — the existing suite (including the 13 `cli_install` tests) still passes; the marker removal must not break `install_cli_symlink` / `already_installed` / `build_install_applescript` coverage.
- [x] Test command: `cargo test -p kanban-app --test cli_install` — passes.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` — clean.
- [x] The escalation path (`install_with_escalation`, `NSAppleScript`, system dialogs) remains a GUI/privilege side effect that is not unit-tested — this task only removes code, and the removal is verified by the existing `cli_install` tests staying green plus clippy. The `build_install_applescript` unit tests already cover the pure, testable part and are unaffected.

## Workflow
- This is primarily a deletion + doc correction; `/tdd` does not strictly apply since no new behavior is added. Make the removal, confirm the existing `cli_install` tests still pass, fix any unused-import clippy warnings, then correct the two READMEs.