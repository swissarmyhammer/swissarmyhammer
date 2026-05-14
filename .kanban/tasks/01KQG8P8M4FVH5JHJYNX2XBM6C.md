---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffb780
project: acp-upgrade
title: Fix or stabilize llama-agent test_validator_shaped_multi_turn_with_real_model
---
## What

`llama-agent` test `integration::tool_use_multi_turn::test_validator_shaped_multi_turn_with_real_model` (file: `llama-agent/tests/integration/tool_use_multi_turn.rs:310`) fails deterministically:

```
Expected at least one tool-role message — the loop should dispatch read_file. Got 0 tool messages.
```

The model emits a `<think>...</think>` block and then a final JSON verdict (`{"status": "passed", "message": "ok"}`) directly, without ever calling the `read_file` tool. The test's contract is that the validator-shaped multi-turn loop MUST dispatch at least one tool call.

This was last touched in `9ecf3e70e feat(avp): make validator pipeline single-path, tools-aware, and verifiable` — investigate whether the test prompt or the validator pipeline regressed during the ACP 0.11 migration such that tool calls are no longer being parsed/dispatched, OR whether the model behavior is the actual issue (in which case the test prompt needs to be tightened so the model has no choice but to call the tool).

## Investigation Outcome

**Diagnosis: Real-model capability limitation, NOT an ACP migration regression.** The tool-call parsing/dispatch path is verified intact by:
- `tool_call_round_trip::test_tool_call_round_trip_with_real_model` — passes (proves parsing path).
- `tool_use_multi_turn::test_multi_turn_tool_use_round_trip_with_real_model` — passes (proves full multi-turn dispatch loop).
- `tool_call_round_trip_via_mcp::test_full_round_trip_with_mcp_fetched_tools_against_real_model` — passes (full MCP round trip).

All three exercise the same agentic loop and the same Qwen3-0.6B test model. Only the validator-shaped variant fails — specifically because the conditional-verdict prompt invites the small model to short-circuit the call.

Prompt-tightening attempts that did NOT recover the call (Qwen3-0.6B is too small):
1. Imperative step-1 framing (`Use the read_file tool to read the file at <path>`) — model still skips call, hallucinates verdict.
2. `/no_think` directive to suppress thinking-mode — model emits empty `<think></think>` then verdict directly without call.

The verdict-shape requirement itself appears to be the trigger. The model treats the rule as a logic puzzle to solve in `<think>` rather than a procedure to execute.

## Resolution

Per acceptance criteria #2: skipped behind a feature flag (`#[ignore]`) with documented reasoning. The test is preserved in source as a real-model sanity check that can be opted into by larger-model CI runs:

```text
cargo nextest run -p llama-agent --test agent_tests \
    integration::tool_use_multi_turn::test_validator_shaped_multi_turn_with_real_model \
    --run-ignored=all
```

When a stronger test model is wired into `test_models.rs` (or the avp validator gains a stricter system prompt that survives Qwen3-0.6B-class models), the `#[ignore]` attribute should be removed.

## Acceptance Criteria

- [x] `cargo nextest run -p llama-agent --test agent_tests integration::tool_use_multi_turn::test_validator_shaped_multi_turn_with_real_model` passes deterministically across at least 3 consecutive runs. **(Skipped under default invocation, 3 consecutive default runs all pass with the test correctly ignored. The opt-in `--run-ignored=all` form runs the test and surfaces the model-capability failure for diagnostic value.)**
- [x] If the issue is model nondeterminism, the prompt is tightened or the test is skipped behind a feature flag with documented reasoning. **(Test is `#[ignore]`d with full docstring explaining why and how to opt in.)**
- [ ] If the issue is a regression in tool-call parsing, the parser is fixed and a smaller unit test is added that doesn't depend on a real model. **(Not applicable — sibling tests prove the parser is not regressed.)**

## Tests

- The test in question is the regression test.
- Confirm `tool_call_round_trip::test_tool_call_round_trip_with_real_model` still passes (it does today) — that test does dispatch a tool call successfully, so the parsing path is at least partially fine. **(Confirmed: passes.)**

## Workflow

- Compare the prompt in `tool_use_multi_turn.rs` against `tool_call_round_trip.rs` to see why one model-driven test calls the tool and the other does not. **(Done.)**
- If the difference is just prompt strength, tighten the prompt; if there's a real parsing regression, fix it. **(Tightened prompt insufficient against Qwen3-0.6B; gated behind `#[ignore]` per acceptance criterion #2.)**

## Depends on

- 01KQ36C3JQ5GKVYXAYW66J4H9H (workspace-wide green) — this task is the route-back for failures discovered there.

## Review Findings (2026-04-29 22:14)

Test-integrity scrutiny: the `#[ignore]` is **genuinely justified**, not a `no-test-cheating` violation.

Verified:
- **Coverage overlap is real for code paths.** The sibling `test_multi_turn_tool_use_round_trip_with_real_model` (same file, lines 280-298) asserts `tool_count >= 1` AND that the model's final response references fixture-only content (`main`) — proving dispatch + result-feedback at the same level as the ignored test. The unique aspect of the validator-shaped variant is structured-JSON-verdict-shape under a conditional rule prompt, which is a model-quality property, not a llama-agent code path.
- **Not an ACP-migration regression.** `git diff mcp..HEAD -- llama-agent/src/{agent.rs,mcp.rs,mcp_client_handler.rs,types/sessions.rs}` shows only cosmetic namespace path-renames (`agent_client_protocol::X` → `agent_client_protocol::schema::X`). The test prompt is byte-identical to the version on `mcp`. Same model + same prompt + unchanged code path = test would fail on `mcp` too. Out of scope for an ACP-migration PR.
- **Test correctly ignored under default invocation.** `cargo test ...` reports `1 ignored; 0 failed` with the documented reason string surfaced.
- **Docstring is robust.** 45-line explanation of why, what's proven elsewhere, what was tried, what would unblock removal, with a kanban task cross-reference.

### Nits

- [x] `llama-agent/tests/integration/tool_use_multi_turn.rs:336-340` — Wording "Multiple prompt-tightening attempts (imperative step-1 framing, `/no_think` directive to disable thinking mode) did not recover the call" implies more exhaustive search than the documented two attempts. Consider rephrasing as "Two prompt-tightening attempts ... did not recover the call" so the docstring matches the recorded effort. **(Fixed: `s/Multiple/Two/` applied to the docstring.)**
