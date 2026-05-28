---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffbd80
project: llama-coverage
title: Cover agent.rs AgentServer::generate path (tool retry, parallel dispatch, title-via-model, auto-compact)
---
## DONE (2026-05-28) — with an honest note on the combined-90% criterion

Lifted `agent.rs` 50.12% → **57.89%** and covered the named generate-path items that are *deterministically* testable. One acceptance criterion (>90% combined) is arithmetically unreachable from this card's scope; explained below.

### What landed
- **`is_tool_error_retriable` — exhaustive pure unit tests** (agent.rs test module, no model). This is the model-free decision at the heart of `execute_tool_with_retry`: 5xx/network → retriable; 429/4xx/validation/unknown → fail-fast; case-insensitive. 7 tests pinning every branch. The retry *loop* (backoff timing, attempt count) is driven by the existing real-model tool tests; this locks the classification it keys on.
- **`generate_session_title` / `title_via_model` success branch** — new real-model test `tests/integration/agent_generate_path.rs::test_generate_session_title_success_branch`. Drives render-title-prompt → create-context → `generate_text_with_borrowed_model` → `normalize_title` → `Ok(Some(title))`. Side effect: this exercise of the batch generation loop lifted `generation/mod.rs` 39% → **61.23%**.
- **Empty-message guard** — `test_generate_session_title_empty_message_returns_none` pins the early-return arm (no model call) the success test steps over.

### Coverage (crate-scoped, baseline methodology)
- Crate total **85.04% line** (20575/24194), up from 78.01% baseline (and 84.29% after the generation-core card).
- `agent.rs`: 50.12% → **57.89%** (528/912).
- Full suite green: 941 lib + 98 real-model + 225 + 19 tests, 0 failures.

### Acceptance criteria
- [x] `AgentServer::generate` single-turn + tool-loop paths covered end-to-end — by the pre-existing `tool_call_round_trip*` / `tool_use_multi_turn` real-model tests (confirmed present).
- [~] Tool retry path: the retry **decision** (`is_tool_error_retriable`) is now exhaustively covered; the fail→retry→succeed **loop** is not driven end-to-end (see constraint below).
- [~] Parallel tool dispatch (>1 tool call in a turn): NOT covered — see constraint below.
- [x] Title-via-model success branch covered.
- [ ] **>90% combined `acp/server.rs` + `agent.rs` + `session.rs`: NOT met, and unreachable from this card.** Current combined = (2098+528+1314)/(2723+912+1475) = **77.1%**. Even at *100%* of agent.rs the ceiling is (2098+912+1314)/5110 = **84.6%** — the 625 uncovered lines in `acp/server.rs` dominate and are a different API surface (the ACP card 01KSQBGPHT216JC640GNAA5NRA's territory, already at 77%). This criterion conflates two surfaces; it should be rescoped to acp/server.rs or dropped.

### Why the parallel-dispatch / retry-loop paths are not driven (constraint, not omission)
`AgentServer::generate` extracts tool calls from the *model's* generated text (`chat_template.extract_tool_calls`). Deterministically hitting `execute_tools_parallel` needs the model to emit ≥2 tool calls in one turn, and hitting the retry loop needs a flaky tool failing-then-succeeding — both depend on the tiny qwen-0.6B reliably emitting structured tool calls on demand, which it does not. The `ScriptedModel` test double implements `TextGenerator`, but `AgentServer` drives generation through `ModelManager::with_model`, not `TextGenerator`, so it cannot inject scripted output into `generate()` without an AgentServer-level generator-injection refactor (out of scope). Per the real-path-tests-not-mocks rule, a flaky model-dependent test is worse than an honest gap — filed as follow-up **01KSQY663QR1RYHVKHRRQDF78V** rather than shipping a flaky "works-at-all" test.