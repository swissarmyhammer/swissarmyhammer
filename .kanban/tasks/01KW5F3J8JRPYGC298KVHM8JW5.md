---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw5fbjmnwdpbkt3fp3k068tx
  text: 'Picked up via /finish (single-task). Iteration 1: dispatching /implement. Scope = make the review find-stage sweep the whole inlined file (not the diff) so all instances of a rule report on the first pass. Primary lever = prompt-framing in crates/swissarmyhammer-validators/src/review/fleet.rs (OUTPUT_CONTRACT + render_file_block + render_focus_files); optional lever 2 = bounded 1-pass within-file completeness re-scan. No severity reintroduction.'
  timestamp: 2026-06-27T21:21:48.181479+00:00
- actor: claude-code
  id: 01kw5fp6g8z32z64zk0bygz84d
  text: 'Research done. Confirmed the engine path: render_run_prime → render_file_payload/render_file_block (inlines full source + "What changed (semantic diff)") and render_validator_suffix → render_focus_files ("focusing on these files") + OUTPUT_CONTRACT. Fan-out: submit_fan_out → collect_fan_out → collect_forked_task (warm fork path, has turn.session_id) / collect_task (degraded monolithic, no session id). Plan: Lever 1 reframes the three render fns + OUTPUT_CONTRACT to name the whole current file as the review boundary and demote the semantic diff to orientation-only. Lever 2 = rescan_for_completeness: after a warm fork returns non-empty findings, ONE extra submit_forked(turn.session_id, RESCAN_PROMPT) turn, parse + merge additional findings, capped at exactly 1 pass (no recursion). Degraded monolithic path can''t re-scan (PromptResult carries no session id) so it keeps first-pass behavior — clean boundary. TDD: tests first.'
  timestamp: 2026-06-27T21:27:36.200237+00:00
- actor: claude-code
  id: 01kw5fxwqas2r0j66cnkgzp273
  text: 'Lever 2 blast-radius assessed and DECIDED: implement it. Production change is a single bounded helper (rescan_for_completeness) at one call site (collect_forked_task warm path), no recursion → capped at exactly 1 extra fork. No architecture complication. Test churn is mechanical: 5 existing scripted warm-fork tests script non-empty findings, so the new re-scan fork re-matches their inherited needle and would double findings. Fix per test = prepend a single-needle entry (''## Completeness re-scan'' -> ''[]'') which wins on the re-scan fork''s context (ordered match) and is absent from the first-pass context; bump fork_count asserts in fan_out_two_validators (2->4) and prefix_is_primed_once (1->2). Empty-script tests (forking_agent(vec![])) produce [] so re-scan never fires — unaffected. Degraded/monolithic fallback (ForkFailed) keeps first-pass behavior (no session id to re-fork) — clean boundary.'
  timestamp: 2026-06-27T21:31:48.330604+00:00
