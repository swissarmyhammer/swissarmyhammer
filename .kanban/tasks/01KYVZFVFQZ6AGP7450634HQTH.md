---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyw88zb6canf00sx814wkc0g
  text: |-
    SCOPE CHANGED by the user, mid-implementation. The card as written called for wiring the global `--format` flag into the tool path. Do not do that.

    Reason: JSON is a strict subset of YAML, so one JSON output satisfies every reader. Claude Code's hook runner strict-parses JSON; a YAML consumer parses that same JSON fine. There is no format to choose, so there is no flag to add and nothing to thread. Verified: a YAML parser reads `{"decision":"block","reason":"keep going"}` correctly.

    Revised requirement — emit JSON unconditionally from the ralph ops, as a single parseable object with no leading blank line. The current YAML output begins with a blank line, which would break a strict parse on its own. No `--format` wiring, no `table` rendering for arbitrary tool JSON, YAML retained for the other `sah tool` families so `sah tool kanban ...` does not start printing braces at a human.

    The user's other point — "it's not rocket science to try both" — applies where we own the reader. On INPUT we already do: `merge_parsed_stdin` (apps/swissarmyhammer-cli/src/main.rs:958) tries JSON then YAML. If our own hook engine (agent-client-protocol-extras / llama-agent) parses hook command stdout, make that reader tolerant the same way. On OUTPUT to Claude Code we cannot — that reader is not ours.

    This is strictly less work than the original card. Revised acceptance is on the card's dispatch, including a strict-parse proof through the real CLI and a RED-first test.

    One consequence for whoever finishes this: the Stop-hook command line in `builtin/skills/finish/SKILL.md` needs NO change under the new design. Under the `--format` design it would have had to become `sah --format json tool ralph ralph check --`. Emitting JSON unconditionally leaves the existing `sah tool ralph ralph check --` correct as written, which removes an edit to a file the user is actively rewriting.
  timestamp: 2026-07-31T14:11:58.438597+00:00
