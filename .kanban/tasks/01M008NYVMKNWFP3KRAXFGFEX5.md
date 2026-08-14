---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m00gaxhm14cw2dxm1pay995a
  text: |-
    ### research

    Read the engine after `^apb04az`. What I found:

    - `scope/resolve.rs::filter_resolved_scope` drops every ignore-matched path with a
      DEBUG log only. The path never becomes an `ExcludedFile`, so nothing downstream
      knows it existed. A `.reviewignore` of `*` therefore lands on the empty-scope
      branch in `synthesize` and renders "Nothing in scope to review." — the exact
      gap message the card says must never cover a deliberate exclusion.
    - `scope/fixtures.rs::ExcludedFile` is the existing "scope stage dropped this
      file, here is why" type. It is reported per file and joins
      `ReviewCounts::skipped_files`. Only the validator-fixture split produces one.
    - `synthesize::render_scope` writes the scope line `^apb04az` added. Its
      `not_reviewed` count is `counts.skipped_files.len()`, so an ignore exclusion
      that joins that list surfaces on the scope line with no parallel mechanism.
    - Clippy gate `too-many-arguments-threshold = 7`, and `synthesize` already takes
      exactly 7 arguments. So the ignore exclusions must ride on the existing
      `excluded` list, not on a new parameter.
    - `ReviewedScope` carries only the REVIEWED file count. "every file in scope was
      excluded" is not provable from it, because a resolved file that matched no
      validator is neither reviewed nor excluded. The resolved total has to be
      plumbed for the claim to be true rather than inferred.

    Plan: move `ExcludedFile` to its own `scope/excluded.rs` with an `ExclusionKind`
    (`ValidatorFixture` | `ReviewIgnore`); `filter_resolved_scope` returns the
    ignore exclusions; `scope_review` chains them ahead of the fixture exclusions and
    records the resolved total on the work-list; `synthesize` renders ignore
    exclusions grouped by pattern and states a full exclusion explicitly when
    `excluded == resolved`. Two real `run_review_over_agent` tests in `drive.rs`.
  timestamp: 2026-08-14T16:05:30.292714+00:00
- actor: claude-code
  id: 01m00h15jay44k9e05m7t9x35j
  text: |-
    ### implementation

    TDD. Both tests were written first and watched fail for the right reason. The
    RED output of the full-exclusion test is the bug the card names, word for word:

    ```
    > Scope: `review file src/lib.rs` — reviewed the whole of each named file. 0 file(s) reviewed, 0 not reviewed.

    Nothing in scope to review.
    ```

    The fork test failed on the exclusion REPORT alone — `src/lib.rs` was already
    the only reviewed file, so the `ignore` crate's re-inclusion semantics were
    correct all along. The gap was entirely in what the engine said about them.

    What landed:

    - `scope/excluded.rs` is a new module holding `ExcludedFile` (moved out of
      `scope/fixtures.rs`, which is about fixtures, not about every exclusion) plus
      a new `ExclusionKind` — `ReviewIgnore` or `ValidatorFixture`. One list, two
      kinds, so the report can tell them apart and `synthesize` keeps its 7
      arguments (the clippy `too-many-arguments` gate stands at 7).
    - `filter_resolved_scope` now returns a `ScopeFiles` carrying the reviewable set
      AND one `ExcludedFile` per ignored path, each with the excluding pattern and
      the ignore file it came from. It used to drop them at DEBUG and say nothing.
    - `WorkList::resolved_files()` records how many files the scope REACHED, read
      before the fixture split narrows it. That is the denominator, so "every file
      in scope was excluded" is a fact the engine proves rather than infers — a
      resolved file no validator matched is neither reviewed nor excluded, so
      reviewed + excluded does not answer it.
    - The report renders ignore exclusions GROUPED by pattern. One pattern routinely
      covers a whole directory (`.kanban/` in this repo), and a line per file would
      bury what the run actually reviewed. Every path still rides in
      `counts.skipped_files` for a consumer that wants them.
    - A full exclusion states itself:
      `> Every file in scope was excluded — 1 of 1 file(s) — so nothing was left to
      review. The exclusions above are deliberate: this is a clean review, not an
      empty scope, a failed run, or a size-cap skip.`
      It and "Nothing in scope to review." are mutually exclusive by construction —
      the first needs exclusions, the second needs none.
    - `describe_glob` names the ignore file by NAME, not by absolute path. Both
      files sit at the repo root, so the path added nothing and differed per
      machine. The report now reads ``* (from .reviewignore)``.

    The two out-of-scope reasons stay separate, which is what `^apb04az` made
    possible to get wrong: an IGNORED file is resolved, then excluded, and is
    reported; an UNCHANGED file under a diff op is never resolved, and the scope
    line's "reviewed the diffs only" is what accounts for it. Neither is counted as
    the other.

    Discovery worth keeping: the shipped-fixture test
    `review_file_excludes_a_shipped_builtin_fixture_under_a_shadowing_user_set` is
    itself a full exclusion (one resolved file, one excluded), so it now renders the
    new line too. That is correct — a fixture exclusion covering the whole scope is
    as deliberate as an ignore one.
  timestamp: 2026-08-14T16:17:39.402351+00:00
