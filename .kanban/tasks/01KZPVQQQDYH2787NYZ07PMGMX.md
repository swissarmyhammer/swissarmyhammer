---
assignees:
- claude-code
position_column: todo
position_ordinal: ffcb80
title: validator-fixture exclusion does not fire at runtime for the builtin layer
---
^4cc5y9b added an exclusion that takes a changed file under a loaded validator set's `fixtures/` directory out of the review work-list, and reports it in `skipped_files` with the reason "validator fixture". The card closed with a clean review.

**It does not fire at runtime for the builtin layer of this repository.**

Measured on 2026-08-10, during the review of ^2syfvyt (commit 758416086), which changes two shipped fixtures:

- `skipped_files` came back EMPTY and `skipped` was 0. Neither `.tmpl` fixture was named.
- The report carries no "N file(s) not reviewed" note.
- `review file builtin/validators/code-hygiene/fixtures/magic-numbers-python.fail.py.tmpl` answers **"Nothing in scope to review."** The engine prints that only when the excluded list is empty (`synthesize.rs:355-360`). So the file left the scope silently, which is the exact behaviour ^4cc5y9b exists to stop.

## The wiring is intact

The reviewer checked each hop rather than assuming: `review/scope/fixtures.rs` runs before any validator pairing (`scope.rs:570`), `ExcludedFile` reaches `WorkList::with_excluded` (`scope.rs:654`), and that reaches `not_reviewed_paths` (`synthesize.rs:310`). Nothing is disconnected.

## The likely cause

`ValidatorLoader::fixture_dirs()` names each LOADED set's `fixtures/` directory. At runtime the builtin layer is not this repository's `builtin/validators/` — it is the compiled-in copy, or a `sah init` snapshot under the user's home. So the fixture root the exclusion compares against never contains the repository path, and the containment test answers false.

## Why the tests did not catch it

^4cc5y9b proved acceptance in two halves, and the halves leave this gap between them:

- `a_changed_builtin_fixture_leaves_the_scope_and_source_stays` builds a loader with `builtin_loader()` and compares against the real repository root. That is not the loader a live review builds.
- `review_e2e_sha_excludes_a_validator_fixture_and_still_reviews_the_source` drives the production tool, but over a PROJECT-layer set at `<temp repo>/.validators/`. The project layer works. The builtin layer is untested end to end.

The card's own implement comment names the reason the halves exist: "the builtin layer's fixture root resolves to this repository's `builtin/validators/`, so a temp-repo e2e cannot host it." That reasoning is what left the gap.

## Done when

- A test drives a REAL review over a change to a shipped `builtin/validators/*/fixtures/*` file, through the same loader a live review builds, and proves the file is reported in `skipped_files` with the reason "validator fixture"
- `review file` on a shipped fixture answers with the exclusion and its reason, never "Nothing in scope to review"
- The runtime builtin fixture root is stated in the code, so a reader can tell which directory the containment test compares against

See [[real-path-tests-not-mocks]]. #tool-validators #objectivity