- actor: claude-code
  id: 01kyway9cebtegcqfhdgvx30r7
  text: |-
    Implemented. Note the approach changed mid-task: the card asked for the global `--format` flag to be honored. The user then narrowed the scope — JSON is a subset of YAML, so there is nothing to choose between and no flag to add. The ralph responder now prints JSON unconditionally; every other `sah tool` family keeps YAML.

    ## What changed

    - `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs` — new `McpTool::cli_output_is_json()`, default `false`. A tool declares whether its CLI output is read by a program. The tool owns the answer because the tool knows its consumer; a list kept in the CLI would drift when a tool moves or is renamed.
    - `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs` — `RalphTool` returns `true`. Its only CLI consumer is a Claude Code Stop hook.
    - `apps/swissarmyhammer-cli/src/mcp_integration.rs` — new `response_formatting::format_success_response_json`. Exactly one JSON document, nothing prepended. `format_success_response` (YAML) is untouched, so human output is byte-identical.
    - `apps/swissarmyhammer-cli/src/main.rs` — `tool_output_is_json()` asks the registry, alongside the existing `tool_has_operations` / `tool_schema` lookups; the value is threaded into `execute_tool_and_format`, which picks the renderer.
    - `apps/swissarmyhammer-cli/src/cli_executor.rs` — same branch, so the in-process test executor cannot disagree with the binary.
    - `crates/agent-client-protocol-extras/src/hook_config.rs` — new `parse_hook_stdout()`: JSON first, then YAML, mirroring `merge_parsed_stdin`. This is the only hook-stdout reader we own (`interpret_exit_0_stdout`, reached from `CommandHandler::handle`). It was `serde_json` only, and non-JSON stdout degraded to `HookDecision::Allow` with only a `tracing::warn!` — a hook that appeared to run and did nothing.
    - `standards/mcp.md`, `ARCHITECTURE.md` — document the output-format contract.
    - `apps/swissarmyhammer-cli/tests/tool_output_format.rs` (new) — spawns the compiled binary via `CARGO_BIN_EXE_sah` in a temp dir with `HOME` redirected.

    ## The leading blank line

    The YAML rendering prefixes `\n`, so stdout was `"\ndecision: allow\n\n"`. `serde_json` rejects that (`expected value at line 2 column 1`). The JSON renderer prepends nothing; stdout now starts with `{`. The test asserts `stdout.starts_with('{')` before parsing, so a future preamble fails loudly instead of silently.

    ## No `table` rendering

    None was added. The `--format` flag is not wired into the tool path at all.

    ## RED proven

    `every_ralph_operation_emits_strict_parseable_json` and `ralph_check_emits_json_when_no_instruction_is_active` both failed on `"\ndecision: allow\n\n"` before the change; `non_ralph_tool_output_stays_yaml` passed throughout as the control. In `agent-client-protocol-extras`, `hook_stdout_falls_back_to_yaml` failed to compile (no `parse_hook_stdout`) before the fix.

    ## Did not work / discovered

    - First attempt threaded `crate::cli::OutputFormat` from `main.rs` into `mcp_integration`. It does not compile: `src/cli.rs` is compiled twice — `lib.rs` has `pub mod cli`, `main.rs` has `mod cli` — so the two `OutputFormat`s are distinct types (`expected OutputFormat, found OutputFormat`). It needs a hand-written bridge conversion in `main.rs`, because `cli_conversions.rs` is itself duplicated and would produce `impl From<X> for X`. That whole branch is backed out; a plain `bool` from the tool avoids the problem. Worth knowing before anyone threads a `cli.rs` type into the library again.
    - The Stop-hook command line in `builtin/skills/finish/SKILL.md` needs NO change. It is already `sah tool ralph ralph check --`, and the output is now JSON. The fixtures in `crates/swissarmyhammer-skills/src/deploy.rs` are unaffected and green.
    - `HookOutput`'s `HookSpecificOutput::PreToolUse` fields are `Option` but not `#[serde(default)]`, so a partial `hookSpecificOutput` fails to deserialize the whole document and silently becomes `Allow`. Not touched here; possible separate card.

    ## Verification

    `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo nextest run -p swissarmyhammer-cli -p agent-client-protocol-extras -p swissarmyhammer-skills` 1276/1276 pass. `cargo nextest run -p swissarmyhammer-tools` 1464/1464 pass. Verified end to end with `./target/debug/sah`: all four ralph ops strict-parse through `python3 -c "import json,sys; print(json.load(sys.stdin))"`, and `sah tool kanban board get --` still prints YAML.

    `llama-agent` and `claude-agent` were NOT run — the run exceeded a 30-minute idle timeout (model loading). Neither crate is modified; both only consume `hook_config`, whose own crate tests are green, and `clippy --all-targets` compiled both.
  timestamp: 2026-07-31T14:58:33.998130+00:00
