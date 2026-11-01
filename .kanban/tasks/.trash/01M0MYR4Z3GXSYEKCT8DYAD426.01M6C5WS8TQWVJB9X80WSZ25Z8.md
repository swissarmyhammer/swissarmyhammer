---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0n4s728v9y20tackbcrxjac
  text: |-
    ### Second independent occurrence — this is reproducible, not a one-off

    Commit `c097c1548`, reviewed in task ^t78sqr4. Same fault, same shape as the `06e7a2fce` case that opened this card.

    Measured: 15 files changed. The engine counted **3 reviewed** and **8 excluded** (6 `.kanban/`, 2 validator fixtures) = 11. Four files unaccounted for.

    The four missing files are **all markdown**, and in this commit they were the files that carried the entire deliverable — the two decisions this card existed to make lived in `builtin/validators/swift/rules/optionals.md`, `builtin/validators/swift/rules/idioms.md`, `builtin/validators/swift/rules/error-handling.md`, and `builtin/validators/code-hygiene/rules/idioms-swift.md`.

    This raises the severity. In the first occurrence the unaccounted files were incidental docs. Here the unreviewed markdown WAS the change. The reviewer only caught the claims because they re-measured all four files by hand against real swiftformat, outside the engine. That is not a gate; that is one reviewer being thorough.

    Two occurrences, both with every unaccounted file being markdown, makes "markdown is silently dropped from diff-scoped review" the leading hypothesis — not the "counting quirk" this card originally guessed. Test that hypothesis FIRST.

    If markdown is genuinely never reviewed on `review sha` / `review working`, then every prompt-rule change, every VALIDATOR.md change, and every doc change we have shipped has gone through an empty gate while reporting clean. That is a much bigger defect than a tally that does not add up, and it deserves its own card and its own priority.
  timestamp: 2026-08-22T16:27:38.952428+00:00
- actor: claude-code
  id: 01m0n8yzznr064h5qch4vdk5v2
  text: |
    ### reproduction 4 — markdown dropped from a diff-scoped review

    `{"op": "review sha", "sha": "HEAD~1..HEAD"}` over commit `0182f3bf` on ^qs32yvp.

    The commit holds 11 files: 4 `.kanban/`, 4 markdown, 3 Rust.

    The engine answered "3 file(s) reviewed, 4 not reviewed" with `counts.findings: 0` and `counts.attempted: 7`. The 4 not reviewed are named in `skipped_files`, and all 4 are `.kanban/`, excluded by `.reviewignore`.

    The 4 markdown files are in NEITHER count:

    - `builtin/validators/code-hygiene/VALIDATOR.md`
    - `builtin/validators/code-hygiene/rules/idioms-swift.md`
    - `builtin/validators/swift/VALIDATOR.md`
    - `builtin/validators/swift/rules/idioms.md`

    3 reviewed + 4 skipped = 7, and 11 - 7 = 4 files that no line of the report accounts for. `skipped_files` names none of them, so the report carries no signal that anything was dropped.

    A hand review of those 4 files against real swiftformat 0.62.1 found 3 findings, so `findings: 0` was not a clean result. Same shape as reproductions 1-3: markdown is silently absent, and the tally hides the absence.
  timestamp: 2026-08-22T17:40:42.613157+00:00
