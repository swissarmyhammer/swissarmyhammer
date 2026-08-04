---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz1qperrhbjdabrx2gj16z4a
  text: |-
    Picked up. Research notes before the change:

    - The tool description reaches the agent through `include_str!("description.md")` in `McpTool::description` for `ShellExecuteTool`. One file, one surface.
    - `builtin/skills/shell/SKILL.md` has no Liquid partial include, so the rendered body equals the file body with the frontmatter stripped. The new `rendered_builtin_instructions("shell")` test therefore asserts against the file directly.
    - `.skills/` is generated and is not tracked by git. It was not touched.

    Deviation from the card, with the reason: the card asks the new tools test to call `ShellExecuteTool::new()`. `new()` calls `ShellState::new()`, which makes a `.shell` directory in the process CWD. Every other test in that `mod tests` block uses the test-only `ShellExecuteTool::new_isolated()`, which puts the state in a temp dir. The test uses `new_isolated()` to follow the prevailing pattern and to keep stray `.shell` dirs out of the repo. The acceptance criterion is worded as `ShellExecuteTool::description()`, which `new_isolated()` satisfies.

    TDD record (RED before GREEN):
    - `cargo nextest run -p swissarmyhammer-skills shell_output_guidance` failed with "builtin skill 'shell' must state that execute command blocks until the command exits".
    - `cargo nextest run -p swissarmyhammer-tools shell_description` failed with: shell tool description must contain "blocks until the command exits".

    The skills test function is named `shell_output_guidance_states_blocking_and_no_tail` so the card's filter command `cargo nextest run -p swissarmyhammer-skills shell_output_guidance` matches it. A nextest substring filter matches the test name, not the test binary name, so a function name without that substring makes the card's command run 0 tests.
  timestamp: 2026-08-02T17:17:41.016263+00:00
- actor: claude-code
  id: 01kz1rwwy6khm0k1rp36d7e2td
  text: |-
    Double-check found one true defect, and I corrected it.

    Finding: the first wording was factually wrong on the timeout path. Both `description.md` and `SKILL.md` said "The full output stays in the history" with no limit. In `execute_command/mod.rs`, `run` branches to `finalize_timed_out` on `RunOutcome::TimedOut`, and `finalize_timed_out` only calls `mark_timed_out`. `store_command_output` is called only by `finalize_completed`. `tokio::time::timeout` drops the `execute_with_guard` future, so the buffered lines never reach `ShellState`. A timed-out command stores zero lines, and the response body has no output block. An agent that read the new text and then hit a timeout would call `get lines` for output that does not exist — the same trap the card is trying to close, inverted.

    Fix, applied to the whole of each file, not to the one sentence in the finding:
    - `description.md` — lead sentence now reads "Every command that exits stores its full output"; paragraph 2 splits the exit case from the timeout case and states "no output is stored".
    - `SKILL.md` — lead sentence, the `### execute command` line, the `## How output works` section, and the `## Timeout` section all state the limit.

    TDD for the correction as well: added the `no output is stored` assertion to both tests and watched both fail first.
    - skills: "builtin skill 'shell' must state that a timed-out command keeps nothing ('no output is stored') ..."
    - tools: shell tool description must contain "no output is stored"

    Other double-check results, all clean: no stale copy of the old text anywhere in the repo, no snapshot or golden of the shell description, `.skills/` is gitignored and absent on disk, and the GFM table with `\|` inside code spans renders correctly.

    Discovered work, filed as a separate card ^0dystg9: `cargo nextest run -p swissarmyhammer-tools` creates a `.shell` directory in `crates/swissarmyhammer-tools/` because `tool_config.rs` tests and `tests/integration/file_size_limits.rs` call `ShellExecuteTool::new()` instead of `new_isolated()`. Pre-existing, not caused by this card.
  timestamp: 2026-08-02T17:38:40.710297+00:00
- actor: claude-code
  id: 01kz1rx4ge10s1dfnqhakgwqbp
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-skills/tests/shell_output_guidance.rs (new), crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md, crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, builtin/skills/shell/SKILL.md. `cargo nextest run --workspace` — 15258 tests run: 15258 passed, 2 skipped. `cargo clippy --workspace --all-targets -- -D warnings` — clean. `cargo fmt --all` — clean.
    - next: ready for /review
  timestamp: 2026-08-02T17:38:48.462228+00:00
- actor: claude-code
  id: 01kz1tt1j3wc439a2edvceys8m
  text: |
    ### review — findings
    - evidence: 40 findings recorded from `review sha HEAD~1..HEAD` (engine: 42 confirmed, 17 refuted). Two engine findings dropped under the skill rule that bars refactoring test code that already existed: `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:435` and `:985`, both inside the `mod tests` block that starts at line 411 and that this commit did not touch.
    - text checked against code: the claims hold. `execute_command::run` awaits `run_with_optional_timeout` and returns only after the child exits or the timeout fires, so "blocks until the command exits" is true. `finalize_completed` calls `store_command_output` on the `Ok` arm, so "a command that exits stores its full output" is true; the `Err` arm covers spawn and output-capture failures, not an exited command. `finalize_timed_out` calls only `mark_timed_out`, so "no output is stored" on the timeout path is true.
    - next: fix the 40 findings. Most are missing doc comments on public trait methods, plus repeated string literals (`.shell`, `config.yaml`, `Bash`, `command_id`, `status`) that need named constants, a missing `Debug` derive on `ShellExecuteTool`, and an operation list hardcoded in the error message at `shell/mod.rs:566`. Remove each cause from the whole file, not only the flagged line.
  timestamp: 2026-08-02T18:12:04.291098+00:00
