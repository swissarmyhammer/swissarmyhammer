---
assignees:
- claude-code
depends_on:
- 01KRPD2X34N6AT0J4QRMTP9QSX
- 01KRPD4G1KTWM5CTRHAZM4JFCX
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffeb80
project: cli-in-app
title: 'CI: verify the shipped Kanban.app contains a working, signed kanban CLI'
---
## What
Add a release-pipeline gate that proves the *final, notarized* `Kanban.app` actually carries a usable `kanban` CLI — catching regressions where the sidecar silently drops out of the bundle or fails signing.

Files to create/modify:
- `build-support/verify-app-bundle.sh` (new) — takes a path to a `.app` bundle and a `--require-cli` flag. Asserts:
  - `Contents/MacOS/kanban` exists and has the executable bit.
  - `Contents/MacOS/kanban --version` exits 0 and prints a non-empty version.
  - `codesign --verify --deep --strict "<bundle>"` passes (skippable via `--skip-signing` for unsigned local/test bundles).
  - Exits non-zero with a clear message on any failure.
- `.github/workflows/release-app.yml` — in the `build-macos` job, after `cargo tauri build` and before the DMG upload, run `verify-app-bundle.sh` against the freshly built `.app` with `--require-cli` for the `kanban` matrix entry (omit `--require-cli` for `mirdan`). A failure must fail the release.

## Acceptance Criteria
- [x] `verify-app-bundle.sh --require-cli` against a bundle missing `Contents/MacOS/kanban` exits non-zero with a descriptive error.
- [x] `verify-app-bundle.sh --require-cli` against a bundle whose `kanban` is present, executable, and version-reporting exits 0.
- [x] `release-app.yml` runs the check for the `kanban` build and fails the job if the CLI is missing or unsigned.
- [x] The check does not run `--require-cli` for `mirdan` (no CLI expected).

## Tests
- [x] New `apps/kanban-app/tests/verify_bundle.rs`: build a mock `.app` directory tree in a tempdir and assert `verify-app-bundle.sh --skip-signing`:
  - exits 0 when `Contents/MacOS/kanban` is a present, executable script that prints a version on `--version`.
  - exits non-zero when `Contents/MacOS/kanban` is absent.
  - exits non-zero when `Contents/MacOS/kanban` exists but is not executable.
- [x] Test command: `cargo test -p kanban-app --test verify_bundle` — passes.

## Workflow
- Use `/tdd` — write `verify_bundle.rs` against mock bundles first, then implement `verify-app-bundle.sh`, then wire the CI step.