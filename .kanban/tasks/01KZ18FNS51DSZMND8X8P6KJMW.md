---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz5nbcswyjgf9hq5tw9sbqrq
  text: |-
    ### implement — changed

    Investigated before choosing: wrote a real repro test for the LITERAL acceptance wording ("two `review working` runs across an intervening commit of UNRELATED files render byte-identical prompts for a file neither run changed"), using `commit_only` (a new `TestRepo` helper that stages only named paths) to commit a truly unrelated file while leaving the target file's own dirty diff untouched. That test PASSED on the unfixed code — git2 blame's history walk already attributes an untouched file's lines correctly regardless of unrelated HEAD movement, so that narrow literal scenario was never actually broken.

    The REAL, demonstrated drift is the file's OWN uncommitted→committed transition getting swept along by an intervening commit that resolves a DIFFERENT finding in the SAME file — exactly what `/finish`'s commit step does (`git add -A && commit` stages everything dirty, not just the fixed lines). Wrote `working_scope_sha_is_stable_across_a_sweep_commit_of_an_unresolved_line`: a still-open, byte-identical dirty line (`BETA-DIRTY`) flips from `worktree` (run 1) to a real commit sha (run 2, `94585678`) purely because an unrelated line in the same file got fixed-and-committed in between. Confirmed RED on unfixed code with exactly that failure.

    **Decision: Option A** — pinned `blame_at` for `Scope::Working`/`File`/`Glob` to `working_tree_blame_anchor`: the merge-base between HEAD and the detected `main`/`master` branch (reusing `GitOperations::main_branch` + `git2::Repository::merge_base`), falling back to `None` (old HEAD-bound behavior) when no such branch/anchor exists (e.g. reviewing directly on main). This anchor stays fixed for the life of a feature/task branch — every `/finish` commit lands strictly after it, so blame bounded there never sees them: a still-open line keeps reading `worktree` across iterations regardless of how many intervening commits land, instead of drifting to a real sha the moment ANYTHING in the file gets committed.

    Rejected Option B (drop the sha column): ^k12rn64 added it specifically for blame-checking findings against the reviewed commit, used successfully throughout today's `/finish` batch. `Scope::Sha` review's blame anchor (the range's "to" commit) is already fully stable for a literal sha range — verified by reading `resolve_sha`, no change needed there. Since Option A fully closes the demonstrated drift without losing that capability, dropping the column was not justified.

    Changed files:
    - `crates/swissarmyhammer-validators/src/review/scope.rs` — added `working_tree_blame_anchor`; wired it into `resolve_working`, `resolve_file`, `resolve_glob` (replacing the `None` blame anchor each passed); updated `ResolvedScope::blame_at` doc; added the RED→GREEN regression test.
    - `crates/swissarmyhammer-validators/src/review/test_support.rs` — added `TestRepo::commit_only`, `TestRepo::rename_current_branch_to`, `TestRepo::checkout_new_branch` (shared git-fixture helpers needed for the repro).

    Evidence:
    - `cargo nextest run -p swissarmyhammer-validators`: 364 tests run, 364 passed, 0 skipped (includes the new regression test and every pre-existing scope/blame test, unaffected).
    - `cargo fmt --all`: clean, no diff beyond the intended edits.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 3071 tests run, 3071 passed, 0 skipped, 0 failed (run in full, not narrowed).

    next: /review
  timestamp: 2026-08-04T05:53:39.132889+00:00
- actor: claude-code
  id: 01kz5ng7xwpw89femq70nmwrdc
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — investigated before choosing (per task instruction). First hypothesis (an unrelated file's commit shifts blame) was tested and REJECTED — git2's blame walk already handles that correctly. Real bug found: /finish's per-iteration `git add -A && commit` sweeps a still-open finding's own uncommitted line into a commit made to resolve a DIFFERENT finding in the SAME file, flipping that line's blame from worktree to a real sha even though its bytes never changed. Chose Option A (pin blame_at to merge-base with main/master via working_tree_blame_anchor, falling back to old HEAD-bound behavior when no anchor exists) over Option B (drop the sha column) — rejected B because the column has demonstrated value used throughout today's entire /finish batch for blame-checking findings (^k12rn64).
    - test: green — swissarmyhammer-validators 364 passed; fmt clean; clippy clean; full rdeps(swissarmyhammer-validators) run for real: 3071 passed, 0 failed
    - commit: be66a8826
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-04T05:56:17.980663+00:00
- actor: claude-code
  id: 01kz5nzc10a2689c72q8nqb917
  text: |-
    ### review — clean
    - evidence: review sha be66a8826~1..be66a8826 — engine returned 2 confirmed findings, both at crates/swissarmyhammer-validators/src/review/scope.rs:2015-2016 (hardcoded padding-loop count `30`); blame-checked against be66a8826 — both lines predate this commit (June 2026, unrelated pre-existing test `working_scope_groups_duplicate_under_validator_with_full_source`), not touched by this diff. Dropped per pre-existing-line check and the existing-test-refactor exception. Zero new findings.
    - next: none — task moved to done
  timestamp: 2026-08-04T06:04:33.696800+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9c80
title: blame shas in the review prime drift the prompt between finish-loop iterations
---
Found while investigating ^k5wsxh0.

`render_numbered_source` in `crates/swissarmyhammer-validators/src/review/fleet.rs` prints every source line as `{line} | {sha} {mark} | {text}`. The sha comes from libgit2 blame, computed in the scope stage.

For `Working`, `File`, and `Glob` scopes, `blame_at` is `None`, so blame binds to HEAD:

- Any commit, amend, or rebase between two runs shifts every sha.
- An uncommitted line renders `worktree`. Commit it and the same review renders a real sha, with no source byte changed.
- `git add` alone flips `untrackd` to `worktree`, because `path_is_tracked` consults the index when `at` is `None`.
- A failed blame degrades to `????????` for every line of that file, changing the prompt silently rather than erroring.

Only `Scope::Sha` pins blame.

## Why it matters

Two back-to-back runs on an unchanged worktree are unaffected. But the finish loop commits between iterations, so **every iteration sends a different prompt for unchanged code**. That breaks prefix-cache reuse, and it feeds the stuck-detection guardrail a moving target — the guardrail declares a task stuck when the SAME finding survives 3 iterations.

## Work

Decide what the blame column is for and make it stable against it. Options to weigh:

- Pin `blame_at` to the merge-base or to the review's base revision for working scopes, so the column answers "who last touched this before my change" rather than "what does HEAD say right now".
- Or drop the sha column and keep only the touched-mark, if the sha is not carrying its weight in prompt bytes.

Do not simply widen a tolerance. The column must mean one thing and mean it every run.

## Acceptance

- Two `review working` runs across an intervening commit of unrelated files render byte-identical prompts for a file neither run changed.
- A test pins it.

#bug #review