- actor: claude-code
  id: 01m0nena5sse56zzxz8etj7g7x
  text: |-
    ## Reproduction 7 — `review sha HEAD~1..HEAD` over `0334b79e6`, from ^052w80d

    The tally does not close, and it does not close in a NEW way: five files are neither reviewed nor named as excluded.

    `{"op": "review sha", "sha": "HEAD~1..HEAD"}` answered:

    ```
    counts: { findings: 0, confirmed: 0, refuted: 0, attempted: 7, failed: 0, skipped: 0, skipped_files: [6 .kanban paths] }
    markdown header: "4 file(s) reviewed, 6 not reviewed."
                     "6 file(s) not reviewed — excluded by an ignore rule: .kanban/ (from .reviewignore)"
    ```

    `git diff --stat 379079d84 0334b79e6` reports **15 files changed**. The report accounts for 10 of them: 4 reviewed, 6 excluded. Five files are missing from both counts:

    - `builtin/validators/swift/VALIDATOR.md`
    - `builtin/validators/swift/rules/idioms.md`
    - `builtin/validators/swift/rules/immutability.md`
    - `builtin/validators/swift/rules/initialization.md`
    - `builtin/validators/swift/rules/preconditions.md`

    Every one is markdown. Every one is the substance of the commit — this commit's whole claim is a set of prompt-rule text edits, and 545 of its insertions are prose.

    Three faults stand in the one answer:

    1. **Markdown is not reviewed.** Same as reproductions 1-6.
    2. **The header arithmetic is wrong.** `4 reviewed + 6 not reviewed = 10`, against 15 changed. The header states "6 not reviewed" when 11 were not reviewed.
    3. **The five are not reported as skipped either.** `skipped_files` names only the six `.kanban` paths. A caller reading `skipped_files` to learn what went unread learns nothing about the markdown, so the gap is invisible to an automated consumer as well as to a reader.

    Fault 2 is a tighter version of the arithmetic fault logged in reproduction 6 (`counts.skipped: 0` beside a six-entry `skipped_files`). Both say the same thing: the counts are not derived from the file set the run actually walked.

    Consequence for ^052w80d, iteration 2: the engine returned `findings: 0` over a commit whose entire delta is prompt-rule prose. Three findings were raised by hand against those five files, one of them a collision between two rules the commit's own sweep declared to share no shape. A thin `findings: 0` over a markdown commit remains worth nothing.
  timestamp: 2026-08-22T19:20:16.825868+00:00
- actor: claude-code
  id: 01m0ngvqjkchk06zhecnn0ws8c
  text: |-
    ## Further reproduction — commit `d4a6da772`, task ^m7ynz9c (2026-08-22)

    Same split, same silence. This commit is a documentation commit, and the engine did not open the document.

    `review sha HEAD~1..HEAD` over commit `d4a6da772`:

    - The commit touches 12 files.
    - 10 are `.kanban/` — the report names all 10 and the `.reviewignore` rule that excluded them.
    - 2 are eligible: 1 `.md` and 1 `.rs`.
    - The report says "1 file(s) reviewed, 10 not reviewed".

    1 reviewed + 10 excluded = 11, not 12. One file is in NO set. It is the markdown file:

    ```
    builtin/_partials/project-types/swift.md
    ```

    The 1 reviewed file is the 1 `.rs` file. `counts` reported `findings: 0, confirmed: 0, refuted: 0, attempted: 7, failed: 0, skipped: 0`, and `skipped_files` listed only the 10 `.kanban/` paths.

    `counts.skipped` is `0` while `counts.skipped_files` holds 10 paths. This is the second arithmetic fault the Reproduction 4 section records, seen again.

    ### The validator rosters confirm the cause

    `list validators` returns 13 validators. NO validator names `*.md` or `**/*.md` in its `match_globs`. The 12 that match source code name 37 code extensions, and markdown is not one of them. So the drop is not a tally fault for this class of file — no validator can match a markdown file at all.

    ### What this cost

    The whole substance of `d4a6da772` is `builtin/_partials/project-types/swift.md`: a new tool-to-config table, the Airbnb plugin command lines, two documented contradictions with our own shipped gates, and four swiftformat options with their defaults. The engine returned `findings: 0` over a commit whose reviewable content it never opened.

    A hand review ran every command line in the partial. Each documented claim measured TRUE, so the document is correct — but the engine did not establish that, and could not have. The hand review also found one defect, and it was in the `.rs` file the engine DID open: a regression test whose name promises agreement with a shipped validator and whose body only checks that six strings are present. The engine reported `findings: 0` on that file.
  timestamp: 2026-08-22T19:58:44.307136+00:00
