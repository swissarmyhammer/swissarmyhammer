---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m07t3h1vjz7tjsp1smbk2c33
  text: |-
    Research done. What the code says:

    - `deinit_profile_validators` (crates/mirdan/src/install/profile.rs) reads `builtin_validators_by_set()` — the CURRENT embed — and removes each embedded file path one by one through `remove_builtin_file_and_cleanup`. A set no current roster names has no embedded path, so no file of it is ever named, and it stays. This is the defect.
    - The store is `.validators/` (`store::validators_store_dir`): `~/.validators` for user scope, `./.validators` for project scope.
    - The loader (`crates/swissarmyhammer-validators/src/validators/loader.rs::load_rulesets_directory`) says what a validator set IS on disk: a top-level subdirectory of the store that holds a `VALIDATOR.md`, and whose name does not open with `_`. That is the unit the store serves.
    - The same store is also the USER precedence layer, and the store README invites a user to add a set of their own there.

    Two tests stand on the present behaviour:
    - `init_profile_validators_idempotent_refreshes_builtin_preserves_user` — INIT keeps user files. Unchanged by this card.
    - `init_profile_writes_store_readme_and_deinit_removes_it` — asserts "deinit must leave user-authored sets untouched". This card changes that statement.

    No marker on disk tells a set an older binary wrote from a set a user wrote. ^35tgz1c removed the retirement mechanism and that decision stands, so no roster of retired names and no per-install record are available. So the only measurement deinit can make is "is this directory a validator set".
  timestamp: 2026-08-17T12:10:54.907569+00:00
- actor: claude-code
  id: 01m07tvgbrszn822my4jxebc9m
  text: |-
    ## The shape chosen, and the reason

    **Remove every set directory in the store, whichever binary wrote it.** Not the whole `.validators` directory.

    The reason is that the store has ONE unit and it is the SET. `load_rulesets_directory` in the validator loader reads a set as a top-level subdirectory of the store that carries a `VALIDATOR.md` and whose name does not open with `_`. `deinit_profile_validators` now makes that same measurement over the store, so deinit and the loader agree on what a set is. Reading the store rather than the embed is what clears a set an older binary wrote.

    Removing the whole directory was refused because it takes content that is not a validator at all. `.validators/` is also the USER precedence layer, so a person can put anything beside the set directories. A directory-wide delete has no measurement behind it; a set-by-set delete has one.

    ## What happens to a file a user edited or added by hand

    - A file INSIDE a set goes with the set: an edited builtin rule, and a rule a user added under `code-hygiene/rules/`. A set is removed whole.
    - A set a user WROTE goes too. No marker on disk tells such a set from a set an older binary wrote, and the store serves the review engine alone, so the validators go when the engine goes.
    - A file that is NOT part of a set stays: a loose file at the store root, and a directory with no `VALIDATOR.md`. Each survivor keeps the store directory itself in place, because `prune_store_readme` removes that directory only when nothing is left in it.

    `builtin/validators/README.md` now carries a `## What `sah deinit` removes` section that states all of this to the user, and it says to copy a set out of the directory first.

    ## Measured with the real binary, on a throwaway HOME

    Run 1 — a store that also holds content that is not a set:
    1. `sah init user` — 13 sets, 156 files, plus the store README.
    2. Staged beside them: `.validators/no-secrets/` (`VALIDATOR.md` + `rules/no-secrets.md`, the pre-merge set), `.validators/my-notes.md`, `.validators/drafts/draft.md`.
    3. `sah deinit user` — every set gone, `no-secrets` included, and the README gone. `my-notes.md` and `drafts/draft.md` stand, and they keep `.validators/` in place.

    Run 2 — a store that holds sets alone:
    1. `sah init user` — 156 files.
    2. Staged `.validators/no-secrets/`.
    3. `sah deinit user` — `.validators` is gone.

    ## Test

    `deinit_profile_clears_every_validator_set_and_keeps_what_is_not_a_set` in `crates/mirdan/src/install/tests.rs` drives `init_profile` then `deinit_profile` over an isolated HOME.
    - RED before the change: `deinit must clear a set no current roster names: ".../.validators/no-secrets"`.
    - The keep half was proved for its own cause too. With the manifest test in `validator_set_names_in` replaced by a bare `is_dir()`, the test fails on `the draft file stands: NotFound`. The real rule was restored after the measurement.

    `init_profile_writes_store_readme_and_deinit_removes_it` now states that a store this install wrote alone leaves no store directory behind. Its earlier user-set assertion moved to the new test, so the two do not overlap.

    `remove_builtin_file_and_cleanup` is deleted. It had one caller and the caller is gone.
  timestamp: 2026-08-17T12:24:00.632442+00:00
