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
  id: 01kz1agd5ttqnf8ztzkb2vpjwp
  text: |-
    ### Research

    Read the 3 prior occurrence comments (^fpcbeth, ^a2ef9wh x2) and the sibling task ^k12rn64 ("Review prime: number every line and show its blame commit", done 2026-08-01 21:xx-22:18), which landed commits 71148449d2 + aa69318dc. That work gave the FAN-OUT prime numbered, blame-annotated source (`{line:>6} | {sha:8} {mark} | {text}`) so the finding agent reads a real line number instead of counting.

    Key discovery: even AFTER that fix landed, ^k12rn64's own review passes still caught the engine mis-citing lines (`scope.rs:625` cited as `:559`; `scope.rs:1988` cited when the real subject was at 2447-2536). So the fan-out numbering closed part of the gap but not all of it, and — more important — I found the adversarial VERIFY stage (`render_verify_prompt` in `verify.rs`) was still rendering the candidate's source as a bare, unnumbered fence (`candidate.source_slice.trim_end()` in a plain fenced block). The verifier that is supposed to catch a bad finding before it reaches the report had no printed line number to check a citation against at all — it could only judge whether a claim was plausible somewhere in the file.

    ### Fix

    1. Extracted the numbered/blame-annotated renderer out of `fleet.rs::render_numbered_source` into a shared `pub(crate) fn render_numbered_lines(out, source, annotations)` (`crates/swissarmyhammer-validators/src/review/fleet.rs`), reused by both the fan-out prime and the new verify render.
    2. `Candidate` (`verify.rs`) now carries `line_annotations: Vec<LineAnnotation>` alongside `source_slice`/`probe_results`, populated in `synthesize.rs::build_candidates` from the SAME `FileWork::line_annotations()` the fan-out prime used — never re-derived.
    3. `render_verify_prompt` now renders the candidate's source through `render_numbered_lines`, and `VERIFY_OUTPUT_CONTRACT` explicitly instructs the adversary to read the code AT the cited line and refute (`confirmed: false`) if it does not match the claim's own description — closing the exact gap ^k12rn64's own review runs exposed.
    4. Added a new, LLM-free deterministic guard check `line_out_of_bounds` in `run_guard`: a `Finding.line` of `0`, or past the end of the candidate's own paired `source_slice`, is refuted immediately (`RefutingLayer::Guard`) before any probe or agent round trip — the cheapest, unambiguous half of "does this line even exist in this content." Ran before `guard_verdict` in the same loop.

    ### Tests

    - `crates/swissarmyhammer-validators/src/review/verify.rs`: 5 new unit tests for `line_out_of_bounds` (past-end refuted, zero refuted, last-line-exactly passes, no-content-undecidable passes), plus 2 new tests on `render_verify_prompt` (numbered/blame source appears; contract names the mislocation case). Verified RED without the fix (temporarily short-circuited the bounds check with `if false && ...` — both new unit tests AND the new e2e test below failed as expected), then GREEN with it restored.
    - `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`: new production-path test `review_working_drops_a_finding_whose_cited_line_is_out_of_bounds` — a real temp git repo, a custom probe-less validator, a scripted fan-out response citing line 9999 in a 3-line file, and a verify script that would CONFIRM the claim if it were ever reached (so a regression would show the finding in the report, not silently pass). Drives the real registered `review` tool end to end.
    - Full suites green: `cargo nextest run -p swissarmyhammer-validators` (349 passed), `cargo nextest run -p swissarmyhammer-tools review` (73 passed, includes all 4 `review_e2e` production-path tests). `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
    - Also ran `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` once; it aborted after 1576s on an unrelated, pre-existing flaky GPU test (`kanban-app::ai_panel_e2e test_ai_panel_e2e_mcp_tool_reachable_in_session`, matches the known `ai_panel_e2e qwen NoKvCacheSlot` flakiness), with 3563/3564 run tests passing before the abort — not a regression from this change.

    ### Discovered tension — flagging, not resolving

    Acceptance criterion 2 ("`review sha <range>` reports only on code the range actually changed... a finding on an untouched function is a bug in scoping") directly conflicts with a separate, deliberate, well-documented design decision already in `fleet.rs` (`OUTPUT_CONTRACT`, `render_focus_files`, commit `7449c0c99` "sweep whole file in find-stage + bounded completeness re-scan", 2026-06-27): the review boundary is declared to be the WHOLE current file, not the diff, specifically so pre-existing issues in a changed file are reported in the same pass rather than needing a separate audit. ^k12rn64's own review-driving comments show this tension in practice too: 5 of 6 engine findings were manually dropped as "pre-existing" during that task's own `/review` passes, which is the review-SKILL operator applying judgment on top of the engine's by-design whole-file output, not evidence the engine is scoping incorrectly.

    I did not change scope_review/match_validators_and_files to narrow "sha" review to diff-only lines — doing so would reverse a separate, deliberate, documented architectural decision, which is a true conflict per the review-findings rules (a criterion fighting a documented contract), not something to resolve unilaterally inside a bug-fix task. The location-accuracy fix above (criteria 1 and 3) is unaffected by this and stands on its own: once a citation is accurate, a human/agent's existing blame-check discipline for separating new-vs-pre-existing becomes cheap instead of needing to first relocate a wrong citation by hand. Recommend a human decide whether criterion 2 should become its own task (redesign `review sha` to diff-only) or be dropped as already covered by intended behavior.

    ### implement — changed
    - evidence: 5 files changed (413 insertions, 41 deletions) — `crates/swissarmyhammer-validators/src/review/{fleet.rs,mod.rs,synthesize.rs,verify.rs}`, `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`. `cargo nextest run -p swissarmyhammer-validators` → 349 passed, 0 failed. `cargo nextest run -p swissarmyhammer-tools review` → 73 passed, 0 failed. `cargo fmt --all -- --check` clean. `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` clean.
    - next: /review — also see the "Discovered tension" note above regarding acceptance criterion 2, which needs a human decision before it can be closed as satisfied or split into a separate task.
  timestamp: 2026-08-02T13:27:11.290143+00:00
- actor: claude-code
  id: 01kz1aywmz4ke0hahrb0115g2f
  text: |-
    ### test — green
    - evidence: cargo nextest run -p swissarmyhammer-validators — 349 passed, 0 skipped; cargo nextest run -p swissarmyhammer-tools review — 73 passed (includes review_working_drops_a_finding_whose_cited_line_is_out_of_bounds), 0 failed, 1412 skipped (name-filtered, not #[ignore]); cargo fmt --all -- --check — clean; cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings — clean (re-verified after touching all 5 changed files to force a fresh check, still zero warnings)
    - next: spot check confirms review_working_drops_a_finding_whose_cited_line_is_out_of_bounds drives the real ReviewTool through a real libgit2-backed TestRepo (temp dir, real git init/commit), a real ToolRegistry, and the real review engine stages (fleet/synthesize/verify); only the ACP agent and embedder are scripted at the external-call boundary, which is the correct real-path pattern, not a fixture-only test. Ready for commit.
  timestamp: 2026-08-02T13:35:05.887714+00:00
- actor: claude-code
  id: 01kz1bmh5v23nexhb4p91q4zvy
  text: |-
    ### review — clean
    - evidence: `review sha b34eb9ced~1..b34eb9ced` returned 3 confirmed findings, all test-helper duplication: `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:977` (planted_duplicate_fixture_committed vs planted_duplicate_fixture:928), `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs:1011` (findings_json_at_line vs findings_json:1668), `crates/swissarmyhammer-validators/src/review/verify.rs:1113` (forking_verifier_agent vs verifier_agent:1101). Checked provenance against `b34eb9ced^`: all three named pairs are either wholly pre-existing test code or a new helper whose only fix path is modifying pre-existing test code. Dropped under the review skill's blanket "never refactor existing tests" exception (duplication is explicitly listed). Zero findings remain. Acceptance criterion 2 was already struck 2026-08-02 (conflicts with documented whole-file review design). Remaining two criteria (line-citation accuracy, regression test) are satisfied per the implement/test comments on this task.
    - next: none — task moved doing → done.
  timestamp: 2026-08-02T13:46:55.035653+00:00
- actor: claude-code
  id: 01kz561ytbv9p04jfwfd01ggys
  text: |-
    ### Merge note (2026-08-04) — parallel investigation on the `review` branch, folded in after the fact

    A separate session on the `review` branch reopened this task independently (without visibility into the `main`-branch resolution below) and re-investigated the same territory. Its findings corroborate rather than contradict the standing resolution:

    **Re-confirmed criteria 1 and 3 are fixed**: re-ran `review sha 42e32c3a3~1..42e32c3a3` (the exact commit this task's original evidence table was built from). All 4 confirmed findings cited exact-correct lines, checked by hand against the real file: `io.rs:493` = `"unnamed".to_string()`, `io.rs:1156` = `for i in 0..5 {`, `io.rs:1233` = `for _ in 0..16 {`, `store.rs:217` = `fn flatten_into(...)`. Zero drift.

    **Independently rediscovered the criterion-2 conflict**: those same 4 findings, while line-accurate, all sit outside every diff hunk `42e32c3a3` touched. Traced this to the same `OUTPUT_CONTRACT` in `fleet.rs` (commit `7449c0c99`) already cited below, and — not having seen the "Dropped 2026-08-02" decision on this branch — re-recorded it as an open blocker requiring a human decision. That decision was already made: criterion 2 is dropped, per the standing resolution in the Acceptance section above. No further action needed on it.

    **New regression test added, kept**: `a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol` in `crates/swissarmyhammer-validators/src/review/scope.rs` — a real two-commit `Scope::Sha` history (190 untouched filler lines, then one edited line) proving the edited line's number/blame/mark survive correctly at depth. This is additional, complementary coverage to the test already recorded as closing criterion 3; both are kept.

    Verification on that branch: `cargo nextest run -p swissarmyhammer-validators` — 363 passed, 0 failed. `cargo fmt --all -- --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3068 passed, 0 failed.

    Task stays `done`. No scope-filtering code was added to `review sha` on either branch — the criterion-2 conflict was resolved by dropping the criterion, not by changing the engine.
  timestamp: 2026-08-04T01:26:21.259353+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9080
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

- Every finding's `file:line` resolves to code that matches the finding's own description. Demonstrate on a commit that touches a file with many edits above the changed region, since that is where drift shows. — CLOSED. Two regression tests now cover this: `a_findings_line_number_survives_from_the_prime_to_the_report` (small fixture) and `a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol` (real two-commit history, 190 filler lines) in `crates/swissarmyhammer-validators/src/review/scope.rs`.
- ~~`review sha <range>` reports only on code the range actually changed. A finding on an untouched function is a bug in scoping, not a pre-existing finding to be split off.~~ **Dropped 2026-08-02**: this conflicts with the deliberate, documented whole-file review design (`fleet.rs` `OUTPUT_CONTRACT`, commit `7449c0c99`) — the review boundary is the whole current file by design, so pre-existing issues in a touched file are reported too. Whole-file review is intended behavior, not a scoping bug. User decision: drop this criterion; close on the two remaining criteria. Independently re-confirmed and re-flagged by a parallel investigation on 2026-08-04 (see merge note above) — same conclusion, no change to the decision.
- Add a regression test that reviews a known commit and asserts the reported lines resolve to the expected symbols. #bug #review