- actor: claude-code
  id: 01kywdmpbqd8n0ydt3avkr0gvw
  text: |-
    Closing. The 33-finding review: 1 fixed here, 32 SPLIT OUT, none dropped except one covered by the test-refactor exception.

    The one introduced item: `tool_output_is_json` was a third near-verbatim copy of `tool_has_operations` and `tool_schema` — same registry open, same lookup, same not-found error, differing only in the final method call. Extracted `tool_property` taking the reader as a closure; all three keep their names and signatures so no call site moved. Fixed in 1bea6b3ce.

    The 32 pre-existing, split by cause:
    - ^j0rkmeg — unvalidated `session_id` reaching `read_ralph`/`write_ralph`. **Security, highest value in the set.** Reachable from the CLI: `merge_parsed_stdin` merges arbitrary stdin into tool arguments, so `{"session_id":"../../../../tmp/pwned"}` escapes `.ralph/`, and `ralph set` is a write.
    - ^8fqg8dn — hook_config.rs handler merge, `route_hook_decision`, `$ARGUMENTS`, lowercase errors
    - ^yjk8kk0 — tool_registry.rs `get_` prefixes + `Eq` derive
    - ^pwakwya — ralph/execute dispatch from RALPH_OPERATIONS, macro, DEFAULT_MAX_ITERATIONS
    - ^fwbwzvq — main.rs/cli_executor.rs SERVE_COMMAND, nesting, ValueExtractor

    Dropped under the review skill's exception: one finding asking to extract a constant for `max_iterations = 25` in a pre-existing test.

    The reviewer's line-number forensics are worth keeping. The two path-traversal findings cited `ralph/execute/mod.rs:244` and `:253` — PRE-image numbers. In the post-image those land inside THIS commit's new doc comment for `cli_output_is_json`. Reading the cited line would have sent someone to a comment; the code the validator described is the pre-existing `execute()` match arms. Same artifact displaced the six hook_config findings by ~18 lines and the `PartialEq`/`Eq` one by ~17.

    I also asked the reviewer to judge, not assume, the untested `llama-agent`/`claude-agent` gap. It checked: neither crate is modified, `parse_hook_stdout` is new and private, `interpret_exit_0_stdout`'s signature is unchanged, and the behavior change only widens in the safe direction — JSON is still tried first and its error still reported. It also checked the one way widening could bite (stdout that used to fail JSON now parsing as YAML into a different decision) and found `HookOutput` does not deny unknown fields, so incidental mapping output yields `decision: None` → `Allow`, which the old path returned anyway. No plausible regression.

    Shipped — the ralph Stop hook now works end to end for the first time. Verified against the local build, default invocation, no flags:

    ```
    $ echo '{"session_id":"hookprobe"}' | ./target/debug/sah tool ralph ralph check --
    {
      "decision": "block",
      "iteration": 1,
      "max_iterations": 50,
      "reason": "work the board. Iteration 1 of 50."
    }
    ```

    Single object, strict-parses, no leading blank line. `sah tool kanban board get` still prints YAML.

    The design is smaller than this card originally specified. The user rejected the `--format` plumbing mid-implementation on the grounds that JSON is a strict subset of YAML, so one output satisfies every reader and there is no flag to thread. `McpTool::cli_output_is_json()` defaults false and `RalphTool` returns true — the tool declares its own consumer, rather than the CLI holding a name list that would drift, which is exactly what ^6xjxebg had to fix twice.

    Consequence: `builtin/skills/finish/SKILL.md` needed NO edit. Its hook line is already `sah tool ralph ralph check --`, which now emits JSON. Under the flag design it would have required editing a file the user is actively rewriting.

    Also fixed a live bug where "try both" genuinely applies: `interpret_exit_0_stdout` deserialized JSON only and silently returned `HookDecision::Allow` on anything else, so a YAML-answering hook appeared to run and did nothing — a meant-to-block hook became a permit. Now JSON then YAML, mirroring `merge_parsed_stdin`.

    Fourth instance this session of accept-then-silently-discard: ^1t92gnj (tags array), ^t7ebyn8 (hooks frontmatter), this card (YAML hook output), and ^ezgxksb (partial hookSpecificOutput, filed, not fixed).
  timestamp: 2026-07-31T15:45:45.335601+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8280
title: sah tool prints YAML, so the ralph Stop hook cannot read the decision
---
The `sah tool ...` path always prints YAML. Claude Code Stop hooks read JSON from stdout. The hook therefore cannot see the ralph decision.

## Symptom

```
$ echo '{"session_id":"probe"}' | sah tool ralph ralph check --
decision: block
iteration: 1
max_iterations: 50
reason: keep going. Iteration 1 of 50.
```

The global `--format json` flag does not change this:

```
$ echo '{"session_id":"probe"}' | sah --format json tool ralph ralph check --
decision: block
...
```

## Cause

`response_formatting::format_success_response` in `apps/swissarmyhammer-cli/src/mcp_integration.rs` converts every tool result to YAML. Its own doc comment says it is "the ONE PLACE where we convert JSON output to YAML for display". The tool execution path never reads the global `--format` value.

## Required change

Make the `sah tool` output honor the global `--format` flag (`table` | `json` | `yaml`). Keep YAML as the default for humans. Then the Stop hook command can ask for JSON.

## Acceptance

- `sah --format json tool ralph ralph check --` prints a JSON object.
- `sah tool ralph ralph check --` still prints YAML.
- A test covers both formats through the real CLI path.