- actor: claude-code
  id: 01kz1tv9v2zc8cx3c2bj1mxrjj
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 5 files (description.md, execute_command/mod.rs op description, shell/mod.rs test, SKILL.md, new shell_output_guidance.rs)
    - test: green — cargo nextest run --workspace, 15258 passed, 0 failed, 2 skipped (pre-existing #[ignore] llama-agent real-model tests); clippy clean; fmt clean
    - commit: cfc469995 docs(shell): state execute command blocks and drop tail/head/grep piping (^mbran97)
    - review: findings — 40 open. Scope HEAD~1..HEAD. The text claims were judged TRUE against the code (run, finalize_completed, finalize_timed_out). All 40 findings are in production code the commit did not change: missing doc comments on public trait methods, repeated string literals needing named constants (`.shell`, `config.yaml`, `Bash`, `command_id`, `status`, the two health-check names), missing `Debug` derive on `ShellExecuteTool` (shell/mod.rs:95), and a hardcoded operation list in the unknown-operation error (shell/mod.rs:566) that duplicates `SHELL_OPERATIONS`.
    - next: iteration 2 — /implement works the unchecked findings.
  timestamp: 2026-08-02T18:12:45.538049+00:00
- actor: claude-code
  id: 01kz1x48w93m1kgfje6dfebh85
  text: |-
    Worked all 40 review findings. Each finding names ONE example of a cause, so I removed the cause from the whole file.

    What changed, by cause:

    1. Missing doc comments on public trait methods. Added to `Operation::verb/noun/description/parameters` in `execute_command/mod.rs`, and to `Default::default`, `Doctorable::is_applicable`, `McpTool::name/description/schema/schema_full/operations/category/execute` in `shell/mod.rs`. Added a doc comment to the public static `SHELL_OPERATIONS`, and to the private `EXECUTE_COMMAND_PARAMS` static so every module-level item in that file matches. The `pub mod` lines keep no `///`: every one of those modules carries its own `//!` inner doc, which is the pattern the whole `mcp/tools/` tree uses (see `files/mod.rs`).

    2. Repeated string literals. New named constants in `shell/mod.rs`: `SHELL_CONFIG_FILE`, `BASH_TOOL_NAME`, `BUILTIN_CONFIG_CHECK`, `REGEX_PATTERNS_CHECK`, `USER_CONFIG_CHECK`, `PROJECT_CONFIG_CHECK`, `SHELL_TOOL_CATEGORY`, `EXECUTE_COMMAND_OP`, `OP_KEY`, `SHELL_STATE_INIT_FAILED`. New `COMMAND_ID_KEY` and `STATUS_KEY` in `execute_command/mod.rs`, used in both response format strings. The findings named only `Regex patterns` and `User config`, but `Builtin config` and `Project config` repeat for the same reason, so they became constants too; the same holds for the category string `tools`, the argument key `op`, and the `expect` message `Failed to initialize shell state`. The `.shell` and `config.yaml` literals inside `format!` messages now read the constants as well, not only the path joins.

    3. Missing `Debug` derive. `ShellExecuteTool` is now `#[derive(Clone, Debug)]`. The finding said the derive would just work; it did not. `ShellState` had no `Debug`, so the derive did not compile. `ShellState` now derives `Debug` too — every field (`String`, `Vec<CommandRecord>`, `HashMap<usize, u32>`, `PathBuf`) already had it.

    4. Hardcoded operation list in the unknown-operation error. The message now builds the list from `SHELL_OPERATIONS` with `op_string()` joined by `", "`. The text is byte-identical to the old hardcoded string, in the same order, so no test changed meaning.

    TDD record: the `Debug` derive was RED first. `cargo nextest run -p swissarmyhammer-tools shell_execute_tool_renders_with_debug` failed to compile with "`shell::ShellExecuteTool` doesn't implement `std::fmt::Debug`", then passed after the derive. The other 39 findings are doc comments and constant extraction with no behavior change, so the existing suite is the guard; `test_unknown_operation_lists_all_valid_ops` already pins all five operation names in the error text.

    Corrections found by /double-check, all applied:

    - `SHELL_CONFIG_DIR` was a third source of truth for `.shell`. `swissarmyhammer_directory::ShellConfig::DIR_NAME` already holds that value and is what the real config loader and `ShellState` resolve through. Deleted the new const and used `ShellConfig::DIR_NAME` at every `.shell` site in `shell/mod.rs`, plus the two sites in `state.rs` (`cwd.join(...)` and the temp-dir prefix) that still hardcoded it. A private const would have let the health checks drift away from the directory the loader actually reads.
    - The doc I first wrote on `Default::default` said the tool "carries in-memory state". False: `ShellState::new()` creates a `.shell` directory, writes `.gitignore`, and appends every command's output to a log file on disk. Reworded, and reworded the same wrong phrase on `ShellExecuteTool::new`, which is where it came from.
    - The module `//!` doc still said "All output is stored in `ShellState`". That contradicts the fact this card pinned: `finalize_timed_out` calls only `mark_timed_out`, so a timed-out command stores nothing. Reworded to split the exit case from the timeout case.

    Not changed, on purpose: the literals inside `mod tests` (`"Bash"`, `"tools"`, `"execute command"`, the health-check names). The review skill bars refactoring tests that already existed, and replacing a test literal with the constant under test makes the assertion compare the constant with itself.

    Verification: `cargo fmt --all` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo nextest run --workspace` 15259 tests run, 15259 passed, 2 skipped (pre-existing `#[ignore]` llama-agent real-model tests).

    The documentation text this card added ("blocks until the command exits", "Do not pipe to `tail`", "no output is stored") is untouched in `description.md`, `SKILL.md`, and the operation description string.
  timestamp: 2026-08-02T18:52:36.617533+00:00
- actor: claude-code
  id: 01kz1x4fetxkrj72xpb7ds1hyc
  text: |-
    ### implement — changed
    - evidence: 3 files — crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs. All 40 review findings closed and checked on the card. `cargo nextest run --workspace` — 15259 tests run: 15259 passed, 2 skipped. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo fmt --all` clean.
    - next: ready for /review
  timestamp: 2026-08-02T18:52:43.354436+00:00
- actor: claude-code
  id: 01kz1xyfntydwygpk6gschz3yp
  text: |-
    ### test — green
    - evidence: `cargo fmt --all --check` clean (0 diffs); `cargo clippy --workspace --all-targets -- -D warnings` clean (0 warnings); `cargo nextest run --workspace` — 15259 passed, 0 failed, 2 skipped (623.06s)
    - the two shell-tool tests for this task both pass: `swissarmyhammer-skills::shell_output_guidance shell_output_guidance_states_blocking_and_no_tail` and `swissarmyhammer-tools mcp::tools::shell::tests::shell_description_states_blocking_and_no_tail`
    - the 2 skipped tests are `#[ignore]`d in `crates/llama-agent/tests/integration/kv_prefix_reuse_recurrent.rs` (`sibling_turns_reuse_pinned_prefix_without_rollback_on_recurrent_model` and a second recurrent-model test in the same file). These predate this task (introduced in commit `ae3de36ec`, an unrelated merged PR), are unrelated to the shell tool code under test, and carry an in-code rationale: the test hangs on the shared CI runner from real recurrent-model KV-slot contention under load, though it passes locally (~462s); the underlying selection/reuse logic is covered in CI by model-free unit tests in `queue.rs`. Not touched by this task's diff.
    - next: none, task can proceed to review
  timestamp: 2026-08-02T19:06:55.546753+00:00
- actor: claude-code
  id: 01kz1yqf9cf1hhzy2gjzf1j2zg
  text: |
    ### review — findings
    - evidence: 12 findings recorded from `review sha HEAD~1..HEAD` (commit e79415ab9; engine: 15 confirmed, 1 refuted). Three engine findings dropped under the skill rule that bars refactoring test code that already existed: `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:444`, `:450`, and `:1000`, all inside the `mod tests` block that starts at line 427 and that this commit did not touch.
    - prior round verified closed: all 40 items from the 2026-08-02 12:52 section hold in code. `ShellExecuteTool` derives `Debug` at `shell/mod.rs:139` and `ShellState` at `state.rs:63`. `SHELL_OPERATIONS` carries a doc comment at `shell/mod.rs:128` and the unknown-operation error at `shell/mod.rs:664` builds its list from that static with `op_string()`. The named constants exist (`SHELL_CONFIG_FILE`, `OP_KEY`, `SHELL_STATE_INIT_FAILED`, `BASH_TOOL_NAME`, the four health-check names, `SHELL_TOOL_CATEGORY`, `EXECUTE_COMMAND_OP`), and no raw `.shell` literal remains outside the test module — the code reads `ShellConfig::DIR_NAME`.
    - documentation claims checked against code, all true: `execute_command::run` awaits `run_with_optional_timeout` and returns only through `finalize_timed_out` or `finalize_completed`, so "blocks until the command exits or the timeout kills it" holds. `finalize_completed` calls `store_command_output` on the `Ok` arm, so "a command that exits stores its full output" holds. `finalize_timed_out` calls only `mark_timed_out`, so "no output is stored" on the timeout path holds.
    - next: fix the 12 findings. They are duplicated status literals (`completed`, `timed_out`) that the `CommandStatus` Display impl already owns, four operation-name literals in the dispatch match that need constants like `EXECUTE_COMMAND_OP`, a missing doc comment on `CommandRecord::duration()`, four `ShellState` signatures that take concrete `PathBuf`/`String`/`&[String]` instead of generic bounds, and a hardcoded default grep limit. Remove each cause from the whole file, not only the flagged line.
  timestamp: 2026-08-02T19:20:34.348085+00:00
- actor: claude-code
  id: 01kz1yrre19b3kg38z8pefp6e7
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 3 files (shell/mod.rs, shell/execute_command/mod.rs, shell/state.rs). All 40 prior findings closed.
    - test: green — cargo nextest run --workspace, 15259 passed, 0 failed, 2 skipped (pre-existing #[ignore] llama-agent real-model tests); clippy clean; fmt clean
    - commit: e79415ab9 fix(shell): close 40 review findings on shell MCP tool (^mbran97)
    - review: findings — 12 new open of 52 total. The prior 40 were verified closed in code. The documentation claims were re-confirmed true against `run`, `finalize_completed`, and `finalize_timed_out`. The 12 new findings are a DIFFERENT set from round 1, not repeats: duplicated status literals the `CommandStatus` Display impl already owns (execute_command/mod.rs:173, :249), four operation-name literals in the dispatch match (mod.rs:658-661), a missing doc comment on `CommandRecord::duration()` (state.rs:54), four `ShellState` signatures taking concrete types instead of generic bounds (state.rs:116, :121, :147, :174), and a hardcoded default grep limit (state.rs:305).
    - next: iteration 3 — /implement works the 12 unchecked findings. Guardrail not armed: no finding has repeated across 3 entries, and there are no no-change entries.
  timestamp: 2026-08-02T19:21:16.481203+00:00
- actor: claude-code
  id: 01kz1zj6qb3gzvmmgwwb1bp265
  text: |-
    Round 3. Worked the 12 findings in the newest `## Review Findings` section. Each finding names ONE example of a cause, so I removed the cause from the whole file.

    Judgement on the four generic-bounds findings (state.rs:116, :121, :147, :174). NOT a true conflict. Each change compiles, and no caller breaks that a mechanical call-site fix cannot correct.

    - `with_dir` and `new_in_dir` now take `impl AsRef<Path>`. Every caller already passed a `PathBuf`, which still coerces, so no call site changed.
    - `start_command` now takes `impl Into<String>`. This one DID break call sites: `state.start_command("echo hello".into())` becomes ambiguous under a generic bound (E0283), because both `T = String` and `T = &str` satisfy it. The fix is to drop the now-redundant `.into()`, which I did at 23 call sites in the `mod tests` block. That is a call-site repair the signature change forces, not a refactor of test logic, so the review-skill rule about pre-existing tests does not apply.
    - `append_lines` now takes `&[impl AsRef<str>]`. `&Vec<String>` and `&[String; N]` both still coerce, so no caller changed.
    - `new_with_preferred` (private) now takes `Option<impl AsRef<Path>>` for the same cause. That also removed a `dir.clone()` the old signature forced.

    What changed, by cause:

    1. Duplicated status literals (execute_command/mod.rs:173, :249). Both response format strings now render `CommandStatus::Completed` and `CommandStatus::TimedOut` through the `Display` impl, so `state.rs` owns the status text alone. The output is byte-identical. I swept the whole file: those were the only two production sites. `list_processes` and `kill_process` already read the status through `Display`.

    2. Operation-name literals in the dispatch match (mod.rs:658-661). New module-level constants `LIST_PROCESSES_OP`, `KILL_PROCESS_OP`, `GREP_HISTORY_OP`, `GET_LINES_OP`, matching the `EXECUTE_COMMAND_OP` pattern. Constants work directly as match patterns. The constants are a second name for a value `SHELL_OPERATIONS` also holds, so I added `test_dispatch_constants_match_the_operation_registry` to pin them to `op_string()`. A constant that drifted would route an advertised operation into the unknown-operation arm.

    3. Missing doc comment on `CommandRecord::duration()` (state.rs:54). The finding text says "all public items should be documented", so I documented every undocumented public item in the file, not the one method: all four `CommandStatus` variants, all nine `CommandRecord` fields, `ShellState::session_id`, all three `GrepResult` fields, and the `Display::fmt` impl. A script now reports 0 undocumented public items in the file.

    4. Hardcoded default grep limit (state.rs:305). New `DEFAULT_GREP_LIMIT`. The same cause — a hardcoded default for a configurable parameter in an `unwrap_or` — also sat in `get_lines`, so `start.unwrap_or(1)` became `DEFAULT_START_LINE`. The `end.unwrap_or(usize::MAX)` sentinel is already a named constant. The `grep` doc that said "default 10" now links the constant.

    TDD record, RED before GREEN. All five new tests were written first and failed to compile: `cargo nextest run -p swissarmyhammer-tools shell::` reported 15 errors — E0425 for the unresolved `DEFAULT_GREP_LIMIT`, `DEFAULT_START_LINE`, and the four op constants, plus E0308 "expected String, found &str" and "expected PathBuf, found &Path" for the borrowed-argument tests. After the change all five pass.

    Verification: `cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo nextest run --workspace` 15264 run, 15264 passed, 2 skipped (the same pre-existing `#[ignore]` llama-agent real-model tests).

    The documentation text this card added is untouched: `blocks until the command exits`, ``Do not pipe to `tail` ``, and `no output is stored` all still read the same in `description.md`, `SKILL.md`, and the operation description string. The round-1 fixes are also untouched.
  timestamp: 2026-08-02T19:35:10.315975+00:00
- actor: claude-code
  id: 01kz21cnjyed06r4pba0bsmppy
  text: |-
    `/double-check` returned REVISE with 7 items. I acted on 6 and declined 1, with the reason.

    Acted on:

    1. `grep_history/mod.rs` advertised a grep default of 50 while the code capped at 10. This is a real agent-facing lie and the same cause as finding 12 — the default duplicated outside its one source of truth. `execute_grep_history` passes `None` straight through, so `ShellState::grep` applies `DEFAULT_GREP_LIMIT`. The description now reads 10. `ParamMeta::description` is a `const fn` taking `&'static str`, and the workspace carries no `const_format`, so a compile-time composed string is not available; instead each advertised default is now pinned by a test that asserts the parameter description contains `(default: {constant})`. Two tests: `test_limit_parameter_advertises_the_real_default` in `grep_history` and `test_start_parameter_advertises_the_real_default` in `get_lines`. Drift can no longer land.

    2. Three doc comments I had written in `state.rs` were false. Corrected:
       - `CommandStatus::Killed`. I had said `kill process` stops the command. That state is transient. `kill process` SIGKILLs the group, but the concurrent `execute command` task still owns the child: `execute_with_guard` returns, `format_execution_result` yields `exit_status.code().unwrap_or(-1)` = -1 for a signal death, and `store_command_output` then writes `Completed` / `Some(-1)` over the `Killed` record. The doc now states that, and states the one case where `Killed` survives.
       - `CommandRecord::exit_code`. I had said `Some(-1)` marks a timeout. It also marks a signal death and a failure inside the shell (`mark_command_errored` calls `complete_command(cmd_id, Some(-1))`). The doc now says `-1` means no exit code was reported, and names all three sources.
       - `CommandStatus::Running`. I had said the command was spawned. `prepare_command` calls `start_command` BEFORE `spawn_shell_command`, so a spawn failure leaves a record that was never spawned. Reworded to match.

    3. The generic bounds bought nothing at the two production call sites, which still allocated. Now `guard.start_command(request.command.as_str())` instead of `.clone()`, and `append_stream` collects `Vec<&str>` instead of `Vec<String>`. Two allocations per command removed, which is the benefit findings 10 and 11 asked for.

    4. `finalize_timed_out`'s response text had no test — nothing in the repo asserted `status: timed_out`, so finding 2's refactor was unverified. New test `test_execute_response_reports_timed_out_status` runs `sleep 30` with `timeout: 1` and asserts both `status: timed_out` and `timeout: 1s`.

    5. `test_get_lines_without_start_begins_at_default_start_line` was not discriminating — its assertion duplicated `test_get_lines_all`. It now asserts the absent `start` reads identically to `Some(DEFAULT_START_LINE)` AND differently from `Some(DEFAULT_START_LINE + 1)`.

    6. Filed the spawn-failure leak as its own card, ^xvwtm2z: a failed spawn leaves a permanent `running` record that `list processes` shows with a duration that grows without bound, and that `kill process` cannot clear because no PID was ever registered. Out of scope here, real bug.

    Proof the four new guard tests are not vacuous. I perturbed each pinned production value and watched every one fail, then reverted: `TimedOut => write!(f, "PERTURBED")`, `start.unwrap_or(DEFAULT_START_LINE + 1)`, the grep description back to 50, and the get_lines description to 2. Result: 4 failed, and all 4 pass again after the revert.

    Declined, with the reason: item 5 of the double-check asked me to replace the status and operation-name literals inside the `mod tests` blocks of the same two files. Those tests already existed, and the review skill's written rule bars refactoring test code that already existed — the same rule under which the reviewer itself dropped three engine findings on this card in each of the last two rounds. Beyond the rule, the literal IS the oracle: `text.contains("status: completed")` compared against `CommandStatus::Completed` would compare the implementation with itself and stop catching a wire-format change. The independent coverage the double-check wanted now exists as new tests instead.

    Final verification: `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero warnings; `cargo nextest run --workspace` 15267 run, 15267 passed, 2 skipped, 1 flaky (`swissarmyhammer-agent::review_real_model_e2e`, a real-model ACP test unrelated to the shell tool, passed on retry 2).
  timestamp: 2026-08-02T20:07:06.078692+00:00
- actor: claude-code
  id: 01kz21cydgfcc35wr5zmqsx1y2
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/grep_history/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/get_lines/mod.rs. All 12 findings in the 2026-08-02 14:08 section closed and checked; the 40 in the 12:52 section stay checked. `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero warnings; `cargo nextest run --workspace` 15267 run, 15267 passed, 2 skipped, 1 flaky (unrelated real-model ACP test, passed on retry).
    - next: ready for /review. New card ^xvwtm2z holds the spawn-failure `running` leak found on the way.
  timestamp: 2026-08-02T20:07:15.120436+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8f80
title: 'shell tool: say that `execute command` blocks to completion, and stop `| tail` pipelines'
---
## What

The `shell` MCP tool runs every command to completion. `execute_command::run`
(`crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:79`)
returns only after the child process exits or the timeout kills it. The full
output goes to the shell log, and the response shows the last 32 lines.

The agent-facing text does not say this. The tool description
(`crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md`) is one
sentence. The skill (`builtin/skills/shell/SKILL.md:18`) has only a weak
bullet: "skip `| tail` / `| grep` pipelines". Agents thus keep writing
`cmd 2>&1 | tail -60`, which throws away output that the tool already keeps.

Make three text surfaces state the same two facts:

1. `execute command` blocks until the command exits or the timeout kills it.
2. Do not pipe to `tail`, `head`, or `grep`. Read the output later with
   `get lines` or `grep history`.

Use these two marker sentences verbatim in each surface, because the tests
below assert on them:

- `blocks until the command exits`
- ``Do not pipe to `tail` ``

Files to change:

- `crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md` — add the
  two facts. This file is the tool description through `include_str!` at
  `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:500`.
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:56`
  — change the `Operation::description()` string from
  "Execute a shell command with timeout and environment control" to also say
  the op blocks until the command exits.
- `builtin/skills/shell/SKILL.md` — replace the weak bullet at line 18 with a
  clear section that gives both facts. Do NOT edit `.skills/`; that directory
  is generated from `builtin/skills/`.

### Subtasks

- [x] Write the two failing tests first (see **Tests**).
- [x] Update `description.md` with the blocking fact and the no-pipe rule.
- [x] Update the `ExecuteCommand::description()` string.
- [x] Update `builtin/skills/shell/SKILL.md`.

### Added during implementation — the timeout limit

A double-check found that the first wording was false on the timeout path.
`finalize_timed_out` only calls `mark_timed_out`; `store_command_output` runs
solely in `finalize_completed`, and `tokio::time::timeout` drops the buffered
output. A command the timeout kills therefore stores nothing, and `get lines`
and `grep history` find nothing for it. Text that promises stored output
without that limit sends the agent to read output that was never written.

- [x] All three surfaces state the limit with the marker `no output is stored`.
- [x] Both tests assert the marker.
- [x] The same unqualified promise removed from the whole of each file — the
      lead sentence of `description.md`, the lead sentence and the
      `### execute command` line of `SKILL.md`, and the `## Timeout` section.

## Acceptance Criteria

- [x] `ShellExecuteTool::description()` contains `blocks until the command exits`.
- [x] `ShellExecuteTool::description()` contains ``Do not pipe to `tail` ``.
- [x] `ShellExecuteTool::description()` names both `get lines` and
      `grep history` as the way to read output after the command ends.
- [x] `ExecuteCommand::description()` says the op blocks until the command exits.
- [x] The rendered `shell` skill body contains both marker sentences.
- [x] `builtin/skills/shell/SKILL.md` no longer contains the old weak bullet
      text "skip `| tail` / `| grep` pipelines".
- [x] Both new tests pass; no other test in the workspace breaks.

## Tests

- [x] New file `crates/swissarmyhammer-skills/tests/shell_output_guidance.rs`,
      modeled on `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs`.
      Use `mod common;` and `use common::rendered_builtin_instructions;`. Assert
      that `rendered_builtin_instructions("shell")` contains
      `blocks until the command exits` and ``Do not pipe to `tail` ``, and that
      it does not contain `skip `| tail``.
- [x] New test `shell_description_states_blocking_and_no_tail` in the `mod tests`
      block of `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`. Assert
      that `ShellExecuteTool::new().description()` contains both marker
      sentences plus `get lines` and `grep history`, and that
      `super::EXECUTE_CMD.description()` contains `blocks until the command exits`.
- [x] Run `cargo nextest run -p swissarmyhammer-skills shell_output_guidance` —
      expect the new test to fail before the text changes and pass after.
- [x] Run `cargo nextest run -p swissarmyhammer-tools shell_description` —
      expect the new test to fail before the text changes and pass after.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
#docs #shelltool #tools

## Review Findings (2026-08-02 12:52)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:50` — Public trait method `verb` implementing Operation trait lacks documentation. Add documentation comment for the verb method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:50` — Public trait method `verb()` in `impl Operation` lacks a doc comment. Add doc comment above line 50: `/// Returns the verb part of the operation string (e.g., "execute").`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:53` — Public trait method `noun` implementing Operation trait lacks documentation. Add documentation comment for the noun method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:53` — Public trait method `noun()` in `impl Operation` lacks a doc comment. Add doc comment above line 53: `/// Returns the noun part of the operation string (e.g., "command").`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:56` — Public trait method `description` implementing Operation trait lacks documentation. Add documentation comment for the description method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:59` — Public trait method `parameters` implementing Operation trait lacks documentation. Add documentation comment for the parameters method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:59` — Public trait method `parameters()` in `impl Operation` lacks a doc comment. Add doc comment above line 59: `/// Returns the metadata for operation parameters (command, timeout, etc.).`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:158` — Response format keys 'command_id' and 'status' are hardcoded in this format string and repeated at line 234. Should be named constants so changes occur in one place. Extract constants: `const COMMAND_ID_KEY: &str = "command_id";` and `const STATUS_KEY: &str = "status";` then use them in both format strings (lines 158 and 234).
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:234` — Response format keys 'command_id' and 'status' are hardcoded in this format string and repeated at line 158. Should be named constants so changes occur in one place. Extract constants: `const COMMAND_ID_KEY: &str = "command_id";` and `const STATUS_KEY: &str = "status";` then use them in both format strings (lines 158 and 234).
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:84` — Public static constant `SHELL_OPERATIONS` lacks documentation describing its contents and purpose. Add a /// doc comment explaining that this is the static list of shell operations available to the tool.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:84` — Public static `SHELL_OPERATIONS` lacks a doc comment, violating the requirement that all public items have doc comments. Add doc comment above line 84: `/// Static registry of all supported shell operations (execute, list, kill, grep, get lines).`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:95` — `ShellExecuteTool` is a public type with non-empty representation (state, mcp_server fields) but does not implement or derive `Debug`. Change line 95 to `#[derive(Clone, Debug)]`. Both `Arc<Mutex<ShellState>>` and `Option<(String, McpServerEntry)>` implement Debug, so the derive will succeed.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:108` — Public trait method `default` implementing Default trait lacks documentation. Add documentation comment for the default method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:183` — Health check name 'Regex patterns' is hardcoded here and also appears at line 188 in the same function. Should be a named constant. Define `const REGEX_PATTERNS_CHECK: &str = "Regex patterns";` at function scope or module level and use it in both locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:207` — Filename 'config.yaml' is hardcoded here and also appears at lines 220 and 294. Should be a named constant so changes occur in one place. Define `const SHELL_CONFIG_FILE: &str = "config.yaml";` and use it in all three locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:210` — Health check name 'User config' is hardcoded here and also appears at line 215 in the same function. Should be a named constant. Define `const USER_CONFIG_CHECK: &str = "User config";` at function scope or module level and use it in both locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:220` — Directory name '.shell' is hardcoded here and also appears at lines 207, 293, and 469. Should be a named constant so changes occur in one place. Define `const SHELL_CONFIG_DIR: &str = ".shell";` and use it in all four locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:220` — Filename 'config.yaml' is hardcoded here and also appears at lines 207 and 294. Should be a named constant so changes occur in one place. Define `const SHELL_CONFIG_FILE: &str = "config.yaml";` and use it in all three locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:293` — Directory name '.shell' is hardcoded here and also appears at lines 207, 220, and 469. Should be a named constant so changes occur in one place. Define `const SHELL_CONFIG_DIR: &str = ".shell";` and use it in all four locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:294` — Filename 'config.yaml' is hardcoded here and also appears at lines 207 and 220. Should be a named constant so changes occur in one place. Define `const SHELL_CONFIG_FILE: &str = "config.yaml";` and use it in all three locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:343` — Public trait method `is_applicable` implementing Doctorable trait lacks documentation. Add documentation comment for the is_applicable method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:343` — Public method `is_applicable()` in `impl Doctorable` lacks a doc comment. Add doc comment above line 343: `/// Returns whether health checks are applicable for this component.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:400` — Tool name 'Bash' is hardcoded here and also appears at lines 445 and 525. Should be a named constant since this tool's purpose is to replace Bash. Define `const BASH_TOOL_NAME: &str = "Bash";` and use it in all three locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:445` — Tool name 'Bash' is hardcoded here and also appears at lines 400 and 525. Should be a named constant since this tool's purpose is to replace Bash. Define `const BASH_TOOL_NAME: &str = "Bash";` and use it in all three locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:469` — Directory name '.shell' is hardcoded here and also appears at lines 207, 220, and 293. Should be a named constant so changes occur in one place. Define `const SHELL_CONFIG_DIR: &str = ".shell";` and use it in all four locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:495` — Public trait method `name` implementing McpTool trait lacks documentation. Add documentation comment for the name method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:499` — Public trait method `description` implementing McpTool trait lacks documentation. Add documentation comment for the description method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:499` — Public method `description()` in `impl McpTool` lacks a doc comment. Add doc comment above line 499: `/// Returns the tool description from description.md for agent guidance.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:503` — Public trait method `schema` implementing McpTool trait lacks documentation. Add documentation comment for the schema method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:503` — Public method `schema()` in `impl McpTool` lacks a doc comment. Add doc comment above line 503: `/// Returns the wire protocol schema with operation definitions.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:507` — Public trait method `schema_full` implementing McpTool trait lacks documentation. Add documentation comment for the schema_full method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:507` — Public method `schema_full()` in `impl McpTool` lacks a doc comment. Add doc comment above line 507: `/// Returns the full schema with CLI-facing keys and operation signatures.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:511` — Complex public trait method `operations` lacks documentation. Add documentation explaining what this method returns and how it's used.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:511` — Public method `operations()` in `impl McpTool` lacks a doc comment. Add doc comment above line 511: `/// Returns the list of supported shell operations.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:522` — Public trait method `category` implementing McpTool trait lacks documentation comment. Add documentation comment (///) for the category method.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:522` — Public method `category()` in `impl McpTool` lacks a doc comment. Add doc comment above line 522: `/// Returns the tool category as a Bash replacement for agent capability routing.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:525` — Tool name 'Bash' is hardcoded here and also appears at lines 400 and 445. Should be a named constant since this tool's purpose is to replace Bash. Define `const BASH_TOOL_NAME: &str = "Bash";` and use it in all three locations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:528` — Complex async public method `execute` lacks documentation describing its operation dispatch and error handling. Add documentation explaining how the method dispatches based on the `op` parameter and what each operation path does.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:528` — Public method `execute()` in `impl McpTool` lacks a doc comment. Add doc comment above line 528: `/// Dispatch a shell tool operation and return the MCP result.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:566` — Operation names are hardcoded in the error message, duplicating the known set defined in SHELL_OPERATIONS. If operations are added, renamed, or removed, this string must be manually updated or it becomes incorrect and stale. Generate the operation list in the error message by iterating SHELL_OPERATIONS and collecting op_string() results, e.g., `format!("unknown operation '{}'. Valid operations: {}", other, SHELL_OPERATIONS.iter().map(|o| o.op_string()).collect::<Vec<_>>().join(", "))`.

## Review Findings (2026-08-02 14:08)

- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:173` — Hardcoded status string "completed" is duplicated with the CommandStatus::Completed Display impl in state.rs:32. The same literal appears in multiple files and should be extracted to a named constant or unified through a single source of truth. Extract the status strings to module-level constants in state.rs (e.g., `const STATUS_COMPLETED: &str = "completed";`) and import them in execute_command/mod.rs, or modify the response formatting to use the Display impl directly.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:249` — Hardcoded status string "timed_out" is duplicated with the CommandStatus::TimedOut Display impl in state.rs:34. The same literal appears in multiple files and should be extracted to a named constant or unified through a single source of truth. Extract the status strings to module-level constants in state.rs and import them in execute_command/mod.rs, or modify the response formatting to use the Display impl directly.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:658` — Hardcoded operation name string "list processes" should be extracted to a named constant for consistency with EXECUTE_COMMAND_OP. All operation names in the match statement should use constants to make maintenance easier if operation names change. Extract the operation name strings to module-level constants: `const LIST_PROCESSES_OP: &str = "list processes";` etc., and use them in the match statement at lines 658–661. This aligns with the pattern already established by EXECUTE_COMMAND_OP and improves maintainability.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:659` — Hardcoded operation name string "kill process" should be extracted to a named constant for consistency with EXECUTE_COMMAND_OP. Extract to a module-level constant and use it in the match statement.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:660` — Hardcoded operation name string "grep history" should be extracted to a named constant for consistency with EXECUTE_COMMAND_OP. Extract to a module-level constant and use it in the match statement.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:661` — Hardcoded operation name string "get lines" should be extracted to a named constant for consistency with EXECUTE_COMMAND_OP. Extract to a module-level constant and use it in the match statement.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs:54` — Public method `duration()` on public struct `CommandRecord` lacks documentation. In Rust, all public items should be documented, especially methods that perform non-trivial computation (here: returning elapsed time with different handling for running vs completed states). Add a doc comment explaining what duration returns and the behavior for running vs completed commands, e.g.: `/// Returns the elapsed time from command start. For completed commands, returns the duration between start and completion; for running commands, returns the time since start until now.`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs:116` — Function parameter accepts concrete `PathBuf` instead of generic `impl AsRef<Path>`. This prevents callers from passing `&Path`, `&str`, or other path types without allocation or conversion. Change signature to `pub fn new_in_dir(shell_dir: impl AsRef<Path>)` and update the function body to use `shell_dir.as_ref()` when passing to `with_dir()`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs:121` — Function parameter accepts concrete `PathBuf` instead of generic `impl AsRef<Path>`. Prevents callers from passing `&Path`, `&str`, or other path types without allocation. Change signature to `pub fn with_dir(shell_dir: impl AsRef<Path>)` and update function body: `fs::create_dir_all(shell_dir.as_ref())?` instead of `fs::create_dir_all(&shell_dir)?`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs:147` — Function parameter accepts concrete `String` instead of generic `impl Into<String>`. Prevents callers from passing `&str` without allocation, requiring them to call `.to_string()` or `.into()` themselves. Change signature to `pub fn start_command(&mut self, command: impl Into<String>)` and update line 142 to `command: command.into()` in the CommandRecord struct initialization.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs:174` — Function parameter accepts `&[String]` (concrete slice type) instead of `&[impl AsRef<str>]` (generic). This prevents callers with `&[&str]` or other string-like types from passing data without converting to `Vec<String>` first. Change signature to `pub async fn append_lines(&mut self, cmd_id: usize, lines: &[impl AsRef<str>])` and update line 187 to use `line.as_ref()` in the format string or convert before the loop.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/state.rs:305` — Hardcoded default grep result limit of 10 should be a named constant. This is a configurable behavior parameter, not a loop index or assertion literal. Define a constant `const DEFAULT_GREP_LIMIT: usize = 10;` at the top of the impl block or module, then use `limit.unwrap_or(DEFAULT_GREP_LIMIT)`.