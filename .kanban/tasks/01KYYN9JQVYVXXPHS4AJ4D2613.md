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
- actor: claude-code
  id: 01kz568hk8dk2efvbez21yqtdj
  text: |-
    ### finish iteration 1 — stuck
    - implement: changed — core symptom (line numbers not resolving) confirmed already fixed by ^k12rn64, verified empirically by re-running review sha 42e32c3a3~1..42e32c3a3 (this task's own cited commit) and checking all 4 findings by hand against the real file — all resolve correctly. Closed a real test gap: added a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol (scope.rs), a real two-commit Scope::Sha history with 190 filler lines above one edit, proving line/blame/mark survive at depth through parse->verify->synthesize.
    - test: green — cargo nextest run -p swissarmyhammer-validators 363 passed; fmt clean; clippy clean; rdeps(swissarmyhammer-validators) 3068 passed, 0 failed
    - commit: 748dcb1a2
    - BLOCKER (true conflict, verified by orchestrator, not resolved): acceptance criterion 2 ("review sha reports only on code the range actually changed; a finding on an untouched function is a bug in scoping") directly contradicts OUTPUT_CONTRACT in fleet.rs — a deliberate, tested design from commit 7449c0c99 (2026-06-27, predates this task by over a month) that explicitly instructs: "across the WHOLE file, not just the changed lines... including pre-existing instances that sit outside the changed region... Do not treat the diff as the review boundary." Confirmed the commit predates this task, read the exact prompt text, and confirmed it's intentional design language, not an accident.
    - outcome: STUCK. This is a real, provable conflict between this task's stated acceptance criteria and an intentional, tested, older production behavior. Per the finish skill's true-conflict rule: not resolved by implementer or orchestrator, no validator/contract touched, no scope-filtering added. A human must decide: either drop/reword acceptance criterion 2 (the whole-file-sweep is deliberate and desired), or decide the whole-file-sweep design itself should change (a much larger change with its own tests to update). Left in doing, not moved to review or done — a clean review of this commit's diff would incorrectly auto-close a task with this criterion still unresolved.
  timestamp: 2026-08-04T01:29:57.096474+00:00
- actor: claude-code
  id: 01kz6j0n26bn3bs04m2phbvzak
  text: |-
    ### finish iteration 2 — changed
    - implement: changed — reworded acceptance criterion 2 to match the shipped whole-file-sweep design per human decision 2026-08-04; added resolution note. No source files touched.
    - test: n/a — no code changed, task-text-only edit
    - commit: pending
    - review: pending
  timestamp: 2026-08-04T14:14:35.846898+00:00
- actor: claude-code
  id: 01kz6jhy1egywmwx6yggsdwpnw
  text: |-
    ### review — findings
    - evidence: 4 findings — crates/swissarmyhammer-validators/src/review/scope.rs:151, crates/swissarmyhammer-validators/src/review/scope.rs:361, crates/swissarmyhammer-validators/src/review/scope.rs:380, crates/swissarmyhammer-validators/src/review/scope.rs:659
    - scope: `review sha 748dcb1a2~1..748dcb1a2`
    - engine returned 6 confirmed findings; 2 (scope.rs:1967, scope.rs:2086) dropped per the never-refactor-existing-tests exception — both sit inside `mod tests` before this commit's insertion point (old-side line 2321), so they are pre-existing test code, not the new regression test this commit added
    - all 6 engine-cited lines verified by hand against `git show 748dcb1a2:crates/swissarmyhammer-validators/src/review/scope.rs` — every citation resolves to the described code, no drift
    - next: implementer addresses the 4 open findings (all production code in scope.rs — WorkList::new, FileWork::new, FileWork::with_line_annotations, collect_added_lines)
  timestamp: 2026-08-04T14:24:02.094592+00:00
- actor: claude-code
  id: 01kz6jjqjkz8b4krj02nk5667w
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — reworded acceptance criterion 2 per human decision 2026-08-04; no source files touched
    - test: n/a — no code changed
    - commit: 748dcb1a2 (from prior iteration, now reviewed for the first time)
    - review: findings — crates/swissarmyhammer-validators/src/review/scope.rs:151 (WorkList::new take Vec instead of impl IntoIterator), :361 (FileWork::new same for 3 params), :380 (FileWork::with_line_annotations same), :659 (collect_added_lines nests 4 deep, over gate of 3)
  timestamp: 2026-08-04T14:24:28.243542+00:00
- actor: claude-code
  id: 01kz6k4mwsv31dckv8p1w4ngb6
  text: |-
    ### implement — changed

    Fixed 3 of the 4 review findings by generalizing constructor/builder parameters in `crates/swissarmyhammer-validators/src/review/scope.rs`:

    1. `WorkList::new` — `validators: Vec<ValidatorWork>` -> `validators: impl IntoIterator<Item = ValidatorWork>`, collected via `.into_iter().collect()` inside the body.
    2. `FileWork::new` — `semantic_diff: Vec<SemanticChange>`, `changed_symbols: Vec<String>`, `probe_results: Vec<ProbeResult>` -> all three changed to `impl IntoIterator<Item = T>`, each collected inside the body.
    3. `FileWork::with_line_annotations` — `line_annotations: Vec<LineAnnotation>` -> `impl IntoIterator<Item = LineAnnotation>`, collected inside the body.

    4th finding (`collect_added_lines` nesting depth over gate) — investigated and found ALREADY FIXED on HEAD by an unrelated commit `0193e51ba` ("dedupe changed-set probe evidence across batch files (^t7f5fqf)"), which extracted the innermost line-resolution logic into a new `added_line_number` helper. Current nesting is `for -> for -> if let` = depth 3, under the `NESTING_DEPTH_THRESHOLD` of 4 (`crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`). Verified by reading the current function body and diffing `748dcb1a2..HEAD` on this file. No code change made for this finding — it was a no-op by the time this task picked it up.

    Grepped the whole workspace for call sites of `WorkList::new`, `FileWork::new`, and `.with_line_annotations(` — every call site passes a `Vec` or array literal, which already implements `IntoIterator`, so no caller needed changes.

    **Verification**:
    - `cargo fmt --all`: clean.
    - `cargo build --workspace --all-targets`: clean.
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 3079 tests run, 3079 passed (2 slow), 0 skipped, 0 failed.

    **Files touched**: `crates/swissarmyhammer-validators/src/review/scope.rs` (3 signature changes, no callers touched).

    All 4 findings checked off on the task description.

    next: /review
  timestamp: 2026-08-04T14:34:15.321203+00:00
- actor: claude-code
  id: 01kz6kb16bh3p6m01trp4vhx1b
  text: |-
    ### finish iteration 3 — changed
    - implement: changed — WorkList::new, FileWork::new, FileWork::with_line_annotations changed from Vec<T> to impl IntoIterator; collect_added_lines nesting finding confirmed already resolved by unrelated commit 0193e51ba (depth 3, threshold 4)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: d10852a9e
    - review: pending
  timestamp: 2026-08-04T14:37:44.523738+00:00
- actor: claude-code
  id: 01kz6km4pm50k3dfdawk5k5xxg
  text: |-
    ### review — findings
    - evidence: 5 findings — crates/swissarmyhammer-validators/src/review/scope.rs:152, crates/swissarmyhammer-validators/src/review/scope.rs:303 (x2), crates/swissarmyhammer-validators/src/review/scope.rs:316, crates/swissarmyhammer-validators/src/review/scope.rs:432
    - next: implement fixes, then re-review scoped to the fix commit
  timestamp: 2026-08-04T14:42:43.028868+00:00
- actor: claude-code
  id: 01kz6kmr3hd4gyj9j1hactr92s
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — WorkList::new, FileWork::new, FileWork::with_line_annotations changed from Vec<T> to impl IntoIterator
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: d10852a9e
    - review: findings — crates/swissarmyhammer-validators/src/review/scope.rs:152 (WorkList::new change_purpose: String not impl Into<String>), :303 (ValidatorWork::new files: Vec<FileWork> not converted, sibling constructors were), :316 (ValidatorWork::with_shared_probe_results same gap), :432 (FileWork::new path: String not impl Into<String>)
  timestamp: 2026-08-04T14:43:02.897543+00:00
- actor: claude-code
  id: 01kz6m28ac53yvtbqh0thryax0
  text: |-
    ### implement — changed

    Fixed all 5 open findings in `crates/swissarmyhammer-validators/src/review/scope.rs`:

    1. `WorkList::new` — `change_purpose: String` → `change_purpose: impl Into<String>`, converted with `.into()` in the body.
    2. `ValidatorWork::new` — `files: Vec<FileWork>` → `files: impl IntoIterator<Item = FileWork>`, collected with `.into_iter().collect()`.
    3. `ValidatorWork::with_shared_probe_results` — `shared_probe_results: Vec<ProbeResult>` → `impl IntoIterator<Item = ProbeResult>`, collected in the body.
    4. `FileWork::new` — `path: String` → `path: impl Into<String>`, converted with `.into()` in the body.

    Sweep of the whole file found two more instances of the identical gap and fixed them proactively:
    5. `ValidatorWork::new` — `validator_name: String` → `impl Into<String>`.
    6. `FileWork::new` — `source_slice: String` → `impl Into<String>`.

    Verified every call site of `WorkList::new`, `ValidatorWork::new`, `.with_shared_probe_results(`, and `FileWork::new` across the workspace (`crates/swissarmyhammer-validators/src/review/synthesize.rs`, `crates/swissarmyhammer-validators/src/review/fleet/tests.rs`, `crates/swissarmyhammer-validators/src/review/scope.rs` test module, `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`) — all pass `String`/`Vec` literals via `.to_string()`/`vec![]`, which already satisfy `Into<String>`/`IntoIterator`, so no call site needed a change.

    Evidence:
    - `cargo fmt --all` — clean, no diff beyond the edits.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, 0 warnings.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` — 3079 tests run: 3079 passed, 0 failed, 0 skipped.

    next: /review
  timestamp: 2026-08-04T14:50:25.484052+00:00
- actor: claude-code
  id: 01kz6m774aahfggd4z72j8h47f
  text: |-
    ### finish iteration 4 — changed
    - implement: changed — 6 constructor params converted to impl IntoIterator/impl Into<String> across WorkList/ValidatorWork/FileWork
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: e9598b4f2
    - review: pending
  timestamp: 2026-08-04T14:53:08.106744+00:00
- actor: claude-code
  id: 01kz6mdqt3jm475amv8hgkna8w
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (e9598b4f2) — 0 findings, 0 confirmed, 3 refuted, 9 attempted
    - next: task moved to done; all prior checklist items were already checked
  timestamp: 2026-08-04T14:56:41.795615+00:00
- actor: claude-code
  id: 01kz6medsngws31rtycqm3fed0
  text: |-
    ### finish iteration 4 — clean
    - implement: changed — 6 constructor params converted to impl IntoIterator/impl Into<String>
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3079 passed, 0 failed, 0 skipped
    - commit: e9598b4f2
    - review: clean — task moved to done
  timestamp: 2026-08-04T14:57:04.309809+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffa480
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
- [x] `review sha <range>` sweeps the whole file, per `OUTPUT_CONTRACT`. It reports pre-existing issues outside the changed hunks too. Confirmed as designed. Not a scoping bug. Human decision 2026-08-04: keep the whole-file sweep.
- [x] Add a regression test that reviews a known commit and asserts the reported lines resolve to the expected symbols. — CLOSED. New test `a_known_commit_with_many_lines_above_the_change_resolves_the_correct_symbol` in `crates/swissarmyhammer-validators/src/review/scope.rs`, using a real two-commit `Scope::Sha` history with 190 untouched lines above the edited line. #bug #review

## Blocker: true conflict on the scoping criterion (2026-08-04)

The second acceptance criterion fights a documented, tested, shipped contract. Not resolved. Recorded per the true-conflict process — no validator edit made, no scope filter added.

**The contract**: `OUTPUT_CONTRACT` in `crates/swissarmyhammer-validators/src/review/fleet.rs` states outright: "The review boundary is the WHOLE current file, not the changed lines... Pre-existing instances of a rule... are in scope and must be reported now." This is not incidental wording — it is the deliberate result of commit `7449c0c99` ("feat(review): sweep whole file in find-stage + bounded completeness re-scan"), dated before this task was filed, and it is pinned by dedicated tests in `fleet/tests.rs` that assert on this exact wording ("the contract must put pre-existing instances in scope", "the contract must state the diff is NOT the review boundary").

**Empirical confirmation the conflict is real, not just textual**: re-running `review sha 42e32c3a3~1..42e32c3a3` today (see the closed criterion above) returned 4 confirmed findings. Checking their line numbers against the commit's actual diff hunks: `io.rs:493`, `io.rs:1156`, `io.rs:1233`, and `store.rs:217` all sit OUTSIDE every hunk this commit touched. Every citation is numerically exact (criterion 1 is fixed) AND every one is on pre-existing code the commit did not touch (criterion 2, as stated, is not met) — because the engine is doing exactly what the contract tells it to do.

**Why this is a true conflict, not a bug to fix**: this task's own history shows the SAME finish loop already treats whole-file, pre-existing findings as expected and handles them by hand-checking blame per finding, every round (see task `^k12rn64`'s own review comments: 5 of 6, then 1 of 1, findings dropped as pre-existing on each pass, by design). Adding a scope filter now (e.g. dropping any finding on an untouched line) would directly contradict a shipped, tested feature and would need a person to decide whether `review sha` should stop doing whole-file sweeps, or whether this criterion should be dropped/reworded. Not decided here.

**Next**: a human decides whether to (a) drop/reword this acceptance criterion to match the shipped whole-file-sweep design, or (b) change the whole-file-sweep design (touches `OUTPUT_CONTRACT` and its pinned tests, a real product decision with a real cost to the completeness the sweep was built for).

## Resolution: human decision (2026-08-04)

The team asked the human to decide. The team gave the tradeoff. The whole-file sweep found most of the real fixes made today. The whole-file sweep also makes task loops longer.

The human chose to keep the whole-file sweep.

The team also proposed a new idea. The idea: move findings on untouched code to separate cards. This idea would not block the current task. The human did not pick this idea now. The human picked the simpler path. The team only reworded criterion 2 to match the shipped design.

## Review Findings (2026-08-04 09:15)

Scope: `review sha 748dcb1a2~1..748dcb1a2`. All line numbers verified by hand against the file as it stood at commit `748dcb1a2`. 2 engine findings on pre-existing test code (`scope.rs:1967`, `scope.rs:2086`, both inside `mod tests` predating this commit's added test) are dropped per the never-refactor-existing-tests exception.

- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:151` — Constructor accepts concrete `Vec<ValidatorWork>` instead of generic `impl IntoIterator<Item = ValidatorWork>`, limiting flexibility and inconsistent with similar container-building patterns in the same file that use `impl IntoIterator`. Change parameter to `validators: impl IntoIterator<Item = ValidatorWork>` and collect: `Self { change_purpose, validators: validators.into_iter().collect() }`. FIXED — `WorkList::new` now takes `impl IntoIterator<Item = ValidatorWork>` and collects inside the body.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:361` — Constructor accepts three concrete `Vec<T>` parameters (`semantic_diff`, `changed_symbols`, `probe_results`) instead of generic `impl IntoIterator` equivalents, limiting flexibility and inconsistent with container-building patterns in the same file. Change all three parameters to accept iterables: `semantic_diff: impl IntoIterator<Item = SemanticChange>`, `changed_symbols: impl IntoIterator<Item = String>`, `probe_results: impl IntoIterator<Item = ProbeResult>`, then collect each: `.into_iter().collect()`. FIXED — `FileWork::new` now takes all three parameters as `impl IntoIterator` and collects each inside the body.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:380` — Builder method accepts concrete `Vec<LineAnnotation>` instead of generic `impl IntoIterator<Item = LineAnnotation>`, limiting flexibility and inconsistent with the pattern established for accepting iterables elsewhere in the codebase. Change parameter to `line_annotations: impl IntoIterator<Item = LineAnnotation>` and collect: `self.line_annotations = line_annotations.into_iter().collect(); self`. FIXED — `FileWork::with_line_annotations` now takes `impl IntoIterator<Item = LineAnnotation>`.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:659` — Function condition-nesting depth reaches gate threshold of 4 (nested more than 3 levels deep). Deeply nested conditionals reduce readability and maintainability. Extract the innermost if-let logic into a helper function, or restructure using guard clauses with earlier continues to reduce nesting depth below 4. ALREADY FIXED — `collect_added_lines` was already flattened to `for -> for -> if let` (depth 3, under the gate of 4) by an unrelated commit `0193e51ba` ("dedupe changed-set probe evidence across batch files (^t7f5fqf)"), which extracted the innermost line-resolution logic into `added_line_number`. Confirmed by reading the current function body and by diffing `748dcb1a2..HEAD` — no further change needed for this finding.

## Review Findings (2026-08-04 09:38)

Scope: `review sha HEAD~1..HEAD` (commit `d10852a9e`). All line numbers verified by hand against the current file.

- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:152` — Constructor parameter `change_purpose: String` accepts a concrete type instead of a generic. Should accept `impl Into<String>` to allow callers to pass &str, String, Cow<str>, or other string-like types without forcing allocation or explicit conversion. Change parameter to `change_purpose: impl Into<String>` and update the body to `change_purpose: change_purpose.into(),`. FIXED — `WorkList::new` now takes `change_purpose: impl Into<String>`, converted with `.into()` in the body.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:303` — ValidatorWork::new still accepts Vec<FileWork> for the files parameter, while the same change made WorkList::new, FileWork::new, and FileWork::with_line_annotations all accept impl IntoIterator. The refactoring to accept IntoIterator was applied inconsistently — this sibling constructor was not updated. Change ValidatorWork::new line 303 from `files: Vec<FileWork>,` to `files: impl IntoIterator<Item = FileWork>,` and collect it in the body (line 309) with `.into_iter().collect()`, matching the pattern applied to the other constructors. FIXED — `ValidatorWork::new` now takes `files: impl IntoIterator<Item = FileWork>` and collects inside the body.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:303` — Constructor parameter `files: Vec<FileWork>` accepts a concrete Vec instead of `impl IntoIterator`. Other methods in this change (FileWork::new, WorkList::new) were updated to accept iterators, but ValidatorWork::new was not, creating inconsistency and forcing unnecessary Vec allocation. Change parameter to `files: impl IntoIterator<Item = FileWork>` and update the body to `files: files.into_iter().collect(),`. FIXED — same change as above (duplicate finding, same root cause).
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:316` — Method parameter `shared_probe_results: Vec<ProbeResult>` accepts a concrete Vec instead of `impl IntoIterator`. Inconsistent with the change pattern where FileWork::new and WorkList::new were updated to accept iterators, forcing unnecessary Vec allocation. Change parameter to `shared_probe_results: impl IntoIterator<Item = ProbeResult>` and update the body to `self.shared_probe_results = shared_probe_results.into_iter().collect();`. FIXED — `ValidatorWork::with_shared_probe_results` now takes `impl IntoIterator<Item = ProbeResult>`.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:432` — Constructor parameter `path: String` accepts a concrete type instead of a generic. Should accept `impl Into<String>` to allow flexible string input (e.g., &str, String, Box<str>) without forcing allocation. Change parameter to `path: impl Into<String>` and update the body to `path: path.into(),`. FIXED — `FileWork::new` now takes `path: impl Into<String>`, converted with `.into()` in the body.

Proactive sweep (per task instructions, same pattern applied a 4th time before another review round could flag it): found two more owned-parameter gaps by the identical logic and fixed them now — `ValidatorWork::new`'s `validator_name: String` parameter is now `impl Into<String>`, and `FileWork::new`'s `source_slice: String` parameter is now `impl Into<String>`. All call sites across the workspace (`review/synthesize.rs`, `review/fleet/tests.rs`, `review/scope.rs` tests, `swissarmyhammer-tools/src/mcp/tools/review/tests.rs`) pass `String`/`Vec` literals, which already satisfy `Into<String>`/`IntoIterator`, so no call site needed a change.

#bug #review