Found while implementing ^6xjxebg. That card made the command exist; this card makes its output readable by the hook. #bug #cli #ralph

## Review Findings (2026-07-31 10:00)

Scope: `894d17993^..894d17993`.

Classification note: only the first item below is on code this commit introduced. Every item under "Pre-existing code" is on lines the commit did not write — most sit in the region the commit's insertions displaced, so the engine's cited line numbers are offset. Those items need their own cards; they do not gate this one.

### Introduced by this commit

- [ ] `apps/swissarmyhammer-cli/src/main.rs:846` — `tool_output_is_json` (added by this commit) is a near-verbatim third copy of `tool_has_operations` (line 829) and `tool_schema` (line 858). All three do the identical registry lookup and error handling — get `registry_arc`, `read()` the registry, call `get_tool()` with the same `"Tool not found: {}"` message — then call a different method on the tool and wrap the result. Only the final `Ok(...)` call differs. Extract one generic async helper `async fn get_tool_property<T>(cli_tool_context: &CliToolContext, full_tool_name: &str, getter: impl Fn(&dyn McpTool) -> T) -> Result<T, String>` that holds the lookup and error handling once, then define `tool_has_operations`, `tool_output_is_json`, and `tool_schema` as thin wrappers that call it with the appropriate getter closure.

### Pre-existing code (split to their own cards)

