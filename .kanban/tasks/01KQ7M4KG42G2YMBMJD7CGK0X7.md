---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8480
title: Set per-rule max_tokens cap so a runaway model doesn't lock the hook
---
**Observed runaway:** the 2026-04-27 qwen Stop-hook log showed one rule generation producing **1,560,260 tokens** with zero tool calls (`naming-consistency`, line 89-91 of the captured log). That's about 90 seconds of pure generation against a 27B model. The model couldn't ground itself (no tools), so it just kept generating until something else stopped it.

Once the tools tasks land, this exact failure mode should disappear — the model will have something to call, get a result, and converge. **But** there should still be a cap on how many tokens any single rule generation gets, as defense-in-depth: a misbehaving model, a bug in the parser, or a prompt that confuses the model shouldn't be able to lock up the entire hook indefinitely.

## What to add

`avp-common/src/validator/runner.rs::execute_rule_in_fresh_session` (or its sub-helpers — likely `send_rule_prompt_and_collect`) is where the `agent.prompt(rule_request)` call happens. The `PromptRequest` doesn't currently set `max_tokens` — it inherits the agent's default, which appears to be effectively unbounded.

Set a per-rule cap. Reasonable starting value: **4096 tokens**. Rationale:
- Typical validator response is `<think>` (a few hundred tokens) + tool calls (~50 each) + tool results (variable) + final JSON verdict (~50 tokens). 4K is comfortable headroom.
- The integration test for the Qwen3 strategy uses 1024; the validator path needs more to accommodate `<think>` blocks.
- If a rule legitimately needs more, that's a sign the rule prompt is too complex or the model is the wrong size — both of those should be addressed at the prompt/model layer, not by raising the cap.

The cap is a config knob, not a hard-coded value — exposed somewhere (maybe `.avp/avp.yaml`, maybe per-RuleSet via the manifest). For this task, hardcode the constant in `runner.rs` with a clear name like `RULE_GENERATION_MAX_TOKENS` and a comment explaining the rationale. A future task can plumb it through to config if anyone needs to tune it.

## What happens at the cap

When `max_tokens` is hit, the agent returns a response with `stop_reason: MaxTokens` (or whatever the equivalent is in ACP — verify by reading `agent_client_protocol::StopReason`). The runner should:

1. Treat this as a rule failure (not silent pass).
2. Failure message: something like `"Validator rule '{rule_name}' exceeded max generation tokens ({cap}) without producing a verdict. This usually indicates a prompt/model mismatch — file an issue with the rule body and the partial response."` Include the partial response (truncated to ~2KB) so the user has a debug trail.
3. Severity follows the rule's `effective_severity(ruleset)` — same as any other failure.

This dovetails with `01KQ35V5GTDS4ED3VWG8SAH4DQ` (unparseable→fail). Same general principle: validators that don't produce a clean verdict fail loudly, not pass silently.

## Tests

- Unit test: construct a `PromptRequest` via the runner's logic, assert `max_tokens` is set to the constant. Probably easiest to inspect what the runner produces by looking at the recorded `RecordingAgent` transcript.
- Mock test: use a `PlaybackAgent` that returns `stop_reason: MaxTokens`, run a rule, assert the rule fails with the expected message format.
- Integration test (optional): use a deliberately-misbehaving rule prompt that would generate a lot of tokens, verify the cap fires and the rule fails. Skippable if too flaky.

## Acceptance

- All `agent.prompt(...)` calls from the validator runner have `max_tokens` set to the constant.
- A response with `stop_reason: MaxTokens` is treated as a failure (not silent pass), severity follows the rule.
- Failure message references the cap value and includes a truncated partial response.
- `cargo test -p avp-common` and `cargo clippy -p avp-common --all-targets -- -D warnings` are clean.
- Manual: re-run the qwen Stop-hook test from earlier sessions; if any rule still runs away (it shouldn't post-tool-task), the cap fires and the rule fails cleanly instead of locking the hook.

## Pairs with

- `01KQ35V5GTDS4ED3VWG8SAH4DQ` (already done — unparseable→fail). Same principle, applied to a different "didn't produce a clean verdict" failure mode. Both are loud-fail policies that prevent silent green. #avp

## Review Findings (2026-04-27 11:01)

### Warnings
- [x] `avp-common/src/validator/runner.rs:413-430` — The cap is communicated via `PromptRequest._meta["max_tokens"]`, but no agent in this repo actually reads that key. `claude-agent` enforces its own `max_tokens_per_turn` from config (default 100k in `claude-agent/src/config.rs:22`) and never inspects the request's `_meta`. `llama-agent`'s ACP server hardcodes its own `MAX_GENERATION_TOKENS = 16384` at `llama-agent/src/acp/server.rs:1439` and likewise ignores `request.meta`. The ACP schema explicitly says "Implementations MUST NOT make assumptions about values at these keys" (`agent-client-protocol-schema-0.11.4/src/agent.rs:2942`). Net effect: as written, the cap is dead code at the request layer — it never causes a `MaxTokens` stop reason to fire. The runner's `MaxTokens → loud failure` mapping is correct and well-tested, but the trigger never reaches the runner from the existing agents. Either (a) wire the cap through to `claude-agent`/`llama-agent` so they read `request.meta.max_tokens` and pass it to their stoppers, or (b) document this as a future-facing contract and lower the runner-side urgency — the current PR claims "defense-in-depth against runaway generation" but does not deliver it for any agent in-tree.

  **Resolution:** hybrid — option (b) for `claude-agent` (CLI subprocess plumbing is non-trivial; tracked in follow-up `01KQ7VB868YZ7AWHNT16YB4XZR`) plus option (a) for `llama-agent` (small surgical change, ~30 lines + 7 unit tests). `llama-agent/src/acp/server.rs::extract_request_max_tokens` now reads `request.meta["max_tokens"]` and clamps the per-turn cap to `min(MAX_GENERATION_TOKENS, available_tokens, requested)`. Hitting the cap surfaces as `StopReason::MaxTokens` to the runner via the existing `map_finish_reason_to_stop_reason` path — defense-in-depth is now live for the agent where the original runaway was observed. Doc on `RULE_GENERATION_MAX_TOKENS` updated to capture the per-agent support status.

### Nits
- [x] `avp-common/src/validator/runner.rs:80` — Doc comment says "4096 leaves comfortable headroom" but the actual constant on line 92 is `16 * 1024` (16384). Update the doc to "16384" / "16k" so the rationale matches the value, or change the constant to 4096 if 16k was a mistake. Right now the next reader has to choose which to believe.

  **Resolution:** doc rewritten to match the 16k value with explicit rationale.

- [x] `avp-common/src/validator/runner.rs:92` — The task description recommends 4096 with explicit rationale ("4K is comfortable headroom for `<think>` + tool calls + verdict"). The implementation chose 16384 without a note explaining the deviation. If 16k was deliberate (e.g. matching `llama-agent/src/acp/server.rs:1439`'s hardcoded `MAX_GENERATION_TOKENS`), call that out in the doc comment so the rationale is captured. If 4096 is still the intended value, drop to that.

  **Resolution:** doc now captures both reasons for choosing 16k over 4k: (1) reasoning models like Qwen3 produce several thousand tokens of `<think>` before tool calls/verdicts, and (2) alignment with llama-agent's existing `MAX_GENERATION_TOKENS = 16384` so we never have an asymmetry where one layer caps tighter than the other.