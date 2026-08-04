---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyyv4he3nd16r0pnmencrg7h
  text: |-
    Second occurrence, 2026-08-01, reviewing `3523b4594` for ^a2ef9wh. Stronger evidence this time: `git blame` on every cited line.

    All 10 engine findings cited lines that blame to commits OTHER than the one under review. Not one was in the delta:

    | Cited | Blames to |
    |---|---|
    | `swissarmyhammer-common/src/frontmatter.rs:118`, `:127` | `ddb3c8da1` |
    | `swissarmyhammer-entity/src/io.rs:104` / `:250` / `:278` | `4b8a48703` / `a3db85e01` / `4227e1331` |
    | `swissarmyhammer-tools/src/health_registry.rs:22`, `:46`, `:68`, `:88` | `569279fb5` |
    | `swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:141` | `355650537` |

    Blame is a better test than the line-mismatch check used on the first occurrence: it proves the cited line is not part of the reviewed commit, rather than only showing the description does not match what sits there.

    The line numbers were also wrong in the same way as before. `frontmatter.rs:118` is the doc line `/// use ...::parse_frontmatter;`, while the `metadata: None` construction the finding describes is at 181 and 223. `health_registry.rs:88` is `for dir in dirs_to_check {`, while the `Arc::new(RwLock::new(..))` it describes is at 194-206.

    One finding was also factually wrong on its own terms, independent of the location problem: it claimed `write_entity` panics on a parentless path. It does not — `swissarmyhammer-entity/src/io.rs:100` is `if let Some(parent) = path.parent() {`, a no-op when there is no parent.

    So the failure has two layers worth separating when fixing this:

    1. **Scope** — `review sha <range>` reports on code the range does not touch. Blame makes this cheap to detect, and cheap to assert in a regression test: every finding's cited line must blame to a commit inside the reviewed range.
    2. **Location** — within a reported file, the line number does not point at the described code.

    A useful acceptance test falls straight out of the blame check: review a known commit, then assert that `git blame` for every reported `file:line` resolves to a commit in the reviewed range. That catches both layers without needing to judge whether a finding is substantively correct.

    Cost so far: two consecutive tasks have each needed a manual cross-check to separate real findings from noise. On ^a2ef9wh that was 10 spurious findings against 1 real one.
  timestamp: 2026-08-01T14:20:04.931960+00:00
- actor: claude-code
  id: 01kyyw91btmxr1sjw2w7y9c942
  text: |-
    Third consecutive occurrence, 2026-08-01, reviewing `60a173bf2` for ^a2ef9wh iteration 2. This one carries a proof that needs no blame check.

    **The reviewed commit changed comment lines only — zero executable lines.** Verified by filtering the diff for any added or removed line that is not `//`, `//!`, or `///`; the result is empty. The engine nonetheless reported 5 findings, including:

    - a missing `PartialEq`/`Eq` derive on `Frontmatter`
    - a CRLF handling gap in `parse_frontmatter_internal`
    - a magic literal `3`
    - a hardcoded `"---\n"`

    A comment-only delta cannot introduce a missing derive, a CRLF gap, or a magic number. These are structurally impossible as findings *on this delta*, independent of where their line numbers point. That makes this occurrence stronger evidence than the previous two: no blame comparison is needed to rule them out, only the observation that no code changed.

    Blame agrees anyway — the 5 cited lines blame to `3523b4594`, `d6dd0ada4`, and `ddb3c8da1`, none of them the reviewed commit. And the line numbers are misplaced in the usual way: `:90` is a closing brace while the derive it names is at 106; `:187` is a closing brace while the `starts_with("---\n")` it names is at 203; `:354`/`:355` sit inside a test fixture string with no `"---\n"` literal at either line.

    Two of the five also target `parse_frontmatter_internal`, which ^tv3692e owns and this commit did not touch.

    ## Running cost

    | Task | Engine findings | Actually in scope |
    |---|---|---|
    | ^fpcbeth | 13 | 0 |
    | ^a2ef9wh iter 1 | 10 | 1 |
    | ^a2ef9wh iter 2 | 5 | 0 |

    28 findings, 1 real. Every one of the three reviews needed a manual cross-check to separate signal from noise, and without that check an implementer would have been dispatched to edit code the commits never touched.

    This suggests a cheap, high-value guard independent of the root cause: **when the reviewed delta contains no executable lines, no finding about code structure can be in scope.** Asserting that alone would have caught this occurrence outright.
  timestamp: 2026-08-01T14:40:00.890638+00:00
