---
assignees:
- claude-code
position_column: todo
position_ordinal: 8a80
project: ai-panel
title: 'claude-agent: forward Claude CLI elicitation to the ACP client (session/elicitation)'
---
#elicitation

## Context / Why
The kanban app's Claude backend runs through `claude-agent`, which wraps the Claude Code CLI over stream-json stdio and is the ACP **Agent** to the webview. When an MCP server (the per-board SAH server) issues an elicitation, the CLI surfaces it, but `claude-agent` has NO code path that turns that into an ACP `session/elicitation` request to the webview — so the request dies in the wrapper. The user confirmed: "claude has elicitation now, but you are going to need to actually support it with the agent wrapper." This task adds that bridge.

The proven analog already in the codebase is the permission round-trip: `crates/claude-agent/src/agent_prompt_handling.rs` builds a `RequestPermissionRequest` and sends it to the client with `client.send_request(acp_request).block_task().await` over a stored `ConnectionTo<agent_client_protocol::Client>` (see `request_user_permission` ~`agent_prompt_handling.rs:992-1056`). Mirror that mechanism for elicitation.

ACP Rust elicitation types live behind the `unstable_elicitation` cargo feature in `agent-client-protocol` 0.11 (`schema::ElicitationRequest`/`session/elicitation`, `ElicitationResponse`, `ElicitationAction::{Accept,Decline,Cancel}`, `ElicitationMode::{Form,Url}`). Enabling that feature for claude-agent is part of this task.

## What
- [ ] **Investigate first (subtask):** Determine exactly how the CLI surfaces an elicitation to claude-agent — a stream-json `control_request` subtype parsed in `crates/claude-agent/src/claude_process.rs` / `protocol_translator.rs`, or via the in-process MCP manager in `crates/claude-agent/src/mcp.rs`. Document the message shape in the task comments.
- [ ] Enable the `unstable_elicitation` feature on the `agent-client-protocol` dependency for `crates/claude-agent` (and any extras crate that re-exports the schema).
- [ ] On receiving a CLI elicitation, build a session-scoped ACP `ElicitationRequest` (set `tool_call_id` when the CLI provides one; carry `message` + `requested_schema`) and send it to the webview via the stored client connection using the `send_request(...).block_task()` pattern.
- [ ] Map the ACP `ElicitationResponse` (Accept{content}/Decline/Cancel) back into the control_response the CLI expects.
- [ ] Store/honor the client's `elicitation` capability from `initialize` (received from the webview's `ClientCapabilities`); if the client did not advertise it, respond/handle gracefully (decline) rather than hanging.

## Acceptance Criteria
- [ ] A CLI-surfaced elicitation results in exactly one ACP `session/elicitation` request to the client.
- [ ] The client's accept/decline/cancel response is relayed back to the CLI as the correct control_response.
- [ ] No regression to the existing permission round-trip.

## Tests (`crates/claude-agent/...`)
- [ ] Unit/integration test with a fake ACP client (see `crates/claude-agent/tests/common/test_client.rs`) asserting: a simulated CLI elicitation triggers a `session/elicitation` request, and a fake Accept-with-content response is mapped back correctly; plus decline and cancel cases.
- [ ] Run: `cargo nextest run -p claude-agent` — all green.

## Workflow
- Use `/tdd`. Investigate the CLI message shape, write the failing fake-client test, then implement the bridge.