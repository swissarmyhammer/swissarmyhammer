---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m07r6ry8h0ma530ms5vncc9p
  text: |-
    Research done.

    Discovery: the code moved since the card was written. The index-side `duplicates` row does NOT carry `detail: None` now. It carries `detail: Some(snippet(&dup.chunk.text))` — a truncated copy of the counterpart text. The row thus gives a path, a similarity and a snippet, and still says nothing about which side the change touched. The gap the card names is still open.

    Second discovery: `find_duplicates_in` can return a counterpart in the SAME file (its doc says it compares "chunks in other files AND the other chunks of the same file"). So the row must not claim the counterpart is unchanged. `clone_sibling_row` CAN claim that, because it filters to members outside the files under review. The two sentences must therefore differ in their second clause, and only the first clause ("the change edited X") is shared.

    Plan:
    1. Extract `index_duplicate_row` (mirrors the existing `clone_sibling_row`), first with the current behavior, so the change is a refactor with no test movement.
    2. Add the two tests from the card. Prove RED.
    3. Add one shared `pair_direction` helper for the first clause, used by both duplicate probes, so no near-copy sentence is written twice.
    4. Rewrite the last paragraph of `duplication.md` to put the remedy on the changed side.
  timestamp: 2026-08-17T11:37:44.136166+00:00
- actor: claude-code
  id: 01m07rs8gtynsabsfvxdb57r27
  text: |-
    TDD record, with the measurements.

    RED, measured before the implementation. Both new tests failed, each for its own cause:
    - `an_index_duplicate_row_renders_the_side_the_change_edited` — left: `- probe `duplicates` on `src/a.rs`:\n  - src/b.rs:42 `dupe` @ 0.94 — fn dupe() {}\n\n`. The direction clause was absent.
    - `duplicates_returns_the_index_hit_for_a_duplicated_function` — `the row must name the changed side, got: ["pub fn compute(input: &[f64]) -> f64 { let mut total = 0.0; ...]`.

    Proof that each test holds ITS OWN cause: with `run_duplicates` unwired from `index_duplicate_row` but the new wording in the builder, the render test PASSED and the probe test FAILED. So the render test holds the wording, and the probe test holds that the production probe path uses the builder.

    GREEN after the implementation: 821 tests run, 821 passed, 0 skipped. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    The rendered row now reads:

        - src/b.rs:42 `dupe` @ 0.94 — the change edited src/a.rs; fix that side, not this copy — fn dupe() {}

    One shared helper, not a second sentence. `pair_direction` builds the clause "the change edited {edited}; {counterpart}". Both duplicate probes call it, so the sentence is written one time.

    The `clone-siblings` row text did not move. Measured with a temporary `assert_eq!` against the real fixture: `the change edited src/first.rs:1 `mean_square`; this near-copy of it is unchanged`. That is byte for byte the row `builtin/validators/completeness/rules/invariant-propagation.md` quotes, so that rule needs no edit. The temporary assertion was removed.

    Why the index-side row does NOT say the counterpart is unchanged: `find_duplicates_in` compares a file's chunks against the whole corpus, which holds the other chunks of that same file. A counterpart can therefore stand in a changed file. The row states the direction and makes no claim about the counterpart. Only `clone_sibling_row` can make that claim, because it filters to members outside the files under review.

    Blast radius: `render_probe_evidence` is `pub(crate)`. The two other places that name the `duplicates` probe — `review/fleet/tests/renderer.rs` and `swissarmyhammer-tools/tests/integration/review_fixture.rs` — read the result HEADER, never the row detail. No other consumer parses this detail.
  timestamp: 2026-08-17T11:47:49.914712+00:00
