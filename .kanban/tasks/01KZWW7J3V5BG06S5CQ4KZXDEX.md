---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzxhnkxhsvhg8m4mbqjgcp1s
  text: |-
    Implemented, following commit 59f31b4f6 (sibling `^w6ypb8b`) exactly.

    ## Snapshots — byte-identity verified

    Each file recovered with `git show 59bd9ae5c~1:<path>`, written under
    `crates/mirdan/retired-validators/<set>/fixtures/`, then compared against the
    git blob with `cmp` and `sha256`. All four are IDENTICAL:

    - `e1bac79977102540320ba153af8e77c7eded70435e00be3a29864574a4bb490d`
      code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl (434 bytes)
    - `cc033268a5aa0df9528e755eb19fff14ca4d1cb6e5a6a85b27a26bea1cc457a5`
      code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl (1375 bytes)
    - `2d041798abda67315b9603eda2d32204363448ac22a48507f592f1448422acad`
      duplication/fixtures/duplication-parsed.fail.rs.tmpl (1406 bytes)
    - `65709779c95d7daf96bbecd56d37e5eef761cc9327ebee380ec3fa5a21035c7d`
      duplication/fixtures/duplication-parsed.pass.rs.tmpl (3742 bytes)

    The check was run again after all mutation testing, and gives the same four
    hashes.

    ## The empty `duplication/fixtures/` directory does NOT matter

    `59bd9ae5c` removed `builtin/validators/duplication/fixtures/` entirely because
    it became empty, while `code-hygiene/fixtures/` still ships many files. The
    file-grain prune removes files only, so a deployed store keeps an empty
    `duplication/fixtures/` after the prune. That costs nothing, and the reason is
    structural, not incidental:

    - `swissarmyhammer-validators/src/validators/parser.rs::require_ruleset_layout`
      is the whole set-layout contract: `VALIDATOR.md` plus `rules/`. `fixtures/`
      is not checked at all, so an empty one cannot make a set malformed.
    - The loader reads the store root one level deep and needs `VALIDATOR.md`, so
      `<set>/fixtures/` is never a set candidate and never a phantom rule.
    - Both readers of a fixtures directory — `doctor.rs::find_fixture` and
      `review/tool_health.rs::fixture_digest` — answer identically for an empty and
      an absent directory.
    - `loader.rs::fixture_dirs` computes `base_path.join("fixtures")` whether or
      not the directory exists, and the review-scope exclusion is a path prefix
      test, never a listing.

    Confirmed by the real binary: after `sah init user` the directory is present
    and empty, and doctor is unaffected.

    ## Report wording corrected

    `install_profile_validators` labelled this prune "retired validator rule(s)".
    With four fixtures now flowing through it, that message named a fixture as a
    rule. Changed to "retired validator file(s)" — the vocabulary
    `RETIRED_VALIDATOR_FILES` and `prune_unmodified_retired_files` already use. No
    test asserted the old string.

    ## Proof

    Real binary, `sah init user`, throwaway HOME (the user's real `~/.validators/`
    was never touched):

    - Store holding all four stale fixtures: reports
      "Removed retired validator file(s): ..." naming all four, and all four are
      gone; the still-shipping sibling `code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl`
      is present.
    - Store with one fixture edited: the edited copy survives byte for byte, the
      other three are pruned.

    Both halves are also covered by tests, and each was proved load-bearing by
    mutation.
  timestamp: 2026-08-13T12:31:06.161406+00:00
