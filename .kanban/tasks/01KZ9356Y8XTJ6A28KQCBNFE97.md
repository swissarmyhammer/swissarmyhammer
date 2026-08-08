---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzbvbw1jc1pmxpe4qdd9fkvk
  text: |-
    Research notes (explore step):

    - Contract read: `builtin/validators/README.md`. No conflict with the card found.
    - Reuse points confirmed: `parse_tool_stdout` (review/tool_output.rs) parses both stdout shapes. `doctor.rs` has `check_tool_rule` (presence + version + fixtures) and `ToolRuleStatus::usable()` — the "tool is healthy" decision. `run_shell` in doctor.rs is the one shell runner (`sh -c <script> sh <args>`); the card says bash, so the shared runner moves to bash for doctor and engine together (one runner, no drift).
    - Pipeline shape: `run_review` (synthesize.rs) = scope_review -> batch_work_list -> per batch run_fleet + verify_findings -> synthesize. Fleet grain is one task per validator (RuleSet); `render_validator_suffix` renders ALL ruleset.rules bodies into the LLM prompt.
    - Plan:
      1. New module `review/tool_rules.rs`: `plan_tool_rules(work, loader, project_types)` -> runs (healthy tool rules + matched files), fallbacks (unhealthy -> prompt rule stays), suppression map (validator -> file -> superseded prompt rules). `execute_tool_runs(runs, repo_root, progress)` runs each script with bash at the repo root (`files` scope: files as `$@`; `workspace` scope: no args, keep findings in matched files), parses stdout with `parse_tool_stdout`, tags validator/rule, wraps as CONFIRMED `VerifiedFinding` (no adversarial verify). Nonzero exit -> `ToolRunError` with raw stderr, no findings.
      2. Fleet: `run_fleet` gets a `&ToolSuppression` param. `plan_fan_out` groups a validator's files by suppressed-rule set and builds owned filtered (ValidatorWork, RuleSet) tasks; a group with zero prompt rules submits no LLM task. `render_validator_suffix` filters out tool rules so no LLM ever reads a tool rule.
      3. `run_review` plans + executes tool rules once per run (before batches), extends verified findings, passes suppression to each batch.
      4. `synthesize` gets a `&ToolReport` (errors + fallbacks): renders a tool-error block (raw stderr) and a fallback note; adds `tool_errors` count.
    - Call sites to update: ~20 `run_fleet` calls in fleet/tests.rs, ~25 `synthesize` calls in tests + 1 in swissarmyhammer-tools review_op.rs.
    - Test level for acceptance: `run_review_over_agent` + ScriptedAgent + TestRepo in drive.rs tests (real pipeline, scripted ACP agent). `write_tool_rule_ruleset` already exists in test_support.
  timestamp: 2026-08-06T15:34:12.786251+00:00
- actor: claude-code
  id: 01kzbwayq8rpfhmpvcyyr8sw9d
  text: |-
    ### implement — changed
    - evidence: 9 files — NEW crates/swissarmyhammer-validators/src/review/tool_rules.rs (plan_tool_rules + execute_tool_runs + ToolSuppression/ToolRun/ToolFallback/ToolRunError/ToolPlan/ToolOutcome/ToolReport, 12 unit tests); review/mod.rs (module + re-exports); review/fleet.rs (run_fleet takes &ToolSuppression, plan_fan_out groups files per suppressed-rule set and builds owned filtered tasks, a fully covered group submits no LLM task, render_validator_suffix never renders a tool rule body); review/synthesize.rs (run_review plans + executes tool rules once per run before batching, tool findings join verified as CONFIRMED and skip verify, synthesize takes &ToolReport and renders tool-error blocks with raw stderr plus fallback notes, ReviewCounts::tool_errors, "Nothing in scope" guarded by ToolReport::is_inert, 3 new tests); review/drive.rs (3 real-pipeline acceptance tests over ScriptedAgent + Scope::File: healthy tool → tool finding with ZERO LLM prompts; tool missing → prompt rule runs and the report notes "prompt rule 'missing-docs' ran instead"; nonzero exit → tool error with raw stderr, not clean, not findings); doctor.rs (run_shell moves sh→bash per the card and is now the ONE shared runner, check_tool_rule/command_failure_detail pub(crate), ToolRuleStatus::degraded_detail); review/fleet/tests.rs + review/scope.rs + swissarmyhammer-tools review_op.rs (call sites).
    - commands: cargo nextest run -p swissarmyhammer-validators → 433/433 pass; cargo nextest run -p swissarmyhammer-tools -E 'test(review)' → 81/81 pass; cargo check --workspace --all-targets clean; cargo clippy -D warnings clean on both touched crates; cargo fmt --check clean. Doc warnings observed under cargo doc are pre-existing (untouched lines in drive.rs/fleet.rs/synthesize.rs module docs), none in the new code.
    - notes: health = doctor's ToolRuleStatus::usable() (presence + fixtures), checked once per tool rule per run. Workspace-scope runs execute once per run at the repo root before batching and keep only findings in the rule's matched files. supersedes is per-file: the fleet groups a validator's files by suppressed-rule set. The engine has no tool-config surface — a rule's run script writes any temp config itself (module docs state this).
    - next: test
  timestamp: 2026-08-06T15:51:11.336195+00:00
- actor: claude-code
  id: 01kzbx3n2q9pefp1d4y9dbat40
  text: |-
    Check-in from the watching session (2026-08-06):

    Verified 716951039 against the contract — all acceptance points land: zero-LLM tool findings (real-pipeline test), supersedes per file, prompt fallback with a report note, nonzero exit = tool error with raw stderr, one shared bash runner with doctor.

    Your script-owns-its-config decision is better than the contract sentence it deviated from (that sentence predated the shell-pipe design). The contract now follows you: README updated — "the run script writes config to a temp path and passes it with a flag; never the project's own lint config." No action needed on this card.
  timestamp: 2026-08-06T16:04:40.663052+00:00
