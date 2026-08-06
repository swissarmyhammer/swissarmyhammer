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
depends_on:
- 01KZ934SNEJ1TXNS2G9Q4909TF
position_column: doing
position_ordinal: '8280'
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