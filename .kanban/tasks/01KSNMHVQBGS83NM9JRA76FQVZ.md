---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
project: ai-panel
title: 'E2E test: prove llama-agent works through the full kanban-app stack (qwen-0.6b-test)'
---
## Why

The ai-panel project shipped tasks 1–4 with green "all tests pass" reports, but **two bugs only surfaced under manual testing**:

1. `update.board` was an unknown command (the picker dispatch silently failed; model never persisted). Tests were mock-based at the dispatch boundary.
2. Qwen-27B returned 0 tokens (the model loaded but generated nothing — the actual chat path is broken). No test exercised real generation.

Mocked unit tests at the dispatch boundary + unit tests at the operation layer aren't enough. We need a **real-path smoke test** that drives an actual llama-agent model through the production code paths and asserts the user-visible contract: a non-empty response.

The user explicitly flagged this gap: *"i want my manual testing to be more quality focused and not the lame works-at-all you just subjected me to"*.

## What

Build an integration test in `apps/kanban-app/tests/` (or wherever Tauri-side integration tests live — check the existing test layout) that drives the **production code path** from `ai_start_agent` through to a returned response:

1. **Use `qwen-0.6b-test.yaml`** — the small qwen model that's already in `builtin/models/`. The 27B model is impractical for CI (downloads ~14 GB, runs slow on every developer machine). 0.6B downloads in seconds and generates in milliseconds.
2. **Wire it through the real `ai_list_models` / `ai_start_agent` / `resolve_model_config`** — call the same Tauri commands the webview calls. Not a mock, not a unit test of `llama_agent::create_agent`.
3. **Send a real `session/prompt`** over ACP (or the in-process equivalent) and assert `tokens_generated > 0`.
4. **Send a second prompt in the same session** and assert it doesn't fail with "Queue is full" — the queue-lifecycle regression from bug `01KSNJ7CBK9333J0T9G4TCA7DH`.
5. **Send a prompt that should call the kanban MCP tool** (e.g. ask the model to count tasks) and assert the tool was invoked at least once. This guards the MCP wiring path — that the board's `mcpUrl` makes it into `newSession.mcpServers` and the tool is reachable from the model.

If `qwen-0.6b-test.yaml` isn't currently tagged `kanban`, that's fine — the test doesn't need the tag to drive `ai_start_agent` directly with the model id. Do NOT add the `kanban` tag to `qwen-0.6b-test.yaml` (it would clutter the user-facing picker).

### What this test must NOT do

- It must NOT use mocks for `useDispatchCommand`, `create_agent`, `ai_start_agent`, or any other boundary. The whole point is to drive the production path.
- It must NOT just assert "no error was thrown". 0 tokens generated is a successful API call with a broken result. Assert on the **content**: token count, response non-empty, tool calls made.
- It must NOT be skipped on CI behind an env var or a flaky-test tag. If it's flaky, fix the underlying flake. A test that's not run is the same as no test.

### Performance budget

The user's session showed Qwen3.6-27B taking 2:02 from prompt to (empty) response. With qwen-0.6b-test the round-trip should be seconds. Hard cap: **30s** for the full test (including first-time model download). If it exceeds that, investigate — that's a signal something is wrong with cold-start.

### File layout suggestion

- `apps/kanban-app/tests/ai_panel_e2e.rs` — the test (or extend an existing integration test file if there's already one for kanban-app).
- Use the `#[tokio::test]` attribute and the existing TempDir + KanbanContext setup pattern from `tests/per_board_model_isolation.rs`.

## Acceptance Criteria

- [ ] A new integration test exists in `apps/kanban-app/tests/` that drives `ai_start_agent` with the literal id `qwen-0.6b-test` (or whatever id `qwen-0.6b-test.yaml` declares) and ends up with a non-empty model response.
- [ ] The test asserts `tokens_generated > 0` (NOT just "no error" — that's the failure mode bug `01KSNJ7CBK9333J0T9G4TCA7DH` would still pass).
- [ ] The test asserts the queue can accept a second prompt after the first completes (queue-lifecycle guard).
- [ ] The test asserts the kanban MCP tool was reachable — easiest version: assert the tool was advertised in the session (≥1 tool from the `kanban` MCP server). Stretch: actually make the model call a kanban tool and assert the tool result was returned.
- [ ] If this test had existed before bug `01KSNJ6AE18EQYDC2WSYFSSAY1` and bug `01KSNJ7CBK9333J0T9G4TCA7DH` were introduced, it would have failed.
- [ ] Runs under 30 seconds on a developer machine (post first-time model download).
- [ ] Runs in CI (no env-var gate, no `#[ignore]`).

## Tests

- [ ] `test_ai_panel_e2e_qwen_small_model_generates_tokens` — the core test above.
- [ ] `test_ai_panel_e2e_second_prompt_queues_after_first_completes` — queue-lifecycle guard.
- [ ] `test_ai_panel_e2e_mcp_tool_visible_in_session` — MCP wiring guard.
- [ ] Run: `cargo test -p kanban-app --test ai_panel_e2e`.

## Workflow

- Start by **reading** the production path end to end: `apps/kanban-app/src/ai/models.rs` (`ai_list_models`, `ai_start_agent`, `resolve_model_config`), `apps/kanban-app/src/ai/agent_ws.rs` (`AgentWebSocketServer::bind_with`), and the llama-agent entry points it calls. Understand exactly what runs in production before deciding which seam to test from.
- Pick the **highest seam that still exercises real code** — likely `ai_start_agent` returning an `AgentEndpoint`, then opening an ACP WebSocket client against the returned `wsUrl`, then driving real ACP `session/new` + `session/prompt`. Drive ACP from the test, not just internal Rust calls — that's the same path the webview uses.
- Use `/tdd` — write the failing assertions first.
- If the test surfaces additional bugs (e.g. discovers that `qwen-0.6b-test` itself also generates 0 tokens), document them as new tasks and tag this one stuck — do NOT silently relax the assertions to "works at all". The whole point of this card is that "works at all" is not good enough.

## Lesson this codifies

A green "all tests pass" report means nothing if no test exercises the path the user hits. Every user-visible feature deserves at least one production-path test that would have caught the kind of regression manual testing finds. Mock-heavy unit tests are a complement to those, not a replacement.

## Related

- Bug `01KSNJ6AE18EQYDC2WSYFSSAY1` (fixed in commit `07c3c6583`): `update.board` unknown command. This test would have caught it because picking a model triggers `update.board` and the test would verify it succeeded.
- Bug `01KSNJ7CBK9333J0T9G4TCA7DH` (open): qwen generates 0 tokens. This test would have caught it on the first prompt assertion.