- actor: claude-code
  id: 01kzbz5kfpetjfm95wk9dgr58v
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (checkpoint 716951039); engine counts: 27 findings, 27 confirmed, 8 refuted, 4/60 tasks failed, 4 files over the prompt cap. 14 findings dropped per the review skill test-refactor exception (subject: test code that existed before this commit). 13 findings recorded — review_op.rs:1, review_op.rs:1064, doctor.rs:166, drive.rs:1383, fleet.rs:1, fleet.rs:62, fleet.rs:1263, fleet.rs:1343, fleet.rs:1470, scope.rs:1, synthesize.rs:257, synthesize.rs:544, tool_rules.rs:325
    - next: implement each open item in the 2026-08-06 10:57 section, then run /review ^cbnfe97 again
  timestamp: 2026-08-06T16:40:41.718109+00:00
- actor: claude-code
  id: 01kzbz6cxjb2d2z638sea73n2r
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 9 files (validators: review/tool_rules.rs new, fleet.rs, fleet/tests.rs, synthesize.rs, mod.rs, scope.rs, drive.rs, doctor.rs; tools: review/review_op.rs)
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators)', 3171 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 716951039
    - review: findings — 13 open: review_op.rs:1,1064; doctor.rs:166; drive.rs:1383; fleet.rs:1,62,1263,1343,1470; scope.rs:1; synthesize.rs:257,544; tool_rules.rs:325 (prompt-cap splits, doc drift, complexity gates, degradation-message divergence, fixture-helper dup, missing PartialEq/Eq)
  timestamp: 2026-08-06T16:41:07.762212+00:00
- actor: claude-code
  id: 01kzc0yevbh98nj0zk4hgtksag
  text: |-
    Work log for the 13 review findings (2026-08-06 10:57). All 13 are done and checked.

    Splits (over-cap findings):
    - scope.rs (177KB → 49KB): NEW scope/resolve.rs (git resolution, path containment, FileChangeBuilder), NEW scope/batch.rs (SkippedFile + batch_work_list + project_onto_files), NEW scope/tests.rs + scope/tests_matching.rs (the old inline test module, split in two). Public paths do not change — scope.rs re-exports batch items; resolve items stay module-internal (pub(super) + glob import).
    - fleet.rs (82KB → 52KB): NEW fleet/render.rs (all prompt rendering + framing/measure helpers), NEW fleet/prime.rs (prime/confirm/pin/unpin + PrefixReuse/classify_reuse). The shared prompt constants (PRIME_HANDOFF, VALIDATOR_HEADER, MANDATE_HEADER, OUTPUT_CONTRACT) stay in fleet.rs because test_support and the child test modules read them there; render.rs imports them from super.
    - review_op.rs (95KB → 17KB): NEW review_op/backend.rs (AgentHandle, factories, embedder cache, pool policy), review_op/progress.rs (progress + content-log bridge), review_op/response.rs (ReviewResponse/ReviewCountsView), review_op/tests.rs (the old inline tests). External paths (review_op::AgentFactory etc.) are preserved by re-exports.

    Point fixes:
    - review_op response: ReviewResponse now derives PartialEq + Eq.
    - doctor.rs: tool_rule_check now builds each degraded message as degraded_detail() + fallback_note(), so doctor and the engine can never describe the same degradation two ways. degraded_fix() picks the fixture-pair fix for MissingFixtures, install commands otherwise. One test assertion moved from "not used" to the fallback note + a starts_with(degraded_detail()) check.
    - drive.rs: write_tool_fixtures deleted. The shared helper is test_support::write_tool_rule_fixtures(base, rule); tool_rules.rs tests call it too (their local write_fixtures is deleted).
    - synthesize.rs: synthesize() extracts render_tool_errors + render_tool_fallbacks; run_review doc now numbers 5 stages with tool-rule planning/execution as stage 2, and the body comments match.
    - tool_rules.rs: plan_tool_rules now delegates to matched_rule_files (per-rule file matching) and plan_rule_by_health (health check + runs/fallbacks/suppression), flattening the nesting.
    - fleet doc drift: module doc and the docs of render_fleet_prompt, render_validator_suffix, prompt_framing_bytes, run-fleet/submit_fan_out now all say "prompt rules (tool rules are executed separately by the tool runner)" — every "full ruleset" / "every rule body" claim in fleet.rs is corrected, not only the cited lines.

    Verification: cargo nextest run -E 'rdeps(swissarmyhammer-validators)' → 3171/3171 pass; -p swissarmyhammer-validators → 433/433; tools review tests 81/81; cargo fmt --check clean; cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -D warnings clean; cargo check --workspace --all-targets clean.
  timestamp: 2026-08-06T17:11:44.747499+00:00
- actor: claude-code
  id: 01kzc0yqta6vw7xpe8w15s0vhq
  text: |-
    ### implement — changed
    - evidence: 13/13 findings worked and checked. 15 files — split: swissarmyhammer-validators review/scope.rs (+ NEW scope/resolve.rs, scope/batch.rs, scope/tests.rs, scope/tests_matching.rs), review/fleet.rs (+ NEW fleet/render.rs, fleet/prime.rs), swissarmyhammer-tools review/review_op.rs (+ NEW review_op/backend.rs, review_op/progress.rs, review_op/response.rs, review_op/tests.rs); fixed: doctor.rs (degraded message unification), synthesize.rs (helpers + 5-stage doc), tool_rules.rs (planner helpers), drive.rs + test_support.rs (shared write_tool_rule_fixtures), review_op response (PartialEq/Eq), fleet/tests.rs (ForkAttachment import). Tests: nextest rdeps(swissarmyhammer-validators) 3171/3171 pass; fmt clean; clippy -D warnings clean.
    - next: test
  timestamp: 2026-08-06T17:11:53.930069+00:00