- actor: claude-code
  id: 01m00h1b6kebwp8dvaejh496d2
  text: |-
    ### implement — changed
    - evidence: 11 files changed + 1 new (`crates/swissarmyhammer-validators/src/review/scope/excluded.rs`). `cargo nextest run -p swissarmyhammer-validators` 722 passed, 0 failed. `cargo nextest run -p swissarmyhammer-tools` 1521 passed, 0 failed. `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-14T16:17:45.171046+00:00
- actor: claude-code
  id: 01m00hkywk4ss89bv9sp19br5q
  text: |-
    ### test — green
    - evidence: `cargo nextest run --workspace` — 14197 passed, 0 failed, 0 skipped (run twice for stability). `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - confirmed `review_file_excludes_a_shipped_builtin_fixture_under_a_shadowing_user_set` is a deliberate full exclusion: `Scope::File(SHIPPED_BUILTIN_FIXTURE)` resolves exactly one file, and the fixture split excludes that same one file, so `excluded.len() == scope.resolved == 1` and `render_full_exclusion` fires by construction. The test's own assertions (`skipped_files == [fixture]`, `contains("validator fixture")`, `!contains("Nothing in scope to review.")`) hold and do not conflict with the new line.
    - next: none
  timestamp: 2026-08-14T16:27:55.155402+00:00
position_column: doing
position_ordinal: '8280'
title: A .reviewignore that excludes everything is a valid clean review, not an empty scope
---
When `.reviewignore` deliberately excludes every file in scope, that is a correct and wanted outcome: there is nothing to review, and the report must say so and pass. It must NOT read as an error, and it must not be confused with the failure modes that also produce an empty result.

The motivating use is a forked open-source repository: ignore the upstream tree, review only your own patches.

```
*
!src/mypatches/
!src/mypatches/**
```

Ignoring everything is the degenerate case of that, and it has to work.

## Two empty results that must never be confused

| cause | verdict |
| --- | --- |
| `.reviewignore` (or `.gitignore`) excluded every file | **valid clean** — report what was excluded and why, and pass |
| the scope resolved to nothing, an agent stalled, files were skipped for the size cap, or the run died | **NOT clean** — report the gap |

The second kind has bitten this board already. `^0fn6dbf` moved into `review` with zero findings recorded because its agent stalled, and zero findings read as a clean pass when it was no review at all. `^07pmgmx` was the same shape one layer down: a shipped fixture left the scope silently, and the engine answered "Nothing in scope to review." — the exclusion never surfaced.

So this card is not "allow empty". It is: **name the cause of an empty result, every time.** An exclusion is a reported exclusion with its reason and its pattern; anything else is a gap.

## The wiring is sound; the coverage is not

`crates/swissarmyhammer-validators/src/review/ignore.rs` builds the matcher from `.gitignore` layered under `.reviewignore`, using the `ignore` crate — git's own semantics, `!` negation and directory patterns included. `.reviewignore` is added last so its negations win. `drive.rs` already asserts that an excluded fixture is a REPORTED exclusion and never "Nothing in scope to review."

Six tests cover the matcher: default creation, byte-for-byte preservation of user edits, no-ignore-files, a directory pattern, a `!` negation, and `.gitignore` honored. Two more cover scope-level exclusion.

**None of them excludes the entire file set.** The untested path is exactly the one the fork workflow depends on, and its failure mode is the worst kind — a review that reports clean because it looked at nothing.

`tool_rules::is_inert()` exists to keep the "Nothing in scope to review." line honest, but nothing drives a FULLY excluded scope end to end.

## Verified against real git, 2026-08-13

`*` behaves as needed, and re-inclusion needs every parent directory — the usual gitignore trap applies:

| pattern | `src/lib.rs` | `vendor/dep.rs` | `README.md` |
| --- | --- | --- | --- |
| `*` | ignored | ignored | ignored |
| `*` + `!README.md` | ignored | — | re-included |
| `*` + `!src/` + `!src/lib.rs` | re-included | ignored | — |

## What to do

- A fully excluded scope returns a clean report that states every file was excluded, and by which pattern from which ignore file.
- Distinguish it in the report from an empty scope, a stall, and a size-cap skip. Those stay gaps.
- Add the missing test: a real review over a repo whose `.reviewignore` is `*`, asserting the report reads as a deliberate full exclusion and passes, and does not read as an ordinary clean pass.
- Add a second test for the fork shape — `*` plus a re-included subtree — asserting only the subtree is reviewed.

## Done when

- `.reviewignore` of `*` gives a clean, passing review that names the exclusion.
- No empty result anywhere reports clean without naming its cause.
- Both tests drive the production review path, not a matcher unit test.

#tool-validators