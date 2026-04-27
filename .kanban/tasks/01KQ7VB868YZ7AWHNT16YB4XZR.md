---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffff8680
title: Wire request.meta.max_tokens through claude-agent so the validator runner's per-rule cap fires
---
## Background

The validator runner in `avp-common/src/validator/runner.rs` attaches a `max_tokens` cap (currently `RULE_GENERATION_MAX_TOKENS = 16 * 1024`) to every per-rule `PromptRequest` via the ACP `_meta` map. The runner already knows how to convert a `StopReason::MaxTokens` response into a loud rule failure (`build_rule_outcome_from_response` + `build_max_tokens_failure_message`).

`llama-agent` honors this cap — `llama-agent/src/acp/server.rs::extract_request_max_tokens` reads it and clamps the per-turn `max_tokens` it passes to `GenerationRequest`. When a runaway generation hits the cap, llama returns "Maximum tokens reached" → `StopReason::MaxTokens` and the runner produces a loud failure.

`claude-agent` does **not** honor the cap. It enforces its own `max_tokens_per_turn` config (default 100k) in `claude-agent/src/agent.rs::check_turn_limits`, but that limit is computed from cumulative *input* token estimates, not generation tokens. The `_meta` map is never consulted in `agent_trait_impl.rs::prompt`. As a result, when the runner runs against claude-agent, the per-rule defense-in-depth cap is inert.

The ACP spec lets agents ignore unknown `_meta` keys, so honoring this is a deliberate opt-in. We want to opt in for claude-agent for symmetry with llama-agent.

## What to do

`claude-agent` shells out to the `claude` CLI subprocess (see `claude-agent/src/claude_process.rs`). The current invocation does not pass any equivalent of `--max-tokens`. Two implementation paths to evaluate:

1. **CLI flag.** If the `claude` CLI accepts a per-turn generation cap (verify against current docs), pass `request.meta["max_tokens"]` as a CLI flag from `prompt()` down through the subprocess command construction.

2. **Streaming token counter.** If the CLI does not accept a generation cap, count streaming output tokens in claude-agent itself and abort the subprocess + return `PromptResponse::new(StopReason::MaxTokens)` when the count exceeds the requested cap. This mirrors how llama-agent works: cap is enforced at the agent layer, not the upstream model layer.

Either way:

- Add a free helper `extract_request_max_tokens` (mirroring the one in `llama-agent/src/acp/server.rs`) to keep the JSON inspection unit-testable.
- Honor `request.meta["max_tokens"]` only as a *tighter* cap — never raise above the agent's existing `max_tokens_per_turn` config. Caller can only narrow, not widen.
- Map the trigger to `StopReason::MaxTokens` in the response so the runner's existing failure-mapping path fires unchanged.

## Acceptance

- A `PromptRequest` with `request.meta["max_tokens"] = N` causes claude-agent to stop generation at N output tokens (or the existing per-turn cap, whichever is smaller) and return `StopReason::MaxTokens`.
- Unit test for `extract_request_max_tokens` covering: missing meta, missing key, positive integer, zero, negative, non-integer (string/float/bool).
- Integration test (likely with a stubbed/mocked subprocess) verifying that a streaming generation hitting the cap surfaces as `StopReason::MaxTokens` from `prompt()`.
- The doc on `RULE_GENERATION_MAX_TOKENS` in `avp-common/src/validator/runner.rs` updates to note that claude-agent now honors the cap.
- `cargo test -p claude-agent` and `cargo clippy -p claude-agent --all-targets -- -D warnings` are clean.

## Why now / why later

This is a follow-up to `01KQ7M4KG42G2YMBMJD7CGK0X7`. That task wired the cap through llama-agent (where the original runaway was observed) and documented the inertness for claude-agent. This task closes the asymmetry. #avp

## Review Findings (2026-04-27 12:18)

### Warnings
- [x] `claude-agent/src/agent_prompt_handling.rs:399` — `build_max_tokens_streaming_response` hardcodes `meta.streaming = true` but is also invoked from the non-streaming path at line 1117 (`handle_non_streaming_prompt`). When the cap fires from the non-streaming path, the response meta will misleadingly report `streaming: true`. The non-streaming success path (line 1181) sets `streaming: false` correctly, so a max-tokens failure is the only outcome with the wrong value. Fix by parameterizing `streaming` (e.g. `fn build_max_tokens_response(&self, …, streaming: bool)`) or by removing the `streaming` field from this specific response (it duplicates info already implicit in the `MaxTokens` stop reason).

### Nits
- [x] `claude-agent/src/agent_prompt_handling.rs:353,391` — Method names `abort_streaming_for_max_tokens` and `build_max_tokens_streaming_response` carry a "streaming" prefix even though both are called from the non-streaming path (`handle_non_streaming_prompt`, lines 1115 and 1117). Consider renaming to `abort_for_max_tokens` and `build_max_tokens_response` to reflect that they are path-agnostic.
- [x] `claude-agent/src/agent_prompt_handling.rs:316-338,1101-1123` — The cap-enforcement block (token estimate + saturating add + threshold check + warn-log + abort + response build) is duplicated verbatim between `process_stream_chunks` and the loop in `handle_non_streaming_prompt`. Extract a helper such as `async fn check_output_token_cap(&self, session_id, session_id_str, output_tokens: &mut u64, chunk_content: &str, effective_cap, requested_cap) -> Option<PromptResponse>` returning `Some(response)` when the cap fires; both call sites become a single `if let Some(r) = …` line.

## Resolution (2026-04-27)

All three review items addressed in `claude-agent/src/agent_prompt_handling.rs`:

1. **Warning (streaming flag)**: `build_output_max_tokens_response` now takes a `streaming: bool` parameter and writes it to `meta.streaming` instead of hardcoding `true`. The streaming call site passes `true`; the non-streaming call site passes `false`.

2. **Nit (method names)**: Renamed `abort_streaming_for_max_tokens` → `abort_for_output_max_tokens` and `build_max_tokens_streaming_response` → `build_output_max_tokens_response`. Both names are now path-agnostic. Note: plain `build_max_tokens_response` was unavailable due to an existing same-named method in `agent.rs` (the input-token cap response), so the renamed method takes the `output_` qualifier to disambiguate input-cap vs output-cap responses.

3. **Nit (duplication)**: Extracted `check_output_token_cap(&self, session_id, session_id_str, &mut output_tokens, chunk_content, OutputCap)` helper. Both call sites (in `process_stream_chunks` and `handle_non_streaming_prompt`) now reduce to a single `if let Some(r) = … { return Ok(r); }`. The cap parameters were bundled into a small `Copy` struct `OutputCap { effective, caller_supplied, streaming }` to satisfy clippy's `too_many_arguments` lint and keep named fields at the call sites.

Verification:
- `cargo clippy -p claude-agent --all-targets -- -D warnings` — clean.
- `cargo nextest run -p claude-agent --lib agent_prompt_handling` — all 18 tests pass, including the streaming and non-streaming cap-firing integration tests (`test_process_stream_chunks_max_tokens_fires_on_cap`, `test_process_stream_chunks_no_cap_fire_when_under_limit`).
- Pre-existing test failures in `terminal_manager`, `path_validator`, `session`, etc. reproduce on `main` without these changes and are unrelated to the cap logic.