- actor: claude-code
  id: 01kzc4rwmcg5958smn5zzefehv
  text: |-
    ### review — findings
    - evidence: 15 findings on checkpoint 503b74346 (HEAD~1..HEAD) — review_op.rs:1, review_op/backend.rs:26, doctor.rs:1, drive.rs:1, fleet.rs:1, fleet/prime.rs:124, scope.rs:1, scope/batch.rs:120, scope/resolve.rs:1, scope/resolve.rs:28, scope/resolve.rs:593, scope/resolve.rs:601, scope/resolve.rs:607, synthesize.rs:1, tool_rules.rs:1. All 13 prior findings are checked. 5 engine findings were dropped per the review skill's exception for test code that existed before the commit. Engine warnings: 6/67 review tasks failed; 13 files were over the prompt cap.
    - next: the implementer must correct the 15 open findings. The task stays in review.
  timestamp: 2026-08-06T18:18:36.556333+00:00
- actor: claude-code
  id: 01kzc4swy6rryj6rdt8zjpwv6g
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — worked all 13 findings from 2026-08-06 10:57 checklist; split scope.rs/fleet.rs/review_op.rs into submodules; 15 files
    - test: green — cargo nextest run, 3171 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 503b74346
    - review: findings — 15 open: review_op.rs:1, review_op/backend.rs:26, doctor.rs:1, drive.rs:1, fleet.rs:1, fleet/prime.rs:124, scope.rs:1, scope/batch.rs:120, scope/resolve.rs:1,28,593,601,607, synthesize.rs:1, tool_rules.rs:1. Note: batch budget shrank 164176 → 54338 bytes between passes; 8 of 15 are further over-cap split requests, incl. files cited in iteration 1 (scope.rs, fleet.rs, review_op.rs)
  timestamp: 2026-08-06T18:19:09.638202+00:00
- actor: claude-code
  id: 01kzc5bry42125fq54sr9s3add
  text: |-
    Progress on the 2026-08-06 12:15 findings. Point fixes are complete:

    - scope/batch.rs: batch_work_list is now generic (`F: Fn(&FileWork) -> usize`). The call sites pass `&fn` and compile with no change.
    - fleet/prime.rs: the three functions (submit_prime, confirm_saved_state, pin_prefix) now log the run name with one field: `run = %name`. Before, two functions used `validator` and one used `scope`.
    - review_op/backend.rs: a compile-time assertion module for AgentHandle is added. IMPORTANT: the finding asks for a Send+Sync assertion, but AgentHandle cannot be Sync. The compiler shows the cause: `agent_client_protocol::ErasedConnectTo` (upstream crate, version 0.12.1) has only the `Send` supertrait, so `DynConnectTo<Client>` is `!Sync`. The rust trait rule requires "all applicable traits" — Sync is not applicable to this type. The fix asserts `Send` at compile time and documents why Sync is structurally unavailable. This is a deviation from the finding's literal code, forced by the upstream type contract.

    The eight module-split findings are in work now (scope.rs, scope/resolve.rs, fleet.rs, synthesize.rs, drive.rs, tool_rules.rs, doctor.rs, review_op.rs). Target: each production file lands well under the 54KB prompt budget.
  timestamp: 2026-08-06T18:28:55.364218+00:00
- actor: claude-code
  id: 01kzc5q95bb47qzx9vwqk6qm62
  text: |-
    STOP before splitting again — from the watching session (2026-08-06):

    The 8 over-cap findings in the 12:15 section are not caused by your code. The batch budget CHANGED between rounds: 164176 bytes in round 1, 54338 bytes in round 2. That is why fleet.rs and scope.rs are "over the cap" again right after you split them, and why untouched doctor.rs is suddenly over. Splitting adds files to the diff, a bigger diff shrinks the budget, a smaller budget flags more files. This loop cannot converge. The budget instability is now its own card: ^tsram0q.

    Recommendation for this card:
    - Fix the 7 point findings — they are real: backend.rs:26 (Send+Sync assertion), prime.rs:124 (log field naming), batch.rs:120 (impl Fn generic), resolve.rs:28/593/601 (derives), resolve.rs:607 (doc comment).
    - Do NOT act on the 8 over-cap split findings (review_op.rs:1, doctor.rs:1, drive.rs:1, fleet.rs:1, scope.rs:1, resolve.rs:1, synthesize.rs:1, tool_rules.rs:1). Check them off with a note pointing at ^tsram0q — the remedy is fixing the budget computation, not splitting converged modules a second time.
    - Then run /review again. If round 3 still flags over-cap files, that is ^tsram0q evidence, not new work for this card.
  timestamp: 2026-08-06T18:35:12.427285+00:00
