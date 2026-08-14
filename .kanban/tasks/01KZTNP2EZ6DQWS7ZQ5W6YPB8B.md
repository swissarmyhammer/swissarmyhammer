---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzwvrd2vmgmfmgkkarcwmycm
  text: |-
    Research done.

    Existing set-level idiom (`crates/mirdan/src/retired_validators.rs`):
    - `RetiredFile { relative_path, content }` + `RetiredSet { name, files }`, a `RETIRED_VALIDATOR_SETS` static, each `content` an `include_str!` of a byte-frozen snapshot under `crates/mirdan/retired-validators/<set>/...`.
    - `prune_unmodified_retired_sets(store_root)` compares the deployed dir against the snapshot (file count, every path present, every byte equal) and calls `store::remove_if_exists` only on an exact match. Returns removed names.
    - Called once, from `install_profile_validators` (`crates/mirdan/src/install/profile.rs`), BEFORE the selected sets are copied in, and reported as an `InitEvent::Action { verb: "Removed" }`.

    The snapshot tree is never scanned by any loader: `crates/mirdan/build.rs` embeds only `../../builtin/validators`, so `retired-validators/` reaches nothing but `include_str!`. That is why `retired-validators/injection/rules/injection.md` must stay byte-frozen — editing it breaks the match and the prune stops firing.

    Snapshots recovered from git at `59bd9ae5c~1`, verified byte-identical by sha256:
    - `builtin/validators/duplication/rules/duplication-parsed.md` — 17288 bytes, sha256 7b361a52a0b150d2674b24a38f5380d1bbd06f5303b08bd1608bbb438fd1a73b
    - `builtin/validators/code-hygiene/rules/no-commented-code-parsed.md` — 9013 bytes, sha256 5212b5fa4f622df6f8e2fb089dfc080003a1c1842aca0db4f94efee08d191c6b

    Both parent sets still ship other rules (`duplication/rules/duplication.md`, `rust.md`, `swift.md`; `code-hygiene/rules/*`), so removing one rule never empties a set directory.

    Live confirmation of the gap on this machine: `review dump validators` reads the deployed store, and its output still carries `### duplication-parsed` and `### no-commented-code-parsed` rule bodies from `~/.validators/`.

    Discovered, NOT in this card's scope: commit 59bd9ae5c also deleted four fixture files —
    `code-hygiene/fixtures/no-commented-code-parsed.{fail,pass}.rs.tmpl` and
    `duplication/fixtures/duplication-parsed.{fail,pass}.rs.tmpl`.
    They are equally stale in a deployed store. They raise no `sah doctor` row (doctor enumerates rules, not fixtures), so the card's done-when is met without them. Filing a follow-up card.
  timestamp: 2026-08-13T06:08:08.795643+00:00
- actor: claude-code
  id: 01kzxg8ym176nb27fg877nz1s6
  text: |-
    Resumed the abandoned 06:04 session. Its uncommitted work was verified independently, not taken on trust.

    Verified with my own commands:
    - Both snapshots are byte-identical to `59bd9ae5c~1`. Piped `git show 59bd9ae5c~1:<path>` through sha256 and compared against the files on disk: duplication-parsed.md = 7b361a52a0b150d2674b24a38f5380d1bbd06f5303b08bd1608bbb438fd1a73b (17288 bytes), no-commented-code-parsed.md = 5212b5fa4f622df6f8e2fb089dfc080003a1c1842aca0db4f94efee08d191c6b (9013 bytes). Both match. The prune fires.
    - The code changes are complete and coherent, not a half-edit. `RetiredSetFile` + `RETIRED_VALIDATOR_FILES` + `prune_unmodified_retired_files`, wired into `install_profile_validators` through a new `report_pruned` helper that both grains share, plus module and `lib.rs` docs updated for the two grains.

    The prior session never watched its tests fail, so I proved both halves load-bearing by mutation:
    - Mutation A, prune body made a no-op: `prune_removes_an_unmodified_retired_file`, `prune_removes_every_unmodified_retired_file_at_once`, and the real-path `init_profile_refresh_prunes_unmodified_retired_rule_file_but_keeps_user_modified_copy` all FAIL. The removal half is real.
    - Mutation B, byte comparison forced to `true`: `prune_leaves_a_user_modified_retired_file_untouched` and the same real-path test FAIL. The honesty half is real.

    Defect found and fixed in the inherited tests: `prune_removes_only_the_named_file_from_a_still_shipping_set` passed under BOTH mutations. It asserted only that the manifest and a sibling survived, never that the named file was removed, so "only" was vacuously true for a no-op prune. Added the missing `!deployed.exists()` assertion. This is the `test-integrity/no-test-cheating` rule: every test must be able to fail.

    End-to-end proof with the real `sah` binary, in a throwaway HOME (the real `~/.validators/` was never touched and still holds both stale files):
    - Aged a store by injecting both rule files exactly as `59bd9ae5c~1` shipped them. `sah doctor` then reported the two degraded rows the card describes, `tool missing: bash: : command not found`.
    - `sah init user` printed `+ Removed retired validator rule(s): duplication/rules/duplication-parsed.md, code-hygiene/rules/no-commented-code-parsed.md`. Both files gone from disk; the still-shipping siblings (duplication.md, rust.md, swift.md; no-commented-code.md and the rest of code-hygiene) all survived.
    - `sah doctor` after: no duplication-parsed or no-commented-code-parsed row.
    - Re-aged with an edited copy of duplication-parsed: `sah init user` pruned only no-commented-code-parsed and left the edited file in place with the edit intact.

    Green: 513/513 mirdan tests pass under cargo nextest, cargo fmt --check clean, cargo clippy --workspace --all-targets -D warnings clean.

    The follow-up card for the four stale fixture files is already on the board as ^4kzxdex.

    ### implement — changed
    - evidence: 6 files — crates/mirdan/src/retired_validators.rs, crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/tests.rs, crates/mirdan/src/lib.rs, and the two new byte-frozen snapshots under crates/mirdan/retired-validators/
    - next: /review
  timestamp: 2026-08-13T12:06:42.561980+00:00
