---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff880
project: cli-schema-gen
title: Migrate code-context-cli to schema-driven op commands
---
## What
Replace the hand-written operation subcommand enums in code-context-cli with schema-driven generation from the code_context tool's FULL schema (via card B's shared generator).

## Acceptance Criteria
- [x] code-context op commands are generated from the schema; the hand-written `*Commands` op enums are gone from `cli.rs`.
- [x] The generated command tree covers every code_context operation.
- [x] Lifecycle commands (serve/init/deinit/doctor/skill/completion) still work and build.rs generation compiles.
- [x] Existing dispatch still works: a generated op invocation reaches `CodeContextTool::execute`.

## Tests
- [x] Adapted `run_operation` integration tests through the schema-built tree + `extract_noun_verb_arguments`.
- [x] Command-tree coverage test (`generated_tree_covers_every_operation`, `build_cli_covers_every_operation`).
- [x] `cargo nextest run -p code-context-cli` passes.

## Review Findings (2026-06-07 19:13) — addressed in 2nd consolidation pass
(All items remain FIXED; see git history.)

## Review Findings (2026-06-07 22:26) — final re-review (working tree)

### Warnings
- [x] `apps/shelltool-cli/src/main.rs:360` — `dispatch_doctor_runs_diagnostics` asserted on a separately-constructed `ShelltoolDoctor::new()`, not on the `dispatch` call (its result was discarded). FIXED: rewrote as `dispatch_doctor_returns_doctor_verdict` — seeds a malformed `.shell/config.yaml` under a hermetic tempdir-as-CWD so the doctor verdict is deterministically exit code 2 (error), then asserts `dispatch(...)` returns 2. Verified RED: deleting the `doctor` dispatch arm routes "doctor" to the op-extraction arm which returns 1, failing the assertion (left:1, right:2).

### Nits
- [x] `apps/shelltool-cli/src/main.rs:311` — program name "shelltool" hardcoded as argv[0] in 5 test invocations despite `const PROGRAM`. FIXED: all five `matches_from(&[...])` calls now use `PROGRAM` for the argv[0] slot. Grep proof: the only `"shelltool"` literal left in main.rs is the `const PROGRAM` definition.
- [x] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:33` — public `InstallTarget` had a `DEFAULT` const but no `Default` impl. FIXED: added `impl Default for InstallTarget` delegating to the `DEFAULT` const so the standard trait and the const default are single-sourced (the const stays because `Default::default()` is not usable in the clap `default_value` const context). Added `default_matches_default_const` test.
- [x] `crates/swissarmyhammer-operations-macros/src/lib.rs:227` — single-caller `serde_args_contain_default` wrapper. FIXED: inlined the one-line `.any()` token scan into `has_serde_default`, keeping the explanatory comment. Regression tests still pass.

## Review Findings (2026-06-08 01:42)

### Nits
- [ ] `apps/code-context-cli/src/commands/ops.rs:70` — `run_operation` runs ~54 lines of actual code (lines 70-144: 75 physical, minus 13 comment-only and 8 blank lines), just over the ~50-line guideline. It mixes three distinct concerns — progress wiring, tool execution, and result rendering — which makes the success path harder to follow. Extract the result-rendering match into a small helper, e.g. `fn print_result(output: OutputMode, result: &CallToolResult) -> i32` covering lines 122-141, leaving `run_operation` to wire progress, execute, and delegate printing. That drops the body under 50 and isolates the JSON-vs-text formatting decision.
- [ ] `apps/kanban-cli/src/main.rs:1` — `handle_kanban_command` logs `error!("Error: {}", e)` for the failure from `extract_noun_verb_arguments`, adding a generic "Error:" prefix with no statement of what operation was being attempted; the error-handling rule asks for context on the operation, not a bare relayed message. Prefix with the operation, e.g. `error!("failed to parse command arguments: {e}")`, so the user can tell which stage failed.
- [ ] `apps/kanban-cli/src/main.rs:1` — `run_serve` collapses the serve failure to `error!("Error: {}", e)`; same generic prefix, no indication that the server (vs. some other step) is what failed. Use `error!("kanban serve failed: {e}")` to name the operation.
- [ ] `apps/shelltool-cli/src/main.rs:0` — The lifecycle artifact directory name ".shell" is hardcoded in three separate test bodies (dispatch_init_local_creates_shell_config, dispatch_deinit_local_removes_shell_config, dispatch_doctor_returns_doctor_verdict). If the ShellExecuteTool lifecycle ever renames its config dir, three tests silently keep asserting the stale path and must be hand-updated in lockstep — the exact drift a named constant prevents. Hoist a test-module `const SHELL_CONFIG_DIR: &str = ".shell";` (or better, expose the dir name from ShellExecuteTool so prod and tests share one source) and join against it in all three tests.
- [ ] `crates/swissarmyhammer-cli-completions/src/lifecycle.rs:220` — `global_flag` builds a clap `SetTrue` flag with an optional short alias — the same shape as `CliBuilder::create_flag_arg` in the sah app. Two near-identical flag builders now coexist, so a future change to flag construction has to be made in both. Low confidence — the contracts differ (`global_flag` is always `.global(true)`; `create_flag_arg` is not) and `create_flag_arg` is a private method in a separate crate, so this may be an acceptable separate-contract case. If the global-vs-not difference is the only axis, consider having sah's `create_flag_arg` delegate to the shared `global_flag` (or add a `global: bool` param) so flag construction is single-sourced.
- [ ] `crates/swissarmyhammer-kanban/src/board/get.rs:27` — `GetBoard::include_counts()` wraps `self.include_counts.unwrap_or(true)` and has a single call site (`if !self.include_counts()` in `execute`); a one-caller helper over a trivial expression is indirection without payoff. Defensible to keep if you want to name the default-true semantic in one place, but with one caller you could inline `self.include_counts.unwrap_or(true)` at the guard. Either is fine — flagging only for completeness.

(Verdict: 0 blockers, 0 warnings, 6 nits. All nits are cosmetic or out-of-scope/deferred items — none are grounds to keep an acceptance-met task out of done. Acceptance criteria, tests, and all prior findings resolved. Moved to DONE.)