- actor: claude-code
  id: 01kzxhp28a47zqkza6astj34bq
  text: |-
    ## Mutation proof — every new test is load-bearing

    `^w6ypb8b` inherited a test that passed even when the prune did nothing. Each
    new test here was mutated to check it cannot do the same. Every mutation was
    reverted after measuring.

    Mutation A — `prune_unmodified_retired_files` skips every `fixtures/` entry
    (the prune exists but never fires for a fixture):

    - `init_profile_refresh_prunes_all_four_unmodified_retired_fixtures` FAILS at
      "an unmodified retired fixture must be pruned by refresh".
    - `init_profile_refresh_keeps_a_user_modified_retired_fixture` FAILS at
      "an unmodified retired fixture must still be pruned". This is exactly the
      assertion the inherited test lacked, so the "keeps that one" half cannot
      pass on a dead prune.

    Mutation B — the byte comparison is replaced by `true`, so the prune removes a
    file the user edited:

    - `init_profile_refresh_keeps_a_user_modified_retired_fixture` FAILS at
      "a user-modified retired fixture must never be pruned".

    Mutation C — one fixture entry removed from `RETIRED_VALIDATOR_FILES`:

    - `test_retired_files_are_the_two_parsed_rules_and_their_four_fixtures` FAILS.
    - `test_every_retired_tool_rule_has_its_fixtures_retired_with_it` FAILS
      (1 fixture, expected 2).
    - `init_profile_refresh_prunes_all_four_unmodified_retired_fixtures` FAILS.

    Order of work was RED first: the four tests were written and watched fail
    before any snapshot or table entry existed.

    ## Green

    - `cargo nextest run -p mirdan` — 516 passed, 0 failed, 0 skipped, including
      `test_no_retired_file_is_still_shipped_by_a_builtin_set`.
    - `cargo nextest run --workspace` — 14176 passed, 0 failed, 0 skipped.
    - `cargo fmt --check` clean.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean.

    Blast radius: `RETIRED_VALIDATOR_FILES` and `prune_unmodified_retired_files`
    are referenced only inside `crates/mirdan` (`retired_validators.rs`,
    `install/profile.rs`, `install/tests.rs`). No other crate reads either.
  timestamp: 2026-08-13T12:31:20.842475+00:00
- actor: claude-code
  id: 01kzxhpb6r979bgwx4hne3kjaa
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/mirdan/retired-validators/code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl (new), crates/mirdan/retired-validators/code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl (new), crates/mirdan/retired-validators/duplication/fixtures/duplication-parsed.fail.rs.tmpl (new), crates/mirdan/retired-validators/duplication/fixtures/duplication-parsed.pass.rs.tmpl (new), crates/mirdan/src/retired_validators.rs, crates/mirdan/src/install/profile.rs, crates/mirdan/src/install/tests.rs. Workspace 14176 tests pass; fmt and clippy clean.
    - next: /review
  timestamp: 2026-08-13T12:31:30.008075+00:00
position_column: doing
position_ordinal: '8280'
title: Prune the four retired tool-rule fixtures from a deployed validator store
---
`^w6ypb8b` added `RETIRED_VALIDATOR_FILES` to
`crates/mirdan/src/retired_validators.rs`, so a rule file deleted from a
still-shipping builtin set is now pruned out of a deployed store when the user
never touched it.

That card named only the two RULE files. Commit `59bd9ae5c` (card `^wwb6hk7`)
also deleted the four FIXTURE files those rules used:

- `builtin/validators/code-hygiene/fixtures/no-commented-code-parsed.fail.rs.tmpl`
- `builtin/validators/code-hygiene/fixtures/no-commented-code-parsed.pass.rs.tmpl`
- `builtin/validators/duplication/fixtures/duplication-parsed.fail.rs.tmpl`
- `builtin/validators/duplication/fixtures/duplication-parsed.pass.rs.tmpl`

Every store an earlier `sah init` wrote still holds all four. Nothing removes
them, exactly as nothing removed the two rule files.

They raise no `sah doctor` row, because doctor enumerates rules and reads a
fixture only through the rule that names it. No rule names these four any more,
so they are inert bytes. That is why `^w6ypb8b` was complete without them.

## What to build

Add the four fixtures to `RETIRED_VALIDATOR_FILES`. The mechanism already
exists and needs no change: recover each file's exact bytes with
`git show 59bd9ae5c~1:<path>`, write them as byte-frozen snapshots under
`crates/mirdan/retired-validators/<set>/fixtures/`, and add one
`RetiredSetFile` entry for each.

## Done when

- A store holding all four stale fixtures loses all four after `sah init`, and
  a store whose copy of one was edited keeps that one.
- The existing guard tests still pass, in particular
  `test_no_retired_file_is_still_shipped_by_a_builtin_set`.

#tool-validators