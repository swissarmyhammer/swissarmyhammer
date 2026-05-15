---
assignees:
- claude-code
depends_on:
- 01KRPD2X34N6AT0J4QRMTP9QSX
- 01KRPD3VEZ90BFMFTX6S0S1FWH
- 01KRPD4G1KTWM5CTRHAZM4JFCX
- 01KRPD501K5D09EC188KJKWT3Z
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffec80
project: cli-in-app
title: Document that installing the Kanban app provides the kanban CLI
---
## What
Update user-facing docs so the "install one thing, get both" story is explicit, and document the honest caveat for DMG-drag installs.

Files to modify:
- `README.md` — in the install/getting-started section, state that installing `Kanban.app` also provides the `kanban` CLI. Cover the three paths:
  - `brew install --cask kanban` — CLI lands on PATH automatically (cask `binary` stanza), no further action.
  - DMG drag to `/Applications` — the app self-installs `kanban` onto PATH on first launch; note the single one-time admin prompt that may appear when no user-writable PATH directory exists.
  - `cargo install --path apps/kanban-cli` / standalone `kanban` Homebrew formula — still available for headless/Linux/CI use; `conflicts_with` keeps it from colliding with the cask-provided CLI.
- `apps/kanban-app/` — add or update a short `README.md` describing the bundled-sidecar layout (`Contents/MacOS/kanban`) and the launch-time self-install behavior, so future contributors understand why `scripts/stage-cli-sidecar.sh` and `src/cli_install.rs` exist.
- If `ARCHITECTURE.md` documents the app/CLI split, add a sentence noting the CLI is co-packaged in the app bundle.

## Implementation notes
- The user-facing install/getting-started doc for the kanban product is `apps/kanban-cli/README.md` (it owns the `Install` + `Desktop app` sections with the cask/DMG paths). The repo-root `README.md` is the SwissArmyHammer suite README and has no kanban app section, so the install-story edits landed in `apps/kanban-cli/README.md`.
- Added a contributor-facing `apps/kanban-app/README.md` describing the sidecar staging script and the `cli_install` launch-time self-install.
- Added a `Bundled CLI` bullet to the `kanban-app` Rust Backend section of `ARCHITECTURE.md`.

## Acceptance Criteria
- [x] `README.md` clearly tells a user that installing the app gives them the `kanban` CLI, and how it reaches PATH for each install method.
- [x] The one-time admin-prompt caveat for DMG installs without a writable PATH dir is documented, not hidden.
- [x] `apps/kanban-app/README.md` explains the sidecar + self-install design for contributors.
- [x] No stale instructions remain telling users to install the CLI separately on macOS as a required step.

## Tests
- [x] This is a documentation task — no automated code tests. Verification is a Markdown link/format check: run the repo's existing docs lint if one exists (`.github/workflows/docs.yml`), otherwise confirm `README.md` and `apps/kanban-app/README.md` render without broken relative links via `markdown-link-check` or equivalent already used in the repo. `docs.yml` only builds the `doc/` mdBook and does not touch these READMEs; relative links and anchors were verified manually (`../kanban-cli/README.md#desktop-app` and `#install` both resolve).
- [x] Acceptance is reviewed against the criteria above during code review.

## Workflow
- Documentation only — no `/tdd`. Make the edits, then re-read the changed sections end-to-end for accuracy against the behavior shipped by the other tasks in this project.