- actor: claude-code
  id: 01m0ngypa0fnx4nctrdp5m82b2
  text: |-
    ### ROOT CAUSE FOUND — stop investigating, start fixing

    Found while reviewing `d4a6da772` (task ^m7ynz9c). This card can stop hunting.

    **`list validators` returns 13 validators. NOT ONE of them names `*.md` in its `match_globs`.**

    That is the whole bug. Markdown is not miscounted, not dropped by a filter, not excluded by `.reviewignore`. **No validator can match a markdown file**, so the engine has nothing to run against one, and the file falls out of every tally because no code path ever considers it.

    This also explains the arithmetic fault that opened this card. The tally is built from files that matched at least one validator, plus files explicitly skipped. A file that matched nothing is in neither set, so `reviewed + not_reviewed < total` — and the difference is always exactly the markdown.

    **Second, separate fault, confirmed again here:** `counts.skipped` reads `0` while `skipped_files` holds 10 paths. Two different fields disagree about the same thing.

    **Eight reproductions across this project's commits**, every one with the unaccounted files being markdown:
    `06e7a2fce`, `c097c1548`, `f79727e2e`, `0182f3bf`, `ae24e8474`, `379079d84`, `0334b79e6`, `d4a6da772`.

    ### Why this matters more than a tally

    Our validator RULES are markdown. Our VALIDATOR.md files are markdown. Our partials are markdown. Our docs are markdown. Every one of those changes has passed through review reporting `findings: 0` — and that zero meant "nothing could look", not "nothing is wrong".

    On `d4a6da772` the reviewed file count was **1 of 12**, and the one file the engine opened was not the substance of the change. Across this project, every real finding on a markdown commit came from a human-directed hand measurement, not from the engine.

    ### The work is now clear

    1. Decide what a markdown validator is. Some rules genuinely apply — dead links, stale command lines, contradictions with shipped config. Prose style is a separate question.
    2. Give at least one validator a `match_globs` that includes `*.md`, so markdown enters the pipeline at all.
    3. Fix the tally so `total == excluded + reviewed + not_reviewed`, with every file in exactly one set, and make the report name WHY each not-reviewed file was not reviewed. "Matched no validator" must be a stated outcome, never silence.
    4. Fix `counts.skipped` to agree with `skipped_files`.

    Item 3 is the safety net: even with no markdown validator, the report must SAY that a file matched nothing. Silence is what let this run for eight commits.
  timestamp: 2026-08-22T20:00:21.312470+00:00
position_column: todo
position_ordinal: ffee80
title: 'review engine: markdown files are silently dropped from diff-scoped review'
---
Found while reviewing commit `06e7a2fce` (task ^3fx5bny). The review engine's own file accounting does not add up.

## What was measured

`review sha HEAD~1..HEAD` over commit `06e7a2fce`:
- The commit touches 27 files.
- The report says 14 `.kanban/` files were excluded by `.reviewignore`, plus 2 `idioms-swift` fixture templates excluded as validator fixtures. That leaves 11 eligible.
- The report then says 8 reviewed and 16 not reviewed — 24 total.

24 is not 27, and 8 is not 11. Three files are unaccounted for:
- `builtin/validators/code-hygiene/VALIDATOR.md`
- `builtin/validators/code-hygiene/rules/idioms-swift.md`
- `doc/src/concepts/validators.md`

All three are markdown.

## Why this is probably a reporting fault, not an analysis gap

The same report says `failed: 0` and `skipped: 0`. No validator errored, and none was dropped. So the likely fault is in how the tally counts files, not in what the engine actually reviewed.

That is a guess. Prove it. Do not close this card on the guess.

## Why it matters

A tally that does not close cannot be used as evidence of coverage. Today a reviewer has to notice the arithmetic by hand. If the engine really did skip three markdown files, every markdown change in every review has been unreviewed and nothing said so.

## Work

1. Reproduce. Run `review sha` over a commit with a mix of code, markdown, ignored paths, and fixtures. Record the real numbers.
2. Find where the counts are produced and where the excluded, reviewed, and not-reviewed sets are built. They are probably three separate walks that disagree.
3. Decide the invariant: `total == excluded + reviewed + not_reviewed`, with every file in exactly one set.
4. Make the invariant a test, not a comment.

## Acceptance

- A test asserts the tally closes for a mixed commit — code, markdown, `.reviewignore` paths, and fixtures.
- The report names WHY each not-reviewed file was not reviewed. "Not reviewed" with no reason is what made this hard to diagnose.
- If markdown was genuinely never reviewed, that is a separate defect. Say so plainly and raise its own card. #review-tool

## Reproduction 3 — commit `f79727e2e`, task ^qs32yvp (2026-08-22)

