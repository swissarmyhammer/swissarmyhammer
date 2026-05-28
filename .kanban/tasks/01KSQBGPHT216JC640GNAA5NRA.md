---
assignees:
- claude-code
depends_on:
- 01KSQBDM9M4RJJYGQDTZYJA107
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
- 01KSQBG2EW2HNHQ911SHN6G6YK
position_column: todo
position_ordinal: '8e80'
project: llama-coverage
title: Cover ACP server + agentic turn loop + session lifecycle (acp/server.rs, agent.rs, session.rs)
---
## What

The top of the agent stack: `acp/server.rs` (5.2k lines), `agent.rs` (2.4k), `session.rs` (2.2k). This is the ACP server that the kanban webview connects to, the agentic turn loop, and session management. Drive it with the `ScriptedModel` so the full loop — prompt in, turns run, tool calls dispatched, response out — is covered without a real model or the GUI.

This is the card that most directly delivers the user's goal: "using the UI from kanban should just work." If the agentic loop is fully covered with a scripted model, the only remaining variable in production is the real model's token quality.

## Cover

- **Single-turn prompt** — `session/new` then `session/prompt`: scripted model emits text → assert the ACP `session/update` notifications + final response carry that text. (This is the exact path that produced 0 tokens in production; cover the non-empty case AND the scripted-immediate-EOS case so an empty turn is reported as a clean completion, not a hang.)
- **Multi-turn agentic loop with a tool call** — scripted model emits a tool-call → loop dispatches to the (mock) MCP tool → feeds the result back → scripted model emits final text. Assert the tool was invoked and the result threaded back. (The bug log showed `0 tool calls executed`; this guards the tool path.)
- **Session lifecycle** — new / resume / concurrent sessions / max_sessions limit / session not found (the ACP session id is opaque — see memory `acp-session-id-opaque`; do NOT validate ULID format).
- **MCP wiring** — the per-session MCP server list from `newSession.mcpServers` is attached and its tools advertised (mirrors the kanban board's mcpUrl path).
- **Cancellation / abort mid-turn** — releases cleanly (ties to the queue lifecycle card).
- **Error propagation** — a generation error surfaces as a proper ACP error to the client, not a hang.

## Acceptance Criteria

- [ ] The single-turn and multi-turn-with-tool paths are covered end-to-end with `ScriptedModel`, no real model.
- [ ] An immediate-EOS scripted turn produces a clean empty completion (the 0-token shape) — asserted, not hung.
- [ ] A tool-calling turn dispatches to a mock MCP tool and threads the result back.
- [ ] Session lifecycle (new/resume/limit/not-found) covered.
- [ ] Combined region coverage of `acp/server.rs` + `agent.rs` + `session.rs` reaches the epic threshold (target >90% given size; justify exclusions for genuinely transport-bound code).

## Tests

- [ ] Extend `crates/llama-agent/tests/acp_integration.rs` / `tests/integration/` driving the ACP server with the scripted model + a mock MCP tool.
- [ ] Run: `cargo test -p llama-agent acp` and confirm the coverage delta.

## Workflow

- Use `/tdd`. Depends on the scripted-model keystone AND the ACP-translation card (so the message mapping it relies on is already pinned).
- Memory: `acp-session-id-opaque` — session ids are opaque; validity = session exists, not format.