- actor: claude-code
  id: 01kzxgrp9yd0qx9kmnvc7fvp68
  text: |-
    ### review — clean
    - evidence: review sha 59f31b4f6~1..59f31b4f6 — 0 findings (16 validator passes attempted, 0 failed, 0 files skipped). No prior open findings on this task.
    - next: none; task moved to done.
  timestamp: 2026-08-13T12:15:18.334659+00:00
- actor: claude-code
  id: 01kzxgsr5ynczxncm2334rnze2
  text: |-
    ### finish iteration 1 — clean
    - Resumed from an abandoned session that started this card at 06:04 and died before committing, leaving four modified mirdan files and the two snapshots uncommitted. Its work was NOT taken on trust.
    - implement: changed — 6 files. The load-bearing check passed: both snapshots are byte-identical to what 59bd9ae5c~1 shipped, verified by piping `git show` through sha256 (duplication-parsed.md 7b361a52…1a73b, 17288 bytes; no-commented-code-parsed.md 5212b5fa…91c6b, 9013 bytes). A single wrong byte would stop the prune firing forever, silently.
    - One real defect found in the inherited work. Both halves were proved load-bearing by mutation, and `prune_removes_only_the_named_file_from_a_still_shipping_set` passed under BOTH mutations — it asserted only that the manifest and a sibling survived, never that the named file was removed, so "only" was vacuously true for a prune that does nothing. The missing assertion is added. This is the test-integrity/no-test-cheating rule: every test must be able to fail.
    - Proved end to end with the real binary in a throwaway HOME. An aged store reproduces the card's exact doctor rows (`tool missing: bash: : command not found`); `sah init user` reports `Removed retired validator rule(s): duplication/rules/duplication-parsed.md, code-hygiene/rules/no-commented-code-parsed.md`; both files go, every still-shipping sibling survives, and doctor afterwards shows neither row. Re-aged with an EDITED copy, the same command pruned only the unmodified file and left the edit intact.
    - test: green — 513/513 mirdan tests, fmt and clippy clean.
    - commit: 59f31b4f6
    - review: clean — 0 findings over 59f31b4f6~1..59f31b4f6, 16 passes attempted, 0 failed, 0 skipped. The engine raised nothing against either byte-frozen snapshot, so nothing had to be set aside on that ground. Task moved to done.

    The user's real ~/.validators/ was never touched and still holds both stale files. To clean it, after this lands and sah is rebuilt: `sah init user`. Edited copies stay, by design.
  timestamp: 2026-08-13T12:15:53.022799+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffff980
title: Prune a retired RULE file from a deployed validator store
---
`mirdan::retired_validators::prune_unmodified_retired_sets` prunes a whole
retired SET directory from a deployed store (`~/.validators/` or
`./.validators/`). It has no facility for a retired RULE FILE inside a set that
still ships.

So a rule deleted from `builtin/validators/<set>/rules/` survives in every store
an earlier `sah init` wrote. `install_profile_validators` overwrites each
embedded active file and adds nothing, so nothing removes the leftover. The
loader then reads the stale rule at user or project precedence and the rule
keeps running.

Measured on this machine after ^wwb6hk7 deleted `duplication-parsed` and
`no-commented-code-parsed`:

- `sah doctor` with the real `$HOME`: two degraded rows, `code-hygiene/no-commented-code-parsed`
  and `duplication/duplication-parsed`, each reading
  `tool missing: bash: : command not found`, because `~/.validators/` still holds
  the two rule files and `SAH_BIN` no longer reaches the scripts.
- `sah doctor` with `$HOME` pointed at an empty directory: neither rule appears,
  and every remaining tool rule reports `tool present; fixtures pass`.

## What to build

Extend the retired snapshot to the FILE level, with the same honesty contract
the set-level prune keeps: an exact byte-for-byte match against the shipped
snapshot removes the file; any difference leaves it alone. Add the two rule
files above as the first entries.

## Done when

- A store holding a stale `duplication/rules/duplication-parsed.md` and
  `code-hygiene/rules/no-commented-code-parsed.md` loses both after `sah init`,
  and a store whose copy of either was edited keeps it.
- `sah doctor` on a machine whose store was written before ^wwb6hk7 reports no
  tool rule for duplication or commented code.

#tool-validators