The cleanest reproduction so far. The split is exactly along file type, with no fixture exclusion to confuse it.

`review sha HEAD~1..HEAD` over commit `f79727e2e`:

- The commit touches 24 files.
- 10 are `.kanban/` — the report names all 10 and the `.reviewignore` rule that excluded them.
- 14 are eligible: 9 `.md` and 5 `.rs`.
- The report says "5 file(s) reviewed, 10 not reviewed".

5 reviewed + 10 excluded = 15, not 24. Nine files are unaccounted for, and they are the nine markdown files — every one of them:

```
builtin/validators/code-hygiene/VALIDATOR.md
builtin/validators/code-hygiene/rules/disallowed-constructs-swift.md
builtin/validators/code-hygiene/rules/idioms-swift.md
builtin/validators/swift/VALIDATOR.md
builtin/validators/swift/rules/concurrency.md
builtin/validators/swift/rules/error-handling.md
builtin/validators/swift/rules/idioms.md
builtin/validators/swift/rules/optionals.md
builtin/validators/swift/rules/value-semantics.md
```

The 5 reviewed are precisely the 5 `.rs` files. `counts` reported `attempted: 7, failed: 0, skipped: 0, findings: 0`, and `skipped_files` listed only the 10 `.kanban/` paths.

### This reproduction settles the open question

Reproduction 1 left it open whether the fault was in the tally or in the analysis. This one answers it: the reviewed set is `.rs` only. Markdown is not merely miscounted, it is **not reviewed at all**, and the report claims a clean result without saying so.

Per the acceptance criterion above — "If markdown was genuinely never reviewed, that is a separate defect. Say so plainly and raise its own card" — that condition is now met and the separate card is owed.

### What this cost

Commit `f79727e2e` is a documentation commit: its whole substance is the 9 markdown files. The engine returned `findings: 0` over a commit whose reviewable content it never opened. A hand review of the same 9 files found four defects, including a real Swift coverage hole (`preferForLoop` silent on the `forEach` + `if` shape whose prompt bullet the commit deleted). A `findings: 0` that means "nothing was read" is worse than an error, because it reads as a pass.

## Reproduction 4 — commit `379079d84`, task ^052w80d (2026-08-22)

Sixth commit measured, same split, same silence.

`review sha HEAD~1..HEAD` over commit `379079d84`:

- The commit touches 14 files.
- 6 are `.kanban/` — the report names all 6 and the `.reviewignore` rule that excluded them.
- 8 are eligible: 6 `.md` and 2 `.rs`.
- The report says "2 file(s) reviewed, 6 not reviewed".

2 reviewed + 6 not-reviewed = 8, not 14. The 6 "not reviewed" ARE the 6 `.kanban/` paths, so the 6 markdown files are in NO set at all — not reviewed, not excluded, not named anywhere in `markdown` or in `counts.skipped_files`:

```
builtin/validators/swift/VALIDATOR.md
builtin/validators/swift/rules/access-control.md
builtin/validators/swift/rules/immutability.md
builtin/validators/swift/rules/initialization.md
builtin/validators/swift/rules/naming-clarity.md
builtin/validators/swift/rules/preconditions.md
```

The 2 reviewed are precisely the 2 `.rs` files. `counts` reported `findings: 1, confirmed: 1, refuted: 1, attempted: 7, failed: 0, skipped: 0`, and `skipped_files` listed only the 6 `.kanban/` paths.

### A second arithmetic fault, in `counts` itself

`counts.skipped` is `0` while `counts.skipped_files` holds 6 paths. The scalar and the list disagree in the same object. Whatever walk fills `skipped_files` is not the walk that increments `skipped`. Add that to the invariant in step 3.

### What this cost

The commit adds eleven Swift prompt-rule bullets across five markdown files, plus one Rust test file. The engine's ONE finding is on the Rust file. Every bullet — the whole substance of the change — went unread. A hand review of the six markdown files found six defects, including a prompt-versus-prompt two-owner conflict between `immutability.md` and `idioms.md` in which one rule's DO is written word for word as the other rule's DON'T. That is the exact defect class the whole `tool-validators` project exists to remove, and the engine reported `findings: 1` on a different file.