- actor: claude-code
  id: 01kw5hp5bqj81dhzkma38znvcm
  text: |-
    DONE (green), left in doing for /review.

    LEVER 1 (framing) implemented in crates/swissarmyhammer-validators/src/review/fleet.rs:
    - render_focus_files: "focusing on these files" -> "Apply the rules below to the WHOLE current contents of each file ... Review every line ... a rule that fires anywhere ... is in scope and must be reported now."
    - render_file_block: full-contents block now states "This whole file is the review boundary ... including pre-existing instances that sit outside the change"; the diff header is now "What changed (semantic diff -- orientation only, NOT the review boundary)" with a paragraph framing it as context, not review scope.
    - OUTPUT_CONTRACT: new "## Review scope" section names the whole current file as the boundary, puts pre-existing instances in scope, and states the diff is orientation only / NOT the review boundary; the every-occurrence paragraph strengthened to "across the WHOLE file, not just the changed lines ... including pre-existing instances". Kept all prior invariants (every occurrence of every rule, do not stop at the first, one finding per file:line, scopes reads to OTHER files, provided in full).

    LEVER 2 (completeness re-scan) IMPLEMENTED (not deferred): new RESCAN_PROMPT const + async rescan_for_completeness(); wired into collect_forked_task warm-fork arm (Ok(Ok(turn))) only -- after a non-empty first pass it issues exactly ONE submit_forked(turn.session_id, RESCAN_PROMPT) turn, parses + tags + merges the additional findings. Capped at exactly 1 extra pass (no recursion/loop). Only ever ADDS: empty first pass skips it; fork-fail/error/unparseable/empty re-scan returns first-pass findings untouched. Degraded ForkFailed->monolithic path keeps first-pass behavior (PromptResult carries no session id) -- clean boundary. Downstream synthesize::dedup_exact still collapses exact repeats. No severity reintroduced (binary pass/fail).

    TESTS (cargo nextest, all green; whole validators crate 290/290 pass; clippy -D warnings clean; fmt applied):
    - NEW fleet::tests::one_rule_with_many_instances_reports_them_all_on_the_first_pass -- N=4 instances of one rule via scripted agent through run_fleet -> all 4 findings on first pass. PASS.
    - NEW fleet::tests::completeness_rescan_fires_once_and_merges_the_missed_instances -- first pass 1 + re-scan 2 = 3 merged; re-scan prompt appears exactly once; fork_count==2 (cap). PASS. Verified RED (1 vs 3) with the re-scan bypassed, then GREEN after restore.
    - NEW fleet::tests::output_contract_names_the_whole_file_as_the_review_boundary_not_the_diff -- asserts "## Review scope", "whole current file", "pre-existing instances", "orientation only", "not the review boundary". PASS.
    - EXTENDED fleet::tests::full_inline_payload_carries_complete_source_and_no_reread_framing -- now also asserts the file-block names the whole-file boundary + diff "orientation only" + "not the review boundary". PASS.
    - EXTENDED synthesize::tests::one_rule_matching_multiple_lines_renders_every_instance -- now N=4 (was 3), data-driven over the line list, asserts all N render + count==N on the first pass. PASS.
    - Updated 5 existing warm-fork tests (fan_out_two_validators, prefix_is_primed_once, degraded_fork_runs_cold, forked_task_with_claude_cache, one_failing_task) to script the now-fired re-scan to [] via a prepended "## Completeness re-scan"->[] entry; bumped fork_count asserts in fan_out_two_validators (2->4) and prefix_is_primed_once (1->2).

    Adversarial double-check: PASS (one non-blocking note: pre-existing intentional dedup-key design means a reworded duplicate re-report would not collapse -- not introduced/worsened here, and RESCAN_PROMPT instructs against it).
  timestamp: 2026-06-27T22:02:32.183693+00:00
- actor: claude-code
  id: 01kw5hq2k76vsxt4sr0bar4r4f
  text: 'Iter 1 implement landed green in doing. Lever 1 (whole-file framing in fleet.rs: OUTPUT_CONTRACT review-scope section, render_file_block boundary note + diff demoted to "orientation only", render_focus_files whole-file language) AND lever 2 (RESCAN_PROMPT + rescan_for_completeness, one extra fork turn on warm-fork arm, capped at 1 pass, additive-only). New tests: one_rule_with_many_instances_reports_them_all_on_the_first_pass, completeness_rescan_fires_once_and_merges_the_missed_instances (verified RED→GREEN), output_contract_names_the_whole_file_as_the_review_boundary_not_the_diff; extended synthesize one_rule_matching_multiple_lines_renders_every_instance to N=4 first-pass. validators crate 290/290, clippy -D warnings clean, fmt applied. Now running /test across blast radius.'
  timestamp: 2026-06-27T22:03:02.119889+00:00
- actor: claude-code
  id: 01kw5scmanb2z8gsw3hmpa3zvt
  text: 'Iter 1 review (review sha HEAD~1..HEAD on 7449c0c9): 3 confirmed / 1 refuted, 32 validators attempted 0 failed. Task → review. Findings: (1) fleet.rs:137 run_fleet unused _config param — remove or document; (2) fleet.rs:830 collect_forked_task ~66 lines, extract handle_fork_success/handle_fork_failed/handle_pool_error/handle_delivery_error; (3) synthesize.rs:153 synthesize takes Vec instead of impl IntoIterator<Item=VerifiedFinding>. All clarity/API on the two files this card already touches. Dispatching iter 2 /implement to fix at root.'
  timestamp: 2026-06-28T00:17:08.437935+00:00
