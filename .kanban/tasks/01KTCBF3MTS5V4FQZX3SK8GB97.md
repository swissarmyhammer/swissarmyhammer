---
assignees:
- claude-code
depends_on:
- 01KTBNHSR4EVTVJ35MGGD510R2
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff480
project: local-review
title: Migrate mirdan validator deploy/scan paths from .avp/validators to ./.validators + $XDG_DATA_HOME/validators
---
## What
The validator **loader** now reads from `./.validators/` (project) and `$XDG_DATA_HOME/validators/` (user) via the new `ValidatorsConfig` (task 01KTBNHSR4EVTVJ35MGGD510R2). But mirdan's install/deploy/list/scan pipeline still targets the OLD `.avp/validators/` layout, so validators installed via `mirdan install` land where the loader no longer looks.

Migrate mirdan's validator paths to match the loader:
- `crates/mirdan/src/install.rs` — deploy target `.avp/validators/` → `./.validators/` (project) and `$XDG_DATA_HOME/avp/validators` → `$XDG_DATA_HOME/validators` (global). Update tests asserting `.avp/validators`.
- `crates/mirdan/src/list.rs` — scan `.avp/validators` and `~/.avp/validators` → new dirs.
- `crates/mirdan/src/info.rs` — `.avp/validators` lookups.
- `crates/mirdan/src/git_source.rs` — priority-directory entry `.avp/validators`.
- `crates/mirdan/src/new.rs` `run_new_validator` global path `avp/validators/<name>` → `$XDG_DATA_HOME/validators/<name>` (use `ValidatorsConfig`).
- `crates/mirdan/src/cli.rs` / `lib.rs` doc strings mentioning `.avp/validators/`.

Once mirdan no longer references `$XDG_DATA_HOME/avp/validators`, re-evaluate whether `AvpConfig` is still needed at all (mirdan `run_new_tool` still uses `avp/tools`).

## Why deferred
Out of scope for the format/loader task, which the implement skill scoped to "the loader's directory constants + docs". Migrating mirdan's full deploy pipeline is a separate, sizable change with its own test surface (~15 assertions across install.rs).

## Acceptance Criteria
- [x] mirdan deploys/scans validators at `./.validators/` and `$XDG_DATA_HOME/validators/` — consistent with `ValidatorLoader`.
- [x] No mirdan code references `.avp/validators` or `$XDG_DATA_HOME/avp/validators`.
- [x] `cargo test -p mirdan` green.

## Review Findings (2026-06-05 12:31)

### Blockers
- [x] `crates/mirdan/src/sync.rs` — The `mirdan sync` Validator verification branch hand-rolled the OLD layout (global `~/.avp/validators`, project `.avp/validators`), so `sync` reported every installed validator as missing because `deploy_validator` now writes to the new dirs. FIXED: the branch now routes through `crate::install::validators_dir(global)` — the same source of truth as `list.rs`/`info.rs`/`guess_installed_type`. The global path is now the XDG location (`validators_dir(true)`), not home-relative `~/.avp/validators`. Removed the now-unused `PathBuf` import. Added regression coverage: `test_sync_validator_present_in_project_dir` (TDD: failed before the fix, passes after) verifies a validator deployed to `validators_dir(false)` is counted as verified, not missing; `test_sync_validator_missing` was made `#[serial]` with CWD isolation so the project-relative `.validators/` lookup is deterministic. `cargo test -p mirdan` = 387 passed / 0 failed; `cargo clippy -p mirdan --all-targets` clean.

### Warnings
- [x] `crates/mirdan/src/install.rs` (`validators_dir`) — CWD-vs-git-root divergence. Reviewed and intentionally left as-is per the reviewer's own "acceptable-but-noted, not a blocker" judgment: `validators_dir(false)` stays CWD-relative (`.validators`), preserving the pre-existing convention and staying consistent with how every other mirdan package type resolves project paths (CWD-relative). The sync fix above routes through this same `validators_dir`, so deploy and sync are now consistent with each other regardless of this resolver choice. Full git-root unification of `validators_dir(false)` with the loader's `ManagedDirectory::from_git_root()` remains a follow-up, not addressed here (out of scope for this task per the implement instructions).