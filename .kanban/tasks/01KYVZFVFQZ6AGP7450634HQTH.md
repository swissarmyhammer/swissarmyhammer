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
position_column: doing
position_ordinal: '8280'
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