- actor: claude-code
  id: 01m07tvp17j3f0jbd0f2rvhfgv
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/tests.rs, builtin/validators/README.md. `cargo nextest run --workspace`: 14164 passed, 0 failed, 0 skipped. `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo fmt --all --check`: clean. Real-binary measurement on a throwaway HOME: `sah deinit user` clears `no-secrets`, a set no current roster names, and keeps a loose file and a manifest-less directory.
    - next: /review
  timestamp: 2026-08-17T12:24:06.439197+00:00
- actor: claude-code
  id: 01m07vd43c2q43xgq22mkc4y60
  text: |-
    ### test — green

    Verified each claim by measurement on a throwaway HOME (`$SCRATCH/fake_home`, `HOME=$FAKE_HOME target/debug/sah init user` / `deinit user`), then swept it away with no repo files touched:

    1. **No install marker exists — confirmed.** Grepped for manifest/receipt/lock-file fields across `crates/mirdan/src`. The only candidate is `crates/mirdan/src/lockfile.rs` (`Lockfile`/`LockedPackage`), but `init_profile`'s own pre-existing doc comment (profile.rs, "# Lockfile" section) states builtins are deliberately never recorded there — only registry-installed (git-source) packages get an entry, since builtins ship in the binary and carry no download/integrity identity. So the "no marker" premise is genuinely true and is a pre-existing, documented design choice, not something this diff introduced. The safe-rule alternative (remove only what an install wrote) has no data to work from.
       README warning — confirmed real and reachable. `builtin/validators/README.md` is written to the store root itself (`.validators/README.md`) at `sah init user` time, so it sits beside the data it warns about from the moment of install, readable any time before a later `sah deinit`. Measured: after `sah init user` in the fake HOME, `.validators/README.md` exists and its "What `sah deinit` removes" section states plainly that built-in sets, an older version's sets, and user-written sets are all removed, and tells the user to copy a set elsewhere first. There is no interactive CLI confirmation prompt at `deinit` time — the README is the only warning mechanism, and it is reachable but not enforced.

    2. **Survivor rule — confirmed by measurement.** In the fake HOME after `sah init user` (13 builtin sets), staged: a loose file (`.validators/my-notes.md`), a manifest-less directory (`.validators/drafts/draft.md`), and a `_`-prefixed directory (`.validators/_shared/partial.md`). Ran `sah deinit user`. Result: all 13 builtin sets removed; `my-notes.md`, `drafts/draft.md`, and `_shared/partial.md` all survived; `.validators/` itself remained (non-empty). Matches the codebase's own test `deinit_profile_clears_every_validator_set_and_keeps_what_is_not_a_set` and the sibling test asserting the store directory disappears when nothing survives.

    3. **`remove_builtin_file_and_cleanup` — confirmed dead, no other caller.** `grep -rn "remove_builtin_file_and_cleanup" --include=*.rs .` across the whole workspace: zero hits. Fully removed with its last caller.

    4. **New test fails for its own cause — confirmed.** Broke `validator_set_names_in` in `crates/mirdan/src/install/profile.rs` by dropping the `VALIDATOR.md` manifest check (every subdirectory becomes a "set"). Ran the new test: FAILED at `tests.rs:320` — `"the draft file stands: Os { code: 2, kind: NotFound, ... }"` — i.e. the manifest-less `drafts/draft.md` survivor got swept because the set rule broke, exactly the cause under test. Reverted the file; `git diff` line count back to 175, matching the original diff exactly.

    5. **No new `.unwrap()`/`.expect(`/`panic!(`/unnamed numeric literal in `profile.rs`.** Confirmed by diff grep — zero hits in the production file, and no numeric literals added in either changed file. `tests.rs` does add several `.expect(...)` calls in the new test, but this matches the file's own prevailing convention (330 pre-existing `.unwrap()`/`.expect()` calls in the same file) and is not gated by any clippy lint (no `unwrap_used`/`expect_used` config found) — consistent with how the most recent sibling commit on this branch (`17f72ce80`) also added a new `panic!(...)` inside test code. Flagging this measurement explicitly rather than silently dropping it, per the no-nit-dropping rule, but treating it as satisfied given the established, repo-wide test convention.

    6. **`git status` clean after all probes.** The throwaway HOME lived entirely under the scratchpad outside the repo; `git status --short` before and after shows the same 7 modified files (4 kanban board files + the 3 task-relevant files), zero untracked/stray entries.

    Full verification:
    - `cargo fmt --check` — clean, no output.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run --workspace` — 14164 tests run: 14164 passed (41 slow, 2 leaky), 0 failed, 0 skipped, 0 build warnings.

    No failures found. Nothing to fix.
  timestamp: 2026-08-17T12:33:37.900214+00:00
- actor: claude-code
  id: 01m07vm6fjef7dtw6nqndq40cc
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (2325e73ae); 9 validators ran, 0 findings, 0 confirmed, 0 refuted, 0 failed; 2 files reviewed, 4 `.kanban/` files excluded by `.reviewignore`
    - next: card moved to done
  timestamp: 2026-08-17T12:37:29.714621+00:00
- actor: claude-code
  id: 01m07vmqj3jc2qyy1jh0hxwbc5
  text: |
    ### finish iteration 1 — clean
    - implement: changed — 3 files. `deinit_profile_validators` reads the STORE, not the roster, and removes each validator SET whole. Content that is not a set stays. Measured with the real binary on a throwaway HOME.
    - test: green — cargo nextest run --workspace, 14164 passed, 0 failed. fmt and clippy clean. The test step confirmed there is truly NO on-disk marker that tells a set the installer wrote from a set the user wrote, so the code cannot keep one and remove the other.
    - commit: 2325e73ae
    - review: clean — 9 validators, 0 findings.
    - OPEN RISK for a person to decide: `sah deinit` removes a validator set the USER wrote, with no prompt. The warning stands in the store README, which the install writes, but nothing asks the user at the moment of the delete. A confirmation prompt is a separate card if you want one.
  timestamp: 2026-08-17T12:37:47.203743+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffa880
title: deinit leaves validator content that an older binary wrote
---
`deinit_profile_validators` in `crates/mirdan/src/install/profile.rs` walks the CURRENT embedded roster to decide what to remove. So it clears every set the current binary installs, and it leaves behind any content that only an OLDER binary wrote.

## Measured

On a throwaway HOME:

1. `sah init user` deployed 13 validator sets, 156 files.
2. Two files of the pre-merge `no-secrets` set were put beside them, which is what a store written before the nine-set merge holds.
3. `sah deinit user` removed the 156 files and left `.validators/no-secrets/VALIDATOR.md` and `.validators/no-secrets/rules/no-secrets.md`.

For a store that holds only what the current binary wrote, deinit removes the `.validators` directory itself.

## Why this matters now

^35tgz1c deleted the retired-validator mechanism, on the decision that "we SHOULD NOT have 'retirement' code at all, we have deinit." That decision stands. This card does not ask for the snapshot mechanism back — a byte-frozen copy of every file ever deleted is the cost the decision refused.

But the premise "deinit clears a store" is true only for a store the current binary wrote. A user who installed an older version and then upgraded keeps running any rule that version deployed, and deinit does not remove it.

## What to do

Make deinit clear the validator content it did not write, not only the sets the current roster names. Read the store rather than the embed. Decide what "the store" means from the code: the `.validators` directory that mirdan itself deploys.

Weigh which shape is right, and state the reason:
- Remove the whole `.validators` directory, since mirdan owns it.
- Remove every set directory in the store, whichever binary wrote it.

Consider what happens to a file a user edited or added by hand. State the answer rather than leaving it to chance.

## Done when

- `sah deinit` clears a store that holds a set no current roster names, measured against a throwaway HOME.
- A test holds the behaviour, so it cannot regress.
- `cargo nextest run --workspace` green; fmt and clippy clean.

## Found by

The reviewer of `6b6fe8cf1` (^35tgz1c). Correctly NOT recorded as a finding there: `deinit_profile_validators` stands on no line of that diff, and a defect on an unchanged line is not a finding under a diff-scoped review.

#tool-validators