- actor: claude-code
  id: 01kz561ytbv9p04jfwfd01ggys
  text: |-
    ### implement — changed

    **Empirical finding: the original symptom (line-number drift) is FIXED by ^k12rn64.** Did not assume from the diff — re-ran `review sha 42e32c3a3~1..42e32c3a3` (the exact commit this task's own evidence table is built from). All 4 confirmed findings this run cite exact-correct lines, checked by hand against the real file: `io.rs:493` = `"unnamed".to_string()`, `io.rs:1156` = `for i in 0..5 {`, `io.rs:1233` = `for _ in 0..16 {`, `store.rs:217` = `fn flatten_into(...)`. Zero drift, on the same file class (io.rs/store.rs) the original bug report used.

    **Separated a second, real issue that the original report conflated with the first**: those same 4 findings, while line-accurate, all sit OUTSIDE every diff hunk `42e32c3a3` touched — i.e. all 4 are pre-existing/untouched-code findings. Traced this to `OUTPUT_CONTRACT` in `crates/swissarmyhammer-validators/src/review/fleet.rs`, which explicitly makes the WHOLE file the review boundary and puts pre-existing instances in scope by design (commit `7449c0c99`, predating this task, pinned by tests in `fleet/tests.rs`). This is a true conflict with acceptance criterion 2 ("review sha reports only on code the range actually changed") — recorded as a blocker on the task description, not resolved. No validator edit made.

    **Closed the test gap (criterion 3)**: added `a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol` to `crates/swissarmyhammer-validators/src/review/scope.rs` — a real two-commit `Scope::Sha` history (190 untouched filler lines, then one edited line) proving the edited line's number/blame/mark survive correctly at depth and that a finding citing it round-trips unchanged through parse → verify → synthesize to the final report. Closes the gap the existing small-fixture test (`a_findings_line_number_survives_from_the_prime_to_the_report`, 4-line file) didn't cover at the scale this task specifically asked for ("many edits above the changed region").

    **Verification**:
    - `cargo nextest run -p swissarmyhammer-validators`: 363 passed, 0 failed, 0 skipped.
    - `cargo fmt --all -- --check`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 3068 passed (1 slow), 0 failed, 0 skipped.

    **Files touched**: `crates/swissarmyhammer-validators/src/review/scope.rs` (one new test).

    next: /review — but flag the blocker on criterion 2 for a human decision before treating the task as closeable; reviewing agent should not add scope-filtering code to satisfy it.
  timestamp: 2026-08-04T01:26:21.259353+00:00
position_column: doing
position_ordinal: '8280'
title: Review engine reports findings against a stale revision — cited line numbers do not resolve
---
`review sha HEAD~1..HEAD` returned 13 confirmed findings whose cited line numbers point at unrelated code in the current tree. Observed 2026-08-01 reviewing commit `42e32c3a3` for ^fpcbeth.

## Evidence

Every citation was off by a large, non-uniform offset. Verified by hand:

| Finding cites | What is actually there | Where the described code really is |
|---|---|---|
| `io.rs:309` `<serialization>` | a bare `///` line | io.rs:443 and 460 |
| `io.rs:140` / `334` `.tmp_{Ulid}` | — | io.rs:115 and 543 |
| `io.rs:1321` magic number | — | io.rs:1709 and 1736 |
| `store.rs:109` `EntityTypeStore::deserialize` | a bare `///` line | store.rs:173 |

Cross-checking the commit's own hunk headers confirms the functions the engine named — `write_entity`, `copy_attachment`, `restore_entity_files`, `read_entity_dir`, `reconcile_read_results` — are untouched by `42e32c3a3`. `copy_attachment` appears zero times in the diff.

So the engine was reasoning over a different revision of the file than the one the range names.

## Why it matters

1. **Findings become unactionable.** An agent told to fix `io.rs:309` finds a doc-comment line. The honest responses are to guess or to dismiss, and both are bad.
2. **It invites wrong edits.** An agent that trusts the line number and "fixes" what it finds there damages unrelated code.
3. **It breaks scoping.** `review sha` exists to review one delta. Reporting on untouched functions defeats the purpose and forces a manual hunk-header cross-check on every run to tell in-scope from out-of-scope.
4. **It interacts badly with the finish loop.** The loop treats any open finding as blocking. Findings that cannot be located cannot be closed honestly.

Related but distinct: ^k5wsxh0 (same validator returns different finding sets across runs on an unchanged file). That one is nondeterminism; this one is a stale or mismatched revision.

## Investigate

- Whether the file content handed to validators comes from the working tree, the index, or the named revision — and whether it matches the line numbers the finding reports.
- Whether the batching that inlines file bytes (`batch_size`) offsets line numbers when a file is split or truncated.
- Whether `review sha` resolves the range to the pre-image or post-image, and whether findings are numbered against the other one.

## Acceptance

- [x] Every finding's `file:line` resolves to code that matches the finding's own description. Demonstrate on a commit that touches a file with many edits above the changed region, since that is where drift shows. — VERIFIED FIXED by ^k12rn64 (numbered/blamed prime). Empirical proof: re-ran `review sha` on this task's own cited commit `42e32c3a3` (the `io.rs`/`store.rs` commit the evidence table above is built from). All 4 confirmed findings this time cite exact-correct lines — checked by hand against the real file at that commit (`io.rs:493` = `"unnamed".to_string()`, `io.rs:1156` = `for i in 0..5 {`, `io.rs:1233` = `for _ in 0..16 {`, `store.rs:217` = `fn flatten_into(...)`). Zero drift, on the same file class the original report used.
- [ ] `review sha <range>` reports only on code the range actually changed. A finding on an untouched function is a bug in scoping, not a pre-existing finding to be split off. — TRUE CONFLICT, NOT RESOLVED. See blocker note below. Do not resolve by editing the validator contract; a human must decide.
- [x] Add a regression test that reviews a known commit and asserts the reported lines resolve to the expected symbols. — CLOSED. New test `a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol` in `crates/swissarmyhammer-validators/src/review/scope.rs`, using a real two-commit `Scope::Sha` history with 190 untouched lines above the edited line. #bug #review

## Blocker: true conflict on the scoping criterion (2026-08-04)

The second acceptance criterion fights a documented, tested, shipped contract. Not resolved. Recorded per the true-conflict process — no validator edit made, no scope filter added.

**The contract**: `OUTPUT_CONTRACT` in `crates/swissarmyhammer-validators/src/review/fleet.rs` states outright: "The review boundary is the WHOLE current file, not the changed lines... Pre-existing instances of a rule... are in scope and must be reported now." This is not incidental wording — it is the deliberate result of commit `7449c0c99` ("feat(review): sweep whole file in find-stage + bounded completeness re-scan"), dated before this task was filed, and it is pinned by dedicated tests in `fleet/tests.rs` that assert on this exact wording ("the contract must put pre-existing instances in scope", "the contract must state the diff is NOT the review boundary").

**Empirical confirmation the conflict is real, not just textual**: re-running `review sha 42e32c3a3~1..42e32c3a3` today (see the closed criterion above) returned 4 confirmed findings. Checking their line numbers against the commit's actual diff hunks: `io.rs:493`, `io.rs:1156`, `io.rs:1233`, and `store.rs:217` all sit OUTSIDE every hunk this commit touched. Every citation is numerically exact (criterion 1 is fixed) AND every one is on pre-existing code the commit did not touch (criterion 2, as stated, is not met) — because the engine is doing exactly what the contract tells it to do.

**Why this is a true conflict, not a bug to fix**: this task's own history shows the SAME finish loop already treats whole-file, pre-existing findings as expected and handles them by hand-checking blame per finding, every round (see task `^k12rn64`'s own review comments: 5 of 6, then 1 of 1, findings dropped as pre-existing on each pass, by design). Adding a scope filter now (e.g. dropping any finding on an untouched line) would directly contradict a shipped, tested feature and would need a person to decide whether `review sha` should stop doing whole-file sweeps, or whether this criterion should be dropped/reworded. Not decided here.

**Next**: a human decides whether to (a) drop/reword this acceptance criterion to match the shipped whole-file-sweep design, or (b) change the whole-file-sweep design (touches `OUTPUT_CONTRACT` and its pinned tests, a real product decision with a real cost to the completeness the sweep was built for).