- [ ] `apps/swissarmyhammer-cli/src/cli_executor.rs:33` — Method accepts concrete `String` instead of generic `impl Into<String>` — less flexible API. Change signature to `pub fn error(stderr: impl Into<String>) -> Self` to accept `&str`, `String`, `Cow<str>`, etc.
- [ ] `apps/swissarmyhammer-cli/src/cli_executor.rs:254` — `extract_value_from_matches` reimplements the logic of `ValueExtractor` from main.rs, which already solves the same problem with more complete functionality (nullable type support). Both extract JSON values from clap matches based on schema type, but main.rs has the more sophisticated version. Extract `ValueExtractor` and its helper functions (`has_type`, `is_nullable`, `extract_nullable_boolean`, `extract_string_vec`) to a shared module in the same crate, then update `cli_executor.rs` to import and use `ValueExtractor` instead of implementing `extract_value_from_matches`. This consolidates the logic to one canonical implementation and brings nullable type support to the test executor.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:280` — Magic string "serve" is hardcoded in a conditional check. It is also hardcoded at line 710 in the command routing match. This repeated literal should be extracted as a named constant like STDIN_ARGS_SUBCOMMAND (already defined at line 411). Define a constant at the top of the file: `const SERVE_COMMAND: &str = "serve";` and use it in both locations.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:323` — Function `display_validation_report` contains 4 levels of nesting in multiple code paths, meeting the threshold for flagging. In the Success arm: match > if verbose > for category > for tool. In the Errors arm: match > if verbose > for (i, error) > if let Some(suggestion). Extract the verbose-conditional content into separate helper functions for each match arm (e.g., `fn display_success_details_verbose()` and `fn display_errors_details_verbose()`) to reduce nesting within the main function.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:374` — Hardcoded value 5 for max_warnings parameter should be a named constant. This configures how many validation warnings are displayed to the user and should be centrally defined. Define a constant at module level: `const MAX_VALIDATION_WARNINGS_DISPLAY: usize = 5;` and replace the literal with the constant name.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:642` — Public async function `handle_dynamic_matches` lacks documentation — it is a core routing function that dispatches CLI commands and needs to explain its role and contract. Add a doc comment explaining what this function does, its role in command dispatch, and what it returns.
- [ ] `apps/swissarmyhammer-cli/src/main.rs:710` — Magic string "serve" is hardcoded in the command routing match pattern. It is also hardcoded at line 280 in a conditional check. This repeated literal should be extracted as a constant. Extract as a named constant and reference it in both locations (see line 280 finding).
- [ ] `apps/swissarmyhammer-cli/src/mcp_integration.rs:162` — Public method uses `get_` prefix — violates idiom to avoid `get_` on getters. Should be `tool_registry_arc()` or similar. Rename to `tool_registry_arc()` or `registry()` to follow getter naming convention.
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:641` — Public trait `HookHandler` is not meant for downstream implementation — only internal trait objects created by config system. Should be sealed to prevent semver hazards when methods are added. Seal the trait using a private marker trait: add `mod private { pub trait Sealed {} }` and change trait to `pub trait HookHandler: private::Sealed + Send + Sync`, then impl Sealed for the three handler types.
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:802` — Error message starts with capital letter — violates Display message convention (must be lowercase). Change to `#[error("invalid regex pattern in hook matcher: {0}")]` (lowercase 'i').
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:804` — Error message starts with capital letter — violates Display message convention (must be lowercase). Change to `#[error("hook entry has empty hooks list")]` (lowercase 'h').
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:806` — Error message starts with capital letter — violates Display message convention (must be lowercase). Change to `#[error("prompt or agent hook requires a HookEvaluator, but none was provided")]` (lowercase 'p').
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:1140` — interpret_exit_2_stderr (lines 1149–1175) and interpret_prompt_response (lines 1498–1505) contain near-verbatim duplicated decision routing logic. Both functions apply identical conditional branching based on event_kind to produce a HookDecision:
  1. if is_blockable(event_kind) → HookDecision::Block { reason }
  2. else if Stop → HookDecision::ShouldContinue { reason }
  3. else if feeds_stderr_to_agent(event_kind) → HookDecision::AllowWithContext { context: reason }
  4. else → HookDecision::Allow (with optional warning)

  The two functions differ only in their input sources (parse output vs response object) and an optional warning log, not in the decision-routing pattern itself. This pattern could drift if changed in one place and not the other. Extract a shared helper function `fn route_hook_decision(event_kind: HookEventKind, reason: String) -> HookDecision` that encapsulates the event_kind-based routing logic once, then call it from both interpret_exit_2_stderr and interpret_prompt_response.
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:1179` — PromptHandler struct and impl (lines 1179–1217) is a near-verbatim copy of AgentHandler (lines 1219–1257). Both define identical field types and implement identical HookHandler logic, differing only in: (1) the boolean parameter passed to `evaluator.evaluate()` — `false` for PromptHandler, `true` for AgentHandler (lines 1209 vs 1244); (2) the error/warning log messages. This is one handler type with an argument, waiting to be extracted. Extract a single `EvaluatorHandler` struct with an `is_agent: bool` field, replacing both PromptHandler and AgentHandler. Pass `is_agent` to the evaluator.evaluate() call and parameterize the log messages by the boolean.
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:1238` — Magic string "$ARGUMENTS" is hardcoded in the PromptHandler.handle method for template replacement. It also appears at line 1293 in AgentHandler.handle with identical usage. This repeated literal should be extracted as a named constant. Define a constant near the top of the file: `const PROMPT_ARGUMENTS_PLACEHOLDER: &str = "$ARGUMENTS";` and use it in both PromptHandler (line 1238) and AgentHandler (line 1293).
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:1290` — The Prompt match arm in `build_handler` (lines 1290–1304) is a near-verbatim copy of the Agent match arm (lines 1306–1320). Both extract the evaluator, build field arguments identically, and return the result wrapped in `Arc::new(...)`, differing only in which handler type they instantiate (PromptHandler vs AgentHandler). Once PromptHandler and AgentHandler are merged into a single parameterized type, these match arms can be combined into a single arm that creates the handler with the appropriate `is_agent` parameter.
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:1471` — Magic strings "deny" and "block" are hardcoded in a match pattern representing permission decision values. These should be named constants or the permission_decision field should store enum variants instead of Option<String>. Define named constants (e.g., `const PERMISSION_DENY: &str = "deny";`) or refactor permission_decision to use an enum variant instead of Option<String>, so the complete set of valid values exists in one place.
- [ ] `crates/agent-client-protocol-extras/src/hook_config.rs:1476` — Magic string "allow" is hardcoded in a match pattern representing a permission decision value. Combined with "deny" and "block" (line 1471), these form a set that should be data. Extract permission decision values as named constants or refactor to use enum variants (see line 1471 finding).
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:721` — get_cli_categories() returns Vec<String> but uses get_ prefix. The get_ prefix should only be used for Option-returning methods per Rust API guidelines; collection-returning methods should omit the prefix. Rename to cli_categories() to match Rust conventions.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:748` — get_tools_for_category() returns Vec but uses get_ prefix. The get_ prefix should only be used for Option-returning methods; collection-returning methods should omit it. Rename to tools_for_category() to match Rust conventions.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:768` — get_cli_tools() returns Vec but uses get_ prefix. The get_ prefix should only be used for Option-returning methods; collection-returning methods should omit it. Rename to cli_tools() to match Rust conventions.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:833` — get_tool_validation_warnings() returns Vec but uses get_ prefix. The get_ prefix should only be used for Option-returning methods; collection-returning methods should use list_, plural form, or descriptive names without get_. Rename to validation_warnings() or tool_validation_warnings() to match collection-returning method conventions.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs:1203` — ToolValidationSeverity derives PartialEq but not Eq. Enums without NaN-like values must implement Eq whenever they implement PartialEq for complete equivalence semantics and type safety. Add Eq to the derive list: #[derive(Debug, Clone, Copy, PartialEq, Eq)].
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:21` — Operation struct definition and impl block appears four times with identical structure, differing only in enum-like values (verb, noun, description, parameters). This is repetitive copy-paste that could be eliminated with a macro. Extract operation definition into a macro (e.g., `define_operation!`) that generates the struct and impl together, parameterized by verb, noun, description, and parameters. Reduces four blocks to one macro definition and four concise macro invocations.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:109` — Default iteration limit 50 is hardcoded without a named constant. This configuration value should be extracted to a named constant for maintainability and clarity. Define `const DEFAULT_MAX_ITERATIONS: u32 = 50;` at module level and use `unwrap_or(DEFAULT_MAX_ITERATIONS)` on line 109.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:168` — Session ID extraction code appears identically in three match arms (set, clear, get). The repeated pattern is: extract session_id with optional defaulting to context.session_id, then convert to &str. Extract a helper method on ToolContext or execute to handle optional session ID extraction with context default. Example: `fn get_session_id_optional(args: &Map, context: &ToolContext) -> String { ... }`. Reduces code and eliminates drift risk.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:175` — RalphTool derives Default but not Debug. Public types should implement Debug for debuggability and to prevent downstream orphan rule issues. Change to #[derive(Default, Debug)].
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:196` — The `execute()` method contains a hardcoded match statement on operation strings (lines 196–250) with literals "set ralph", "check ralph", "clear ralph", and "get ralph". These same operation names are defined in the `Operation` trait implementations above (SetRalph, CheckRalph, ClearRalph, GetRalph), where verb() and noun() combine to form the op_string. Hardcoding the operation strings in execute duplicates the truth and risks desynchronization if an operation name is changed in the trait but not in the match. Instead of hardcoding operation strings in the match, dispatch by iterating through RALPH_OPERATIONS and matching on each operation's op_string() value, or build a dispatch map at startup from the operations' op_string values to handler functions. This keeps the operation names as the single source of truth.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:244` — Path traversal vulnerability: unvalidated user-supplied `session_id` from line 235-238 is passed directly to `read_ralph` file operation without validation. Validate `session_id` at line 235 before use. Use a whitelist pattern like `^[a-zA-Z0-9_-]+$` to ensure it contains only safe characters.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs:253` — Path traversal vulnerability: unvalidated user-supplied `session_id` from line 235-238 is passed directly to `write_ralph` file operation without validation. Validate `session_id` at line 235 before use. Use a whitelist pattern like `^[a-zA-Z0-9_-]+$` to ensure it contains only safe characters.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/mod.rs:30` — Public module `execute` lacks documentation comment. Public re-exported modules should document what they expose to users of this crate. Add a doc comment above `pub mod execute;`: `/// MCP tool implementation for ralph operations. Contains `RalphTool` and operation definitions.`.