- actor: claude-code
  id: 01m07rse79axdwymycevsx3jcx
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-validators/src/review/probes.rs, builtin/validators/duplication/rules/duplication.md. RED then GREEN measured on both new tests; `cargo nextest run -p swissarmyhammer-validators` 821 passed, 0 skipped; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-17T11:47:55.753925+00:00
- actor: claude-code
  id: 01m07sfxgq3bbwc0xnwrprm7f0
  text: |
    ### test — green
    - evidence: `cargo nextest run --workspace` — 14163 passed, 0 failed, 0 skipped; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - Claim 1 (clone-siblings row byte-identical, so invariant-propagation.md needs no edit): VERIFIED. `pair_direction(&mirror.site, "this near-copy of it is unchanged")` = `format!("the change edited {edited}; {counterpart}")` produces the exact same string the old inline `format!("the change edited {}; this near-copy of it is unchanged", mirror.site)` produced — confirmed symbolically from the diff (the refactor only extracted the format string, did not change it). The doc's quoted row (`src/backup_writer.rs:88 ... — the change edited\n    src/writer.rs:41 ...`) matches the code's output byte-for-byte once the doc's manual line-wrap (a markdown code-block convention already used elsewhere in these rule files) is joined back to one line. No edit to invariant-propagation.md needed.
    - Claim 2 (index-side row correctly omits the "unchanged" claim; clone-sibling row correctly can make it): VERIFIED by reading both code paths. `find_duplicates_in` ranks a changed file's chunks against the *whole* corpus with no filter excluding other files under review, so `index_duplicate_row`'s counterpart can be a file this same diff also edits — correctly, it never claims "unchanged". `run_clone_siblings` explicitly filters candidates with `.filter(|dup| !under_review.contains(dup.chunk.file_path.as_str()))` before calling `clone_sibling_row`, so its sibling site is guaranteed to sit outside the changed set — the "unchanged" claim is justified there. Real distinction, not a defect.
    - Claim 3 (both new tests fail for their own cause): proved by breaking and reverting real source, not asserted. Broke the snippet text after the direction clause in `index_duplicate_row` (`"fix that side, not this copy —"` → `"BROKEN not this copy —"`): `an_index_duplicate_row_renders_the_side_the_change_edited` FAILED (exact byte mismatch), `duplicates_returns_the_index_hit_for_a_duplicated_function` still PASSED (only checks a prefix). Reverted, confirmed clean via diff. Then broke the `run_duplicates` wiring (`index_duplicate_row(file, dup)` → `index_duplicate_row("WRONG_FILE_BROKEN", dup)`): `duplicates_returns_the_index_hit_for_a_duplicated_function` FAILED (`"the row must name the changed side, got: [\"the change edited WRONG_FILE_BROKEN; ...\"]"`), the format-level test still PASSED (calls `index_duplicate_row` directly with correct args, bypassing the loop). Reverted; `git diff` of probes.rs is byte-identical to the pre-experiment diff.
    - Claim 4 (duplication.md prose matches code): found and FIXED a real mismatch. The new "Where the Fix Goes" section said "Each `duplicates` evidence row ... names the side the change edited" as a blanket claim, but the `duplicates` probe also emits **changed-set** rows (`changed_set_duplicates`, untouched by this diff) whose detail reads `"changed-set duplicate of {a.entity_name} in {a.file_path}"` — no "the change edited X" clause, because in the paste-into-two-new-files case (case 3) BOTH halves are new and inside the change, so "one half was there before" is false for that row type. Edited `builtin/validators/duplication/rules/duplication.md` to scope the index-backed guidance to index-backed rows and add a distinct paragraph for changed-set rows: both copies are under review, extract one shared function and update both sites. The example row (`src/existing.rs:41 ...`) itself was confirmed correct — verified byte-for-byte against `index_duplicate_row`'s actual output (mod the same intentional markdown line-wrap as claim 1).
    - Claim 5 (no new `.unwrap()`/`.expect(`/`panic!(`/unnamed numeric literal): verified by grepping every added line in the probes.rs diff — none present. The only new raw numeric literals (`42`, `0.94`) are the definitions of named test constants (`RENDERED_COUNTERPART_LINE`, `RENDERED_COUNTERPART_SIMILARITY`), which is the named form, not a magic number; all other digits appear only inside string-literal expected-output text.
  timestamp: 2026-08-17T12:00:12.311175+00:00
- actor: claude-code
  id: 01m07stmbk4bqwtr1tavgst7wq
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (3c176660b). 9 validators ran, 0 failed, 0 skipped. 0 findings. 1 file reviewed, 4 `.kanban/` files excluded by `.reviewignore`.
    - note: `builtin/validators/duplication/rules/duplication.md` changed in this commit but shows in neither the reviewed count nor the excluded list. No validator matches `.md`, so the rule text got no review.
    - next: card moved to done.
  timestamp: 2026-08-17T12:06:03.379242+00:00