- actor: claude-code
  id: 01kzc6h36bcfvc151nw8ctrq01
  text: |-
    Work log for the 2026-08-06 12:15 findings, after the coordinator stop order (18:35 comment):

    The 8 over-cap split findings are NOT actioned. Cause: the review batch budget changed between rounds (164176 → 54338 bytes), so splits cannot converge. Card ^tsram0q tracks the budget instability. All split work from this session was reverted — scope.rs, fleet.rs, fleet/tests.rs, synthesize.rs, drive.rs, tool_rules.rs, doctor.rs, and review_op.rs are byte-equal to HEAD again, and all new split files were deleted. Each of the 8 items is checked in the description with the note "not actioned — prompt-cap budget instability, see ^tsram0q".

    The 7 point findings are fixed:
    - backend.rs:26 — compile-time assertion module for AgentHandle. DEVIATION, verified with the compiler: AgentHandle cannot be Sync because the upstream `agent_client_protocol::ErasedConnectTo` trait (v0.12.1) has only the `Send` supertrait, so `DynConnectTo<Client>` is `!Sync`. The assertion covers `Send` (the load-bearing bound — the handle moves across tasks) and the module doc records why Sync is not applicable. The rust trait rule requires "all applicable traits"; Sync is not applicable here.
    - prime.rs:124 — all three functions (submit_prime, confirm_saved_state, pin_prefix) now log the run with one uniform field: `run = %name` (the finding's second option; the value is the run, not a validator).
    - batch.rs:120 — batch_work_list is generic: `<F: Fn(&FileWork) -> usize>`. The 10 call sites (synthesize.rs, fleet/tests.rs, scope/tests.rs) drop the now-needless `&` (clippy needless_borrows_for_generic_args under -D warnings).
    - resolve.rs:28, :593, :601 — ResolvedScope, FileVersions, and FileChangeBuilder all derive `Debug, Clone, PartialEq, Eq`. To make the full derive set possible, `swissarmyhammer_sem::git_types::FileChange` (a workspace type held in two of the structs) now derives `PartialEq, Eq` — its fields are String, FileStatus, and Option<String>, all Eq-able. The other types in resolve.rs (GitRefSpec, FilePath, BeforeContent, AfterContent) already carried full derives.
    - resolve.rs:607 — FileChangeBuilder::new() has a doc comment; no other public item in the file is undocumented.

    Verification: cargo fmt --check clean; cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools -p swissarmyhammer-sem --all-targets -- -D warnings clean; cargo nextest run -E 'rdeps(swissarmyhammer-validators)' → 3173/3173 pass, 0 skipped.
  timestamp: 2026-08-06T18:49:18.283109+00:00
- actor: claude-code
  id: 01kzc6haks5mec2z1ze2qmjfc4
  text: |-
    ### implement — changed
    - evidence: 8 files — crates/swissarmyhammer-tools/src/mcp/tools/review/review_op/backend.rs (Send assertion + !Sync cause documented), crates/swissarmyhammer-validators/src/review/fleet/prime.rs (uniform `run = %name`), crates/swissarmyhammer-validators/src/review/scope/batch.rs (generic `F: Fn(&FileWork) -> usize`), crates/swissarmyhammer-validators/src/review/scope/resolve.rs (derives + doc), crates/swissarmyhammer-sem/src/git_types.rs (FileChange + PartialEq/Eq), plus call-site `&` removals in synthesize.rs, fleet/tests.rs, scope/tests.rs. 15/15 findings checked: 7 point findings fixed; 8 over-cap split findings not actioned — prompt-cap budget instability, see ^tsram0q — and split work reverted to HEAD. fmt clean; clippy -D warnings clean; nextest rdeps(swissarmyhammer-validators) 3173/3173 pass.
    - next: test
  timestamp: 2026-08-06T18:49:25.881524+00:00
- actor: claude-code
  id: 01kzc77qxt0s7cgnqwzpvbsggw
  text: |-
    ### review — findings
    - evidence: 3 findings — crates/swissarmyhammer-sem/src/git_types.rs:3, crates/swissarmyhammer-sem/src/git_types.rs:4, crates/swissarmyhammer-sem/src/git_types.rs:33 (scope: HEAD~1..HEAD, checkpoint 1c6407b3b; engine counts: 4 findings, 4 confirmed, 13 refuted; 1 finding dropped per the review skill's test-code exception — scope/tests.rs:155 targets test lines that predate this commit; 0 over-cap flags, no new ^tsram0q evidence)
    - next: resolve the 3 open findings in git_types.rs, then re-run /review ^cbnfe97
  timestamp: 2026-08-06T19:01:40.410162+00:00
- actor: claude-code
  id: 01kzc78j443bc4pmme3af4afx7
  text: |-
    ### finish iteration 3 — findings
    - implement: changed — 7 point findings fixed; 8 over-cap findings checked as not actioned per the watching session's direction (budget instability → ^tsram0q); second-round split work reverted; 8 files
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-validators) | rdeps(swissarmyhammer-sem)', 3470 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 1c6407b3b
    - review: findings — 3 open, all crates/swissarmyhammer-sem/src/git_types.rs: :3 (DiffScope PartialEq/Eq), :4 (DiffScope doc comment), :33 (CommitInfo PartialEq/Eq). No fresh over-cap flags this pass (skipped_files empty).
  timestamp: 2026-08-06T19:02:07.236237+00:00
- actor: claude-code
  id: 01kzc7fr0aew95z2d8fvwkywq1
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-sem/src/git_types.rs. All 3 findings from 2026-08-06 13:53 worked and checked: DiffScope gets `PartialEq, Eq` (:3), DiffScope gets a doc comment (:4), CommitInfo gets `PartialEq, Eq` (:33). Full-file sweep per the rules: module doc added; FileStatus, FileChange, and all variants and fields get doc comments; every comparable public type now derives PartialEq + Eq (FileStatus and FileChange already had them). Verification: cargo nextest run -p swissarmyhammer-sem 297/297 pass; cargo clippy -p swissarmyhammer-sem --all-targets -- -D warnings clean; cargo check --workspace --all-targets clean; cargo fmt --check clean.
    - next: test
  timestamp: 2026-08-06T19:06:02.634387+00:00
- actor: claude-code
  id: 01kzc86qx39jjps9e91sn86xy7
  text: |-
    ### review — findings
    - evidence: 4 findings — crates/swissarmyhammer-sem/src/git_types.rs:8, git_types.rs:8, git_types.rs:29, git_types.rs:65 (scope: review sha HEAD~1..HEAD, checkpoint 7b5cf81a4; counts: 4 confirmed, 3 refuted, 0 failed; skipped_files empty — no new ^tsram0q evidence)
    - next: implement the 4 open findings in git_types.rs, then re-review
  timestamp: 2026-08-06T19:18:36.195225+00:00
- actor: claude-code
  id: 01kzc87gvcvpnyv1xq3kqcrj62
  text: |-
    ### finish iteration 4 — findings
    - implement: changed — worked the 3 git_types.rs findings (DiffScope derives + doc, CommitInfo derives) with full-file sweep; 1 file
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3470 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 7b5cf81a4
    - review: findings — 4 open, all crates/swissarmyhammer-sem/src/git_types.rs: :8 ×2 (DiffScope Serialize/Deserialize), :29 (FileStatus Hash), :65 (CommitInfo serde rename_all camelCase). No over-cap flags (skipped_files empty).
  timestamp: 2026-08-06T19:19:01.740730+00:00
- actor: claude-code
  id: 01kzc8n7wvsbqw9hrb83d6109h
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-sem/src/git_types.rs. All 4 findings from 2026-08-06 14:10 worked and checked. Before the camelCase change, a workspace grep confirmed NO consumer serializes the sem CommitInfo — no wire contract or persisted format depends on the current casing, so no conflict. Changes (TDD, RED confirmed with 11 compile errors first): DiffScope gets `Hash, Serialize, Deserialize` plus `#[serde(rename_all = "lowercase")]` to match the module's enum convention (FileStatus); FileStatus gets `Hash` (:29); FileChange gets `Hash` (full-file sweep — every Eq type now carries Hash); CommitInfo gets `Hash` and `#[serde(rename_all = "camelCase")]` (:65) to match FileChange. New test module pins the contract: DiffScope serde round trip, lowercase variant wire form, FileStatus in a HashSet, Hash on all four types, CommitInfo `shortSha` on the wire with round trip. Verification: cargo nextest run -p swissarmyhammer-sem git_types 5/5 pass; cargo nextest run -E 'rdeps(swissarmyhammer-sem)' 3475/3475 pass; cargo fmt --check clean; cargo clippy -p swissarmyhammer-sem --all-targets -- -D warnings clean.
    - next: test
  timestamp: 2026-08-06T19:26:31.323853+00:00
- actor: claude-code
  id: 01kzc989dtb6zaqk968g353f82
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (checkpoint 5734aa85f) — 2 findings: crates/swissarmyhammer-sem/src/git_types.rs:1, crates/swissarmyhammer-sem/src/git_types.rs:32
    - 0 over-cap flags this pass — `skipped_files` was empty; no new evidence for ^tsram0q
    - next: correct the 2 open findings in the 2026-08-06 14:29 section, then run /review again
  timestamp: 2026-08-06T19:36:55.482158+00:00
- actor: claude-code
  id: 01kzc9906ja2vz8cx56x7rsbt2
  text: |-
    ### finish iteration 5 — findings
    - implement: changed — worked the 4 git_types.rs findings (DiffScope Serialize/Deserialize + lowercase, Hash sweep, CommitInfo camelCase) with pinning tests; 1 file
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3475 passed, 0 failed; fmt clean; clippy -D warnings clean
    - commit: 5734aa85f
    - review: findings — 2 open, both crates/swissarmyhammer-sem/src/git_types.rs: :1 (missing # Examples in crate-level docs), :32 (missing Display on FileStatus). No over-cap flags (skipped_files empty).
  timestamp: 2026-08-06T19:37:18.802011+00:00
- actor: claude-code
  id: 01kzc9q87s3wrm4h7yr67r177g
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-sem/src/git_types.rs. The 2 findings from 2026-08-06 14:29 are done and checked. Finding :1 — the module docs now have an `# Examples` section with a doctest that constructs `DiffScope::Working`, `DiffScope::Commit`, a `FileStatus` variant, and a `FileChange` instance; the example uses no `.unwrap()`. Finding :32 — `FileStatus` now implements `Display` with lowercase variant names (`added`, `modified`, `deleted`, `renamed`); a test pins that the Display form equals the serde wire form. Full-file sweep so no item stays for a next round: `DiffScope` also implements `Display` (git revision syntax: `working`, `staged`, the sha, `from..to`); all four public types now derive `PartialOrd, Ord`; `FileStatus` also derives `Copy` (a workspace grep found no `.clone()` call on a `FileStatus` value, so no clippy `clone_on_copy` risk). `Default`, `From`, and `AsRef` are not applicable — these types have no natural default value or canonical conversion. TDD: RED first (14 compile errors for the missing Display/Ord/Copy), then GREEN. Verification: cargo nextest run -p swissarmyhammer-sem git_types 10/10 pass; cargo test --doc -p swissarmyhammer-sem git_types 1/1 pass; cargo nextest run -E 'rdeps(swissarmyhammer-sem)' 3480/3480 pass; cargo clippy -p swissarmyhammer-sem -p swissarmyhammer-validators -p swissarmyhammer-tools -p swissarmyhammer-git --all-targets -- -D warnings clean; cargo fmt --check clean.
    - next: test
  timestamp: 2026-08-06T19:45:05.785687+00:00
- actor: claude-code
  id: 01kzca4jtbtcf5z42v0amvhqnw
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (checkpoint e9f50259c) — 0 findings, 9 attempted, 0 failed, skipped_files empty (no new over-cap flags for ^tsram0q); all prior findings checked
    - next: none — task moved to done
  timestamp: 2026-08-06T19:52:22.603259+00:00
- actor: claude-code
  id: 01kzca593t19gxs9dx1bbgqyn2
  text: |-
    ### finish iteration 6 — clean
    - implement: changed — worked the 2 git_types.rs findings (# Examples doctest, Display on FileStatus) plus sweep (Display on DiffScope, PartialOrd/Ord, Copy); 1 file
    - test: green — cargo nextest run -E 'rdeps(swissarmyhammer-sem)', 3480 passed, 0 failed; 1 doctest passed; fmt clean; clippy -D warnings clean
    - commit: e9f50259c
    - review: clean — 0 findings, 0 over-cap flags; task moved to done
  timestamp: 2026-08-06T19:52:45.434039+00:00
depends_on:
- 01KZ934SNEJ1TXNS2G9Q4909TF
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffb280
title: Tool-runner execution path in the review engine
---
Run tool rules in the review engine. No LLM reads the code for a tool rule.

The contract is `builtin/validators/README.md`.

Work:
- Pair matching stays the same — the existing `ValidatorMatch` path. When a matched rule has a `tool` block and the tool is healthy, run the `run` script instead of an LLM reviewer.
- Execute `run` with the shell (bash). `scope: files` — pass the changed files as script arguments (`"$@"`). `scope: workspace` — run once at the workspace root with no arguments, then keep only findings in changed files.
- Read findings from stdout, one per line: `path:line: message` or a JSON object `{file, line, message}`. Empty stdout = clean. Parsing these two line shapes is the ONLY parsing the engine does — no format/jq/regex config; the rule's pipe already did the mapping.
- Exit 0 = the script judged the code. Nonzero exit = tool error, not findings — raw stderr goes to the diagnosing agent, and no findings are read.
- `supersedes`: when a healthy tool rule matches a file, skip the named prompt rule for that file. When the tool is missing or unhealthy, run the named prompt rule as today.
- Stream findings on the existing channels. Skip adversarial verification for tool findings.
- When a tool needs a config file, write it to a temporary path and pass it with a flag. Never change the project's lint config.

Acceptance:
- A real-pipeline test: run `review file` on a fixture with the tool present, and see tool findings with zero LLM validator calls for that pair.
- A supersedes test: tool present skips the prompt rule; tool absent runs the prompt rule and the report notes the fallback.
- A nonzero-exit test: the run is reported as a tool error, not as clean and not as findings.

#tool-validators

## Review Findings (2026-08-06 10:57)

> Scope: checkpoint 716951039 (HEAD~1..HEAD at review time).
> ⚠️ 4/60 review tasks failed — results are INCOMPLETE.
> ⚠️ 4 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` — 226459 rendered bytes, over the 164176-byte batch budget; not reviewed by: duplication (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/fleet.rs` — 170047 rendered bytes, over the 164176-byte batch budget; not reviewed by: duplication (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/fleet/tests.rs` — 231233 rendered bytes, over the 164176-byte batch budget; not reviewed by: duplication (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/scope.rs` — 416572 rendered bytes, over the 164176-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
>
> Note: 14 engine findings were dropped per the review skill's written exception — each had, as its subject, a change to test code that existed before this commit (`review_op.rs:1469/1644/1847`; `fleet/tests.rs:1/85/141/141/159/165/439/476/488/1679/1799`).

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1` — This file exceeds the review prompt cap — 226459 rendered bytes against the 164176-byte batch budget — so these validators could not review it: duplication. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1064` — Public struct ReviewResponse should implement PartialEq and Eq. It contains only fields (String and ReviewCountsView) that support these traits, and downstream crates cannot add these traits later due to orphan rules. Add PartialEq and Eq to the derive macro: `#[derive(Debug, Clone, PartialEq, Eq, Serialize)]` on line 1064.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:166` — The new `degraded_detail()` method formats degradation reasons with inconsistent punctuation and missing suffixes compared to the established format in `tool_rule_check()`. The same rule status should produce the same degradation description everywhere. The comment at line 250-253 states this method exists so the review engine 'reuses the same health decision doctor reports — "healthy" can never mean two different things' — but the formats diverge: `degraded_detail()` produces 'tool missing: X' (colon) while `tool_rule_check()` line 494 produces 'tool missing (X)' (parentheses); `degraded_detail()` produces 'fixtures failed: X' while line 507 produces 'fixtures failed: X; the tool rule is not used' (missing explanatory suffix). Ensure the degradation message format is consistent between `degraded_detail()` and `tool_rule_check()`. Either refactor `tool_rule_check()` to build its message using `degraded_detail()` plus the fallback note, or update `degraded_detail()` to include the full context (e.g., append `fallback_note()` when appropriate) so both code paths produce identical output for the same status.
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:1383` — Function write_tool_fixtures reinvents code that exists in tool_rules.rs. Per the probe result showing 0.98 similarity to tool_rules.rs::tests::write_fixtures (line 549), this function duplicates nearly identical fixture-writing logic that already exists in the same crate and should be imported and called instead. Import write_fixtures from the tool_rules test module (or from test_support if moved there) and call it with appropriate parameters instead of duplicating the fixture-writing logic.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:1` — This file exceeds the review prompt cap — 170047 rendered bytes against the 164176-byte batch budget — so these validators could not review it: duplication. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:62` — Module-level documentation at line 64 describes render_validator_suffix as emitting 'every rule body verbatim', but the implementation filters out tool rules. Documentation does not reflect that tool rules are excluded from rendering. Update the module documentation to clarify: 'every prompt rule body verbatim (tool rules are executed separately by the tool runner)' to match the actual behavior.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:1263` — Function doc comment at line 1269 states it renders 'mandate + every rule body + output contract', but since this function calls render_validator_suffix which filters out tool rules, the rendered output does not include tool rules. Documentation is inaccurate. Update the function doc comment to clarify: 'mandate + every prompt rule body (excluding tool rules which are executed separately) + output contract' to match the actual behavior.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:1343` — Function doc comment states it renders 'every one of the validator's rule bodies', but implementation at line 1366 filters out tool rules. Documentation does not reflect that tool rules are excluded from the rendered output. Update the function doc comment to clarify: 'Render the per-validator suffix a forked session is prompted with: the validator header, mandate, the files this validator must focus on, every prompt rule body (excluding tool rules which are executed separately), and the output contract.'.
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:1470` — Function doc comment states the suffix carries 'every rule body' (line 1481), but the implementation at line 1510 calls render_validator_suffix which filters out tool rules. Documentation does not reflect that tool rules are excluded from the framing byte calculation. Update the function doc comment to clarify: 'every prompt rule body (excluding tool rules)' or 'every rule body that gets sent to the LLM' to match the actual behavior.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:1` — This file exceeds the review prompt cap — 416572 rendered bytes against the 164176-byte batch budget — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:257` — The `synthesize` function has cognitive complexity 15, meeting the gate of 15 or more. Complex functions are harder to understand, test, and modify, increasing maintenance burden. Extract tool-error rendering (lines 330–343) and tool-fallback rendering (lines 345–359) into separate helper functions. This clarifies the pipeline and brings complexity under the gate.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:544` — The `run_review` function documentation describes a 4-stage pipeline but omits the tool-rule execution stage that occurs between `scope_review` (stage 1) and `batch_work_list` (stage 2) in the actual implementation. The numbered stages in the doc comment do not match the code execution order, making the documentation incorrect for readers trying to understand the pipeline. Update the `run_review` function's doc comment to insert a new numbered stage 2 describing tool-rule planning and execution, and renumber the subsequent stages accordingly (batch_work_list becomes stage 3, fleet becomes stage 4, synthesize becomes stage 5).
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:325` — Function exceeds cognitive complexity gate: 19 measured vs. gate 15. The function has nested loops (validator → rule → file), multiple early-return guards, deeply-nested filtering logic, and branching on tool rule health status — all combining to make the control flow difficult to follow. Extract the tool-rule planning logic into smaller helpers: one to match a single rule against files, one to check health and build runs/fallbacks. This refactoring flattens the nesting and pushes detail into named functions.

## Review Findings (2026-08-06 12:15)

> Scope: checkpoint 503b74346 (HEAD~1..HEAD at review time).
> ⚠️ 6/67 review tasks failed — results are INCOMPLETE.
> ⚠️ 13 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` — 120346 rendered bytes, over the 54338-byte batch budget; not reviewed by: duplication, reuse (narrow the scope)
> - `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op/tests.rs` — 114624 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/doctor.rs` — 99338 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/drive.rs` — 156931 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/fleet.rs` — 127833 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/fleet/tests.rs` — 158703 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/scope.rs` — 227165 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/scope/resolve.rs` — 83595 rendered bytes, over the 54338-byte batch budget; not reviewed by: reuse (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/scope/tests.rs` — 118827 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/scope/tests_matching.rs` — 123243 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/synthesize.rs` — 157073 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/test_support.rs` — 169765 rendered bytes, over the 54338-byte batch budget; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (narrow the scope)
> - `crates/swissarmyhammer-validators/src/review/tool_rules.rs` — 100176 rendered bytes, over the 54338-byte batch budget; not reviewed by: duplication, reuse (narrow the scope)
>
> Note: 5 engine findings were dropped per the review skill's written exception — each had, as its subject, a change to test code that existed before this commit (`review_op/tests.rs:1`, `fleet/tests.rs:1`, `scope/tests.rs:1`, `scope/tests_matching.rs:1`, `test_support.rs:1` — all are test modules whose code the split moved, or pre-existing test fixtures).

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs:1` — This file exceeds the review prompt cap — 120346 rendered bytes against the 54338-byte batch budget — so these validators could not review it: duplication, reuse. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op/backend.rs:26` — Public struct AgentHandle contains reference types (DynConnectTo trait object, broadcast::Receiver) and lacks compile-time Send+Sync assertions; downstream consumers cannot add these bounds due to orphan rules. Add a compile-time assertion after the Debug impl block (line 74):
```rust
#[cfg(test)]
mod send_sync_assertions {
    use super::*;
    const _: () = {
        const fn assert_send_sync<T: Send + Sync>() {}
        const fn check() {
            assert_send_sync::<AgentHandle>();
        }
    };
}
```.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:1` — This file exceeds the review prompt cap — 99338 rendered bytes against the 54338-byte batch budget — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs:1` — This file exceeds the review prompt cap — 156931 rendered bytes against the 54338-byte batch budget — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-validators/src/review/fleet.rs:1` — This file exceeds the review prompt cap — 127833 rendered bytes against the 54338-byte batch budget — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-validators/src/review/fleet/prime.rs:124` — The `name` parameter (representing the run being primed) is logged with inconsistent field names across functions that operate on the same entity: `validator = %name` in submit_prime (lines 55, 63) and confirm_saved_state (lines 85, 95), but `scope = %name` in pin_prefix (lines 124, 135). Same parameter, same semantic meaning (the run), inconsistent field naming makes log queries and analysis fragile. Use `validator = %name` consistently across all three functions (submit_prime, confirm_saved_state, pin_prefix), or choose a more semantically accurate field name such as `run = %name` and apply it uniformly.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs:1` — This file exceeds the review prompt cap — 227165 rendered bytes against the 54338-byte batch budget — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-validators/src/review/scope/batch.rs:120` — Parameter uses trait object &dyn Fn instead of generic impl Fn, making the API less flexible and forcing callers to construct trait object references. Change to generic parameter: `pub fn batch_work_list<F: Fn(&FileWork) -> usize>(work: &WorkList, budget: usize, cost: F)`. Allows callers to pass function pointers, closures, or any Fn-implementing type without requiring reference construction.
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:1` — This file exceeds the review prompt cap — 83595 rendered bytes against the 54338-byte batch budget — so these validators could not review it: reuse. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:28` — New public type ResolvedScope has non-empty representation but lacks Debug (explicitly required for all public types with non-empty representation) and other applicable traits (Clone, PartialEq, Eq). Add `#[derive(Debug, Clone, PartialEq, Eq)]` above the struct definition.
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:593` — New public type FileVersions has non-empty representation but lacks Debug and other applicable traits (Clone, PartialEq, Eq). Add `#[derive(Debug, Clone, PartialEq, Eq)]` above the struct definition.
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:601` — New public type FileChangeBuilder has non-empty representation but lacks Debug and other applicable traits (Clone, PartialEq, Eq). Add `#[derive(Debug, Clone, PartialEq, Eq)]` above the struct definition.
- [x] `crates/swissarmyhammer-validators/src/review/scope/resolve.rs:607` — Public function `FileChangeBuilder::new()` lacks documentation. Every other public method in the file is documented; this inconsistency violates Rust documentation conventions for public items. Add a doc comment such as `/// Create a new, empty [`FileChangeBuilder`].` before the `new()` function definition.
- [x] `crates/swissarmyhammer-validators/src/review/synthesize.rs:1` — This file exceeds the review prompt cap — 157073 rendered bytes against the 54338-byte batch budget — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q
- [x] `crates/swissarmyhammer-validators/src/review/tool_rules.rs:1` — This file exceeds the review prompt cap — 100176 rendered bytes against the 54338-byte batch budget — so these validators could not review it: duplication, reuse. Split the file into smaller modules that fit the review prompt cap. — not actioned — prompt-cap budget instability, see ^tsram0q

## Review Findings (2026-08-06 13:53)

> Scope: checkpoint 1c6407b3b (HEAD~1..HEAD at review time).
> Note: 1 engine finding was dropped per the review skill's written exception — its subject was a change to test code that existed before this commit (`scope/tests.rs:155` — the padding lines predate this commit; the commit's only change to that file was the `raw_source_bytes` call-site update).
> Note: 0 over-cap flags this pass — `skipped_files` was empty; nothing new to record on ^tsram0q.

- [x] `crates/swissarmyhammer-sem/src/git_types.rs:3` — Public enum `DiffScope` is missing `PartialEq` and `Eq` implementations. These are standard traits for any public data type and allow downstream crates to compare instances for equality; without them, those operations are impossible without re-implementing locally. Add `PartialEq, Eq` to the derive macro: `#[derive(Debug, Clone, PartialEq, Eq)]`.
- [x] `crates/swissarmyhammer-sem/src/git_types.rs:4` — Public enum `DiffScope` lacks documentation explaining the different scope targets for git operations. Add a doc comment like `/// Represents different git scope targets (working tree, staged, commit, range)` before the enum definition.
- [x] `crates/swissarmyhammer-sem/src/git_types.rs:33` — Public struct `CommitInfo` is missing `PartialEq` and `Eq` implementations. These are standard traits for public data structures and are necessary for equality comparisons; without them, downstream crates cannot effectively work with this type in collections or comparison contexts. Add `PartialEq, Eq` to the derive macro: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`.

## Review Findings (2026-08-06 14:10)

> Scope: checkpoint 7b5cf81a4 (HEAD~1..HEAD at review time).
> Note: 0 over-cap flags this pass — `skipped_files` was empty; nothing new to record on ^tsram0q.

- [x] `crates/swissarmyhammer-sem/src/git_types.rs:8` — DiffScope's derives are added in this change but omit Serialize/Deserialize, while CommitInfo (line 65, also modified in this commit) includes these derives. FileStatus (line 29, pre-existing) and FileChange (line 44, pre-existing) both have Serialize/Deserialize. Since all git-facing types in this module represent serializable diff metadata, DiffScope should follow the same pattern. Add Serialize, Deserialize to the DiffScope derive list on line 8 to maintain consistency with CommitInfo and other git-facing types in the module.
- [x] `crates/swissarmyhammer-sem/src/git_types.rs:8` — DiffScope is missing Serialize and Deserialize derives. Other data types in this module (FileStatus line 29, FileChange line 44, CommitInfo line 65) implement these for consistency across the semantic diff data model. Add Serialize and Deserialize to the derive list: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`. Also add `serde` import if not present for this to work.
- [x] `crates/swissarmyhammer-sem/src/git_types.rs:29` — FileStatus implements Eq but not Hash. Rust convention requires that types implementing Eq also implement Hash for safe use in collections. Add Hash to the derive list: `#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]`.
- [x] `crates/swissarmyhammer-sem/src/git_types.rs:65` — CommitInfo has snake_case field names (short_sha on line 70, author, date, message) and is serializable, but lacks the #[serde(rename_all = "camelCase")] attribute that FileChange (line 45) applies for consistent snake_case-to-camelCase serialization. Both are git-facing types with matching serialization structure. Add #[serde(rename_all = "camelCase")] after line 65 to ensure CommitInfo fields serialize as camelCase (shortSha) like FileChange, maintaining consistency across git-facing types.

## Review Findings (2026-08-06 14:29)

> Scope: checkpoint 5734aa85f (HEAD~1..HEAD at review time).
> Note: 0 over-cap flags this pass — `skipped_files` was empty; nothing new to record on ^tsram0q.

- [x] `crates/swissarmyhammer-sem/src/git_types.rs:1` — Crate-level docs lack examples showing common use cases, which reduces discoverability for users of this module. Add an `# Examples` section to the crate-level docs showing how to construct `DiffScope::Working`, `DiffScope::Commit`, `FileStatus` variants, and `FileChange` instances.
- [x] `crates/swissarmyhammer-sem/src/git_types.rs:32` — `FileStatus` enum does not implement `Display`, making it harder to log or display these values to users in human-readable form. Implement `Display` for `FileStatus` to show variant names in lowercase (e.g., `"added"`, `"modified"`, `"deleted"`, `"renamed"`).