- actor: claude-code
  id: 01kw5swsk3n1n9sg03hqgqd2n3
  text: |-
    Iter 2 /implement: addressed all 3 review findings AT THE ROOT in fleet.rs + synthesize.rs. No severity reintroduced; whole-file framing + completeness re-scan untouched.

    (1) fleet.rs run_fleet unused `_config: FleetConfig` — REMOVED entirely. Investigated: batching is done UPSTREAM in run_review (synthesize.rs) via batch_work_list(work, fleet_config.batch_size) BEFORE run_fleet is called per-batch; run_fleet receives one already-content-budgeted batch and never needed the config. Updated signature to `run_fleet(work, loader, pool)`, added a doc paragraph stating the size policy is applied upstream by run_review. Updated all 9 in-crate call sites (synthesize.rs run_review + 8 fleet.rs tests incl. run_fleet_and_unpin helper). swissarmyhammer-tools review_op.rs is unaffected — it constructs FleetConfig for run_review/run_review_over_agent, not run_fleet. FleetConfig struct kept (still carries batch_size for run_review; test asserts default==DEFAULT_BATCH_SIZE keeps the import live).

    (2) fleet.rs collect_forked_task (~66 lines, nested match arms) — extracted each arm into a focused helper so the fn is now a clean dispatch: handle_fork_success (warm/degraded success: reuse log + parse_task_response + bounded completeness re-scan, identical incl. the warm-fork rescan_for_completeness call), handle_fork_failed (monolithic fresh-session fallback), handle_pool_error (Err(()) zero findings), handle_delivery_error (dropped channel -> Err(())). Behavior byte-identical; each helper carries a doc comment.

    (3) synthesize.rs synthesize — widened `verified: Vec<VerifiedFinding>` to `verified: impl IntoIterator<Item = VerifiedFinding>`; collects to Vec at the top (`let verified = verified.into_iter().collect::<Vec<_>>();`) so the downstream .iter()/.len()/.into_iter() are unchanged. Source-compatible for the existing Vec callers (run_review + all tests), so no call-site changes needed.

    Green: cargo nextest -p swissarmyhammer-validators 290/290 pass; cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings clean (exit 0, no warnings); cargo fmt applied. Left in doing.
  timestamp: 2026-06-28T00:25:58.115139+00:00
- actor: claude-code
  id: 01kw5tw34vyk6tphtq9446q9hv
  text: 'Iter 2 implement+test green (validators+tools 1703 passed, 0 failed, clippy+fmt clean, double-check PASS). Fixed all 3 findings at root: (1) REMOVED unused run_fleet _config param — batching is applied upstream in run_review via batch_work_list before run_fleet runs per batch; updated 9 in-crate call sites; tools review op unaffected (calls run_review, not run_fleet). (2) collect_forked_task split into handle_fork_success/handle_fork_failed/handle_pool_error/handle_delivery_error, re-scan preserved. (3) synthesize widened to impl IntoIterator<Item=VerifiedFinding>, source-compatible. Checkpoint 5feebcd6. Re-reviewing HEAD~1..HEAD.'
  timestamp: 2026-06-28T00:43:03.707379+00:00
- actor: claude-code
  id: 01kw5vf5v1f8x5r7zf38c0e29c
  text: 'Iter 2 review (review sha HEAD~1..HEAD on 5feebcd6): prior 3 findings all REFUTED (code now satisfies them — none re-surfaced). 2 NEW findings, both test-code magic numbers: fleet.rs:492 (probe row line 88) and synthesize.rs:448 (attempted-tasks literal 8). 2 confirmed / 4 refuted, 32 attempted 0 failed. Task stays in review. Iter 3 plan: fix both at root AND sweep the whole of fleet.rs + synthesize.rs test code for sibling unnamed literals (practice the card''s own whole-file rule to avoid a round 4), then flip the 3 prior now-satisfied boxes + the 2 new ones to [x].'
  timestamp: 2026-06-28T00:53:29.057012+00:00
