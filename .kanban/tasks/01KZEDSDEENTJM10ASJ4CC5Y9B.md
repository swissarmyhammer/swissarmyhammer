---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9880
title: 'review scope: exclude validator-set fixture files from review pairs and tool runs'
---
The review engine reviews validator fixture files as ordinary changed source, so every missing-docs tool rule fires on the fail fixture built to make it fire. This blocked ^f0wna3d: six eslint findings asked to document `missing-docs-typescript.fail.ts`, and documenting it breaks the fixture contract in `builtin/validators/README.md` (the fail fixture must hold undocumented items). Every future fixture edit re-raises the same findings.

Fix — derive the exclusion from the validator store, not from a user glob:
- The loader knows every validator set root across all three layers (builtin, user `~/.validators/`, project `./.validators/`). A changed file under any set's `fixtures/` directory leaves the review work-list: no LLM (validator, file) pair, and never an argument to a tool rule's `run` script.
- Report each excluded file in `skipped_files` with the reason "validator fixture", and log it. No silent truncation.
- This does not conflict with the no-path-based-test-exclusion rule. That rule forbids user path globs for TEST code, because tests live inline in source. A fixture directory is not test code: the README contract defines its files as intentionally failing data, and doctor is their gate — doctor runs every tool rule against them on each health check. The exclusion comes from the store structure, a single source of truth.

Acceptance:
- A `review sha` over a commit that touches `builtin/validators/code-hygiene/fixtures/*.fail.*` reports zero findings about fixture files and lists them in `skipped_files`.
- Doctor still runs the fixtures and still fails a broken tool rule — the gate moves, it does not vanish.
- A production-path test covers the scenario: a changed fail fixture plus a changed source file; the source file is reviewed, the fixture is skipped with the reason.

#tool-validators