---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb180
project: local-review
title: 'fix(review): ensure_gitignore doesn''t add the .hashes/ line against a pre-existing swissarmyhammer-directory .validators/.gitignore'
---
## What

The incremental-tracking recorder now writes `.validators/.hashes/*.yaml` correctly in production (verified on a live calcutron qwen run — `subtracted=4` then `subtracted=7` observed, duplicate avoidance working). **But `ensure_gitignore` does NOT add the `.hashes/` ignore line in the live path**, so the ephemeral hash dir is not gitignored.

### Evidence (live calcutron run, 2026-06-15)
- `.validators/.hashes/` populated (13 entries, correct YAML).
- `git check-ignore .validators/.hashes/` → **fails (not ignored)**.
- `../calcutron/.validators/.gitignore` contains only the swissarmyhammer-directory default:
  `# Validators store … automatically created by swissarmyhammer-directory … # Keep validator definitions (they should be committed)` — **no `.hashes/` line**.
- (For calcutron specifically the whole `.validators/` shows as untracked `?? .validators/`, so it doesn't bite there — but the acceptance criterion "`.validators/.gitignore` contains the `.hashes/` line after a review" is not met in the live path.)

### Why the tests miss it
`ensure_gitignore`'s unit test (`ensure_gitignore_preserves_existing_committed_lines`) and the production-path test pass — but their pre-existing `.validators/.gitignore` content differs from the real one **swissarmyhammer-directory writes when it deploys the validators store**. Against that actual store-created gitignore, the append (or call) doesn't happen. Either:
1. `ensure_gitignore` is not actually invoked on the recorder path that writes `.hashes/` entries (the dir+entries get created by a path that skips it), or
2. `ensure_gitignore`'s logic treats the pre-existing store gitignore as "already set up" and returns without appending the `.hashes/` line.

Determine which (likely #1 or a content-detection edge case in #2) and fix it.

## Fix
- Make the recorder path that writes `.validators/.hashes/` entries reliably ensure the `.hashes/` ignore line is present in `.validators/.gitignore`, **appending** it while preserving the swissarmyhammer-directory-authored content (do not clobber the store gitignore).
- Coordinate with swissarmyhammer-directory's gitignore authorship: the `.hashes/` line should live alongside the store's existing lines (one source writes the file; the recorder appends its line idempotently).

Files: `crates/swissarmyhammer-validators/src/review/tracking.rs` (`ensure_gitignore`, `record_reviewed`/`upsert_entry` call site); check whether swissarmyhammer-directory owns `.validators/.gitignore` creation and whether that races/overwrites.

## Acceptance Criteria
- [ ] After a real production-path `review working` against a repo whose `.validators/.gitignore` was created by swissarmyhammer-directory (i.e. the line already pre-exists from the store deploy), `.validators/.gitignore` contains the `.hashes/` ignore line and the store's original lines are preserved.
- [ ] `git check-ignore .validators/.hashes/somefile.yaml` succeeds (the hash dir is ignored).
- [ ] `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools` and clippy `-D warnings` green.

## Tests
- [ ] A test that seeds `.validators/.gitignore` with the EXACT swissarmyhammer-directory default content (the "# Validators store … Keep validator definitions" text), then runs the recorder/`ensure_gitignore`, and asserts the `.hashes/` line is appended and the store lines remain — reproducing the live gap (must fail before the fix).
- [ ] Reuse the production-path harness so the assertion covers the real `review_op` → recorder call chain, not just `ensure_gitignore` in isolation.

## Workflow
- Use `/tdd` — reproduce the live gap with the real store-gitignore content first.

## Review Findings (2026-06-15 17:48)

### Warnings
- [x] `crates/swissarmyhammer-validators/src/review/tracking.rs` — the unit-test fixture `const STORE_GITIGNORE` (in `mod tests`) hand-copies the exact bytes of `swissarmyhammer_directory::ValidatorsConfig::GITIGNORE_CONTENT` instead of referencing it. This is the task's explicit blocker criterion: the test "must seed the EXACT swissarmyhammer-directory store gitignore content (the real `ValidatorsConfig::GITIGNORE_CONTENT` constant, not a hand-written approximation)." It is byte-identical today, but a frozen literal copy is decoupled from the source of truth — if the directory crate reorders or adds a line, the copy silently drifts and `ensure_gitignore_appends_to_the_store_authored_gitignore` stops reproducing the real on-disk store content it exists to guard, giving false-green confidence. Fix: `use swissarmyhammer_directory::{ValidatorsConfig, DirectoryConfig};` and write `<ValidatorsConfig as DirectoryConfig>::GITIGNORE_CONTENT`; delete the `STORE_GITIGNORE` literal. The validators crate already depends on swissarmyhammer-directory. — FIXED: the literal is gone; the test now binds `let store_gitignore = <ValidatorsConfig as DirectoryConfig>::GITIGNORE_CONTENT;` and seeds from it.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — the production-path test `run_review_request_appends_hashes_to_a_store_authored_gitignore` defines its own `const STORE_GITIGNORE`, a second hand-copied duplicate of `ValidatorsConfig::GITIGNORE_CONTENT`. The test comment claims it is "The EXACT content swissarmyhammer-directory writes on store deploy," but because it is a literal copy rather than a reference, the store constant can change and this copy silently drifts, so the test would no longer reproduce the real store-authored content it asserts against. Fix: seed the file from `<ValidatorsConfig as DirectoryConfig>::GITIGNORE_CONTENT` (the same source of truth) rather than a private copy. — FIXED: the literal is gone; the test now binds `let store_gitignore = <ValidatorsConfig as DirectoryConfig>::GITIGNORE_CONTENT;` (importing `swissarmyhammer_directory::{DirectoryConfig, ValidatorsConfig}`, already a dependency of the tools crate) and seeds from it.

### Notes (not blocking)
- Teeth verified: the production-path test drives the real `run_review_request` chain (`Scope::Working`, `force: false`, tracking ON), asserts `tasks_attempted > 0` (recorder reachable), and asserts the `.hashes/` line is appended AND the store's `# Keep validator definitions...` line preserved. If `ensure_gitignore` were skipped/short-circuited, no `.hashes/` line would be added to the seeded store content and the test would fail — so the test does have teeth and the recorder→`ensure_gitignore` call chain is genuinely exercised. The only defect is that the seed content is a copy, not the real constant.
- `review file` on `tests.rs` reported 2/15 review tasks failed (results flagged incomplete), but its returned finding corroborates the `tracking.rs` review's confirmed findings, so the picture is consistent.