- actor: claude-code
  id: 01kw5w6q76pp01vnf5f0gfdx77
  text: |-
    Iter 3 /implement: fixed both NEW magic-number findings AT THE ROOT + swept the whole test modules of fleet.rs and synthesize.rs.

    fleet.rs test module:
    - NEW `const TEST_PROBE_LINE: u32 = 88;` (doc'd as the immaterial hidden fixture constant for the `duplicates` probe row). Replaced `file_work`'s hardcoded `line: Some(88)` and the paired assert `prompt.contains("src/dup_of_a.rs:88")` (now `format!(... {TEST_PROBE_LINE})`).
    - SWEEP sibling fixture line numbers: the `one_rule_with_many_instances...` and `completeness_rescan...` tests had bare tuple lines 10/22/41/88. Re-expressed as TEST_FINDING_LINE + 0..3 offsets (distinct, immaterial), and the count assert `4` -> `instances.len()` (bound the array). No raw line literals remain; the only `88` left in fleet.rs is the OUTPUT_CONTRACT production-doc example `bar.rs:88` (illustration, not a fixture).

    synthesize.rs test module:
    - NEW `const ATTEMPTED_TASKS: usize = 8;` (doc'd: magnitude immaterial, tests assert the attempted/failed relationship). Replaced the cited `FleetTally::new(8, 0)` (x2: a_fully_successful + an_attempted_clean_run) and the assert `tasks_attempted == 8`.
    - SWEEP sibling: the same-kind `FleetTally::new(60, 60)` all-failed fixture + its `60`/`60`/`"60/60..."` asserts re-expressed as `new(ATTEMPTED_TASKS, ATTEMPTED_TASKS)` and a `format!("{ATTEMPTED_TASKS}/{ATTEMPTED_TASKS} review tasks failed")` so one named concept covers both the success and total-failure fixtures.

    Calibration: left visible role-clear per-call test data (confirmed()/finding() positional `line` args, the documented `let lines = [12u32,34,56,78]` array, classify_reuse CacheUsage token counts, range bounds like (0..10), pool concurrency) — the validator demonstrably did NOT flag those across two prior rounds (it flagged the HIDDEN shared-fixture `Some(88)` and the role-ambiguous `new(8,0)` count). Did not touch the whole-file framing or the completeness re-scan; no severity reintroduced.

    Also flipped checkboxes in the description: the 3 prior 19:06 findings (run_fleet _config removed, collect_forked_task extracted, synthesize generic — all REFUTED/satisfied this round) -> [x], and the 2 new 19:43 findings -> [x]. Section text otherwise verbatim.

    Green: cargo fmt clean; cargo nextest -p swissarmyhammer-validators 290/290 pass; cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings exit 0, no warnings. Left in doing.
  timestamp: 2026-06-28T01:06:20.518932+00:00
position_column: doing
position_ordinal: '8280'
project: local-review
title: Review find-stage must sweep the whole file, not the diff (kill finding-dribble)
---
## Problem

The review engine surfaces same-category findings **one instance per round** ("dribble"): a validator flags one magic number, the agent fixes it, re-review surfaces the *next* one in the same file, and so on. Each round is a full re-review (tokens + wall-clock) and it can bait an agent into force-closing a task that looks like an endless stream of pedantry.

## What was verified in the engine code (this is NOT a wiring bug)

The find-stage prompt already demands exhaustiveness and is injected into **every** per-validator fork:

- `crates/swissarmyhammer-validators/src/review/fleet.rs:887` `OUTPUT_CONTRACT` — "Report every occurrence of every rule that fires, in this single pass... Do not stop at the first match and do not collapse repeated matches into one finding."
- `fleet.rs:842` — `render_validator_suffix` appends `OUTPUT_CONTRACT` to every fork suffix.

Downstream nothing collapses them:
- No finding cap / `take(n)` / truncation in the find→parse path (`fleet.rs` collect, `drive.rs`).
- `parse_task_response` is all-or-nothing: a truncated reply drops the **whole** task (zero findings), it does not salvage "the first object" — so truncation is not the dribble source.
- `synthesize.rs::dedup_exact` keys on `(file, line, rule, suggestion, claim)` — two instances on different lines both survive.
- `verify.rs` runs **per finding** (one confirm/refute object each) — it never merges same-rule findings.

So when the engine reviews a file, it is *instructed and structurally able* to report all instances.

## Root cause (what the engine actually controls)

1. **Prompt framing fights the contract.** Each file block (`fleet.rs::render_file_block`) inlines the full source ("review it directly") but then prominently renders **"What changed (semantic diff)"**, and the validator suffix (`render_focus_files`) says **"focusing on these files."** A small model anchors on the *diff region* and reports the salient instance there, under-reporting pre-existing instances elsewhere in the same file — despite the "every occurrence" line.

2. **`/finish` reviews `HEAD~1..HEAD`.** Only files touched by the last commit are in scope. When the agent fixes the cited line, that edit *is* the next commit → the file re-enters scope → the next review re-examines it and surfaces the next nearby instance. This is a dribble engine by construction **unless** the per-file review sweeps the whole inlined file the first time a file is touched, fixing every instance at once so the file never returns dirty. (The tight per-iteration scope is a deliberate `/finish` design choice — the mitigation is whole-file sweep, not widening scope.)

## Fix

Make the find stage review the **entire inlined file** for every rule, explicitly including lines outside the diff; treat the semantic diff as *context*, not the review boundary. The full file is already in the prompt, so this is primarily a prompt-framing change:

1. **`OUTPUT_CONTRACT` / `render_file_block` framing** — state plainly that the review boundary is the whole current file; the "What changed" section is orientation only; pre-existing instances of a rule in a changed file are in scope and must all be reported now. De-emphasize "focus on the change."
2. **(Reinforcement, optional) bounded within-file completeness re-scan** — after a validator returns findings, one extra fork turn: "You reported these findings. Scan the SAME files again for any further instance of these same rules you missed; reply with the additional findings or `[]`." Loop-until-dry, capped at 1 extra pass to bound tokens. Attacks residual model under-compliance directly.

## Acceptance

- A file containing N instances of one rule, touched by a single commit, yields N findings on the **first** review pass (assert in a fleet/synthesize test with a scripted agent — extend the existing `no-bail-fast` tests at `fleet.rs:1398` / `synthesize.rs:638`).
- The contract/test forbids the model treating the diff as the review boundary (assert the framing names the whole-file scope).
- Real reproduction: a `/finish HEAD~1..HEAD` pass on a file with several magic numbers reports them all in round 1, not one-per-round.

## Notes
- Do NOT reintroduce severity — review stays binary pass/fail.
- Source of truth for skills is `builtin/skills/...`; the skill text ("Fix at the root, not the cited line") is already correct and is the *agent's* fixing behavior — this card fixes the *engine's reporting* behavior so the agent actually receives all instances to fix.

## Review Findings (2026-06-27 19:06)

- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:137` — Unused `_config` parameter creates noise in the API surface. Callers must construct and pass `FleetConfig` even though `run_fleet` never uses it — wasted API friction. Remove the `_config` parameter entirely. If this is reserved for future use, add a `TODO` comment in the docstring instead; don't silently accept unused parameters. If the parameter must stay for API stability, add a comment explaining why.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:830` — The `collect_forked_task` function spans approximately 66 lines (from line 830 to 895), exceeding the 50-line guideline. It handles multiple error cases with nested match arms and logging, making it difficult to follow the primary logic path. Extract error handling into separate helper functions: `handle_fork_success()`, `handle_fork_failed()`, `handle_pool_error()`, and `handle_delivery_error()`. This will make each path's responsibility clearer and improve testability.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:153` — Function `synthesize()` accepts `Vec<VerifiedFinding>` directly instead of a generic iterator, limiting API flexibility. Callers with other iterable sources must convert to Vec; future callers with non-Vec collections incur unnecessary friction. Change signature to `pub fn synthesize(verified: impl IntoIterator<Item = VerifiedFinding>, tally: &FleetTally, now: &str) -> ReviewReport`. Collect to Vec at function start: `let verified = verified.into_iter().collect::<Vec<_>>();` to support the existing `.len()` call on line 156.

## Review Findings (2026-06-27 19:43)

- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:492` — Hardcoded probe row line number 88 should be a named constant — it configures test fixture behavior and appears without explanation. Extract as a named test constant (e.g., `const TEST_PROBE_LINE: u32 = 88;`) and use it consistently across test fixtures.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:448` — Hardcoded numeric literal `8` configures the number of attempted tasks in a test without explanation. It is a test configuration value that should be a named constant. Extract as a named constant (e.g., `const TASK_COUNT: usize = 8;`) and use it in the constructor.