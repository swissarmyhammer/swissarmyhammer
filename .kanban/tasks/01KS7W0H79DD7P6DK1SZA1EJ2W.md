---
assignees:
- claude-code
depends_on:
- 01KS865E8GE912VE7KSW836N8V
position_column: todo
position_ordinal: 8a80
project: ai-panel
title: 'claude-agent: forward Claude CLI elicitation to the ACP client (elicitation/create)'
---
#elicitation

## Context / Why
The kanban app's Claude backend runs through `claude-agent`, which wraps the Claude Code CLI over stream-json stdio and is the ACP **Agent** to the webview. When the per-board SAH MCP server issues an elicitation, the CLI surfaces it, but `claude-agent` has NO code path that turns that into an ACP `elicitation/create` request to the webview — so the request dies in the wrapper. The user confirmed: "claude has elicitation now, but you are going to need to actually support it with the agent wrapper." This task adds that bridge.

The proven analog already in the codebase is the permission round-trip: `request_user_permission` in `crates/claude-agent/src/agent_prompt_handling.rs` builds a `RequestPermissionRequest` and sends it to the client with `client.send_request(acp_request).block_task().await` over a stored `ConnectionTo<agent_client_protocol::Client>`. Mirror that exact mechanism for elicitation.

## Dependency
Requires the ACP elicitation types to be available — depends on "Enable ACP elicitation types in the workspace". Use `agent_client_protocol::schema::{CreateElicitationRequest, CreateElicitationResponse, ElicitationAction}` (do NOT add feature flags here; the enablement task handles that). Method name is `elicitation/create`, matching the webview client.

## What
- [ ] **Investigate first (subtask):** Determine exactly how the CLI surfaces an elicitation to claude-agent — a stream-json `control_request` subtype parsed in `crates/claude-agent/src/claude_process.rs` / `protocol_translator.rs`, or via the in-process MCP manager in `crates/claude-agent/src/mcp.rs`. Note: permissions are NOT relayed from the CLI (it runs with `--dangerously-skip-permissions`; claude-agent's own policy engine handles them), so the elicitation receive-path is genuinely new — document the message shape you find in the task comments.
- [ ] On receiving a CLI elicitation, build a `CreateElicitationRequest` (form mode; set `tool_call_id` when the CLI provides one; carry `message` + `requested_schema`) and send it to the webview via the stored client connection using the `send_request(...).block_task()` pattern.
- [ ] Map the `CreateElicitationResponse` (`ElicitationAction` accept{content}/decline/cancel) back into the control_response the CLI expects.
- [ ] Store/honor the client's `elicitation` capability from `initialize` (the webview's `ClientCapabilities`); if not advertised, handle gracefully (decline) rather than hanging.

## Acceptance Criteria
- [ ] A CLI-surfaced elicitation results in exactly one ACP `elicitation/create` request to the client.
- [ ] The client's accept/decline/cancel response is relayed back to the CLI as the correct control_response.
- [ ] No regression to the existing permission round-trip.

## Tests (`crates/claude-agent/...`)
- [ ] Unit/integration test with a fake ACP client (see `crates/claude-agent/tests/common/test_client.rs`) asserting: a simulated CLI elicitation triggers an `elicitation/create` request, and a fake accept-with-content response is mapped back correctly; plus decline and cancel cases.
- [ ] Run: `cargo nextest run -p claude-agent` — all green.

## Workflow
- Use `/tdd`. Investigate the CLI message shape, write the failing fake-client test, then implement the bridge.