- actor: claude-code
  id: 01m07sv6v3xmv1b1de21zbqdap
  text: |
    ### finish iteration 1 — clean
    - implement: changed — 2 files. Each index-backed row carries a direction clause from one shared `pair_direction` helper. The rule body gained a `## Where the Fix Goes` section. The card's premise that the row carried `detail: None` was wrong, and the correction is on the card.
    - test: green — cargo nextest run --workspace, 14163 passed, 0 failed. fmt and clippy clean. The test step found the new section made a BLANKET claim that is false for a changed-set row, where the change wrote both halves, and split the section in two.
    - commit: 3c176660b
    - review: clean — 9 validators, 0 findings.
    - note: the review engine reviewed 1 of the 6 changed files. The rule prose this card rewrote was neither reviewed nor counted as excluded, because no validator matches a `*.md` file. That gap is not this card's work.
  timestamp: 2026-08-17T12:06:22.307074+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffa780
title: A duplication finding must be fixed on the changed side, not the counterpart
---
Under a diff op the engine enforces where a finding LANDS, but nothing states where its REMEDY may land. For duplication that gap is load-bearing, because a duplication finding is inherently a PAIR: the change on one side, a pre-existing block on the other. A finding correctly landing on a changed line still carries a remedy that points at the unchanged counterpart, and the fix edits a file the change never touched.

## What

Two edits, one concern: state the direction of the fix, in the two places an agent reads it.

**1. `builtin/validators/duplication/rules/duplication.md:71`** currently ends:

> The fix is always the same: extract a shared function and parameterize the difference.

That is direction-neutral. Given a pair, "extract a shared function" reads equally as "edit the pre-existing block". Rewrite it so the changed side is the subject:

- The changed code is what is under review. The remedy lands there.
- Where the counterpart already exists, the fix is to CALL it from the new code, not to rewrite it.
- Where extraction is genuinely needed, extract from the changed code. Touching the counterpart is a separate change and belongs to a separate task.

**2. `crates/swissarmyhammer-validators/src/review/probes.rs:743`** — the index-side `duplicates` rows built inside `run_duplicates` (line 720) carry no `detail`, so the row names a path and a similarity with no statement of which side the change touched. The `ProbeResult.target` IS the changed file and `dup.file_path` IS the counterpart, so the direction is already known and simply not written down.

Give that row a `detail` naming the direction, the way the sibling paths already do — `changed_set_duplicates` (line 769, row at 796) writes `"changed-set duplicate of {} in {}"`, and `clone_sibling_row` (line 970) writes which side the change edited and which is unchanged.

NOTE (measured 2026-08-17): the row no longer carries `detail: None`. It carries `Some(snippet(...))` — the counterpart text alone. The gap the card names stands: the row states no direction.

## Acceptance Criteria

- [x] `duplication.md` states that the remedy lands on the changed code, and that calling an existing counterpart is preferred over rewriting it
- [x] An index-side `duplicates` row carries a `detail` that names which side the change touched, so the row alone tells a reader which file to edit
- [x] The wording distinguishes the two cases — counterpart exists (call it) versus extraction needed (extract from the changed code)
- [x] `cargo nextest run -p swissarmyhammer-validators` passes; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean

## Tests

- [x] In `crates/swissarmyhammer-validators/src/review/probes.rs`, extend the existing `duplicates_returns_the_index_hit_for_a_duplicated_function` (line 1270) to assert the row's `detail` names the changed target and the counterpart, not just the path and similarity
- [x] Add a rendering assertion beside the existing row-format tests near line 1156, pinning the full rendered row text so the direction wording cannot be dropped silently
- [x] Run `cargo nextest run -p swissarmyhammer-validators -E 'test(duplicates)'` — the new assertions fail against the current `detail: None` and pass after

## Workflow

- Use `/tdd` — write the failing assertions first, then implement.

## Why the existing guard does not cover this

`^apb04az` made a diff op review only added and modified lines, and `scope::line_is_reviewed` refutes a finding that lands elsewhere. That guard is about the finding's LOCATION. It cannot see a remedy that names another file in its prose, which is why duplication still produces edits outside the diff.

#tool-validators