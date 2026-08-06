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