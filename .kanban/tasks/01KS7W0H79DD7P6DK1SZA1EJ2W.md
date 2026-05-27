---
assignees:
- claude-code
depends_on:
- 01KS865E8GE912VE7KSW836N8V
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff9d80
project: ai-panel
title: 'claude-agent: forward Claude CLI elicitation to the ACP client (elicitation/create)'
---
#elicitation

## Context / Why
The kanban app's Claude backend runs through `claude-agent`, which wraps the Claude Code CLI over stream-json stdio and is the ACP **Agent** to the webview. When the per-board SAH MCP server issues an elicitation, the CLI surfaces it, but `claude-agent` has NO code path that turns that into an ACP `elicitation/create` request to the webview — so the request dies in the wrapper. The user confirmed: "claude has elicitation now, but you are going to need to actually support it with the agent wrapper." This task adds that bridge.

The proven analog already in the codebase is the permission round-trip: `request_user_permission` in `crates/claude-agent/src/agent_prompt_handling.rs` builds a `RequestPermissionRequest` and sends it to the client with `client.send_request(acp_request).block_task().await` over a stored `ConnectionTo<agent_client_protocol::Client>`. Mirror that exact mechanism for elicitation.

## Dependency
Requires the ACP elicitation types to be available — depends on "Enable ACP elicitation types in the workspace". Use `agent_client_protocol::schema::{CreateElicitationRequest, CreateElicitationResponse, ElicitationAction}` (do NOT add feature flags here; the enablement task handles that). Method name is `elicitation/create`, matching the webview client.

## Investigation Findings (2026-05-22) — CLI elicitation wire shape
Verified against the actual installed CLI binary (`@anthropic-ai/claude-code`, Mach-O arm64) by extracting embedded zod schema + handler strings. There is NO recorded elicitation in the transcript (only init/success), so the shape was derived from the binary, which is authoritative.

RECEIVE (CLI -> claude-agent, on stdout): a JSON-RPC control envelope
```
{"type":"control_request","request_id":"<id>","request":{
  "subtype":"elicitation",
  "mcp_server_name":"<server>",
  "message":"<human prompt>",
  "mode":"form"|"url",                 // optional; treat absent as form
  "url":"<string>",                    // optional (url mode)
  "elicitation_id":"<string>",         // optional
  "requested_schema":{...JSON schema...}, // optional (snake_case on the control wire)
  "title":"<string>",                  // optional
  "display_name":"<string>",           // optional
  "description":"<string>"             // optional
}}
```
Note the SDK control wire is snake_case (`requested_schema`, `elicitation_id`, `mcp_server_name`), whereas the ACP `elicitation/create` method is camelCase (`requestedSchema`, `elicitationId`). The bridge translates snake_case CLI -> camelCase ACP types and back. (Confirmed in binary: `H.request.subtype===\"elicitation\"` handler reads exactly these fields; the SDK consumer response zod schema `_84` is `{action: enum[\"accept\",\"decline\",\"cancel\"], content?: record}` \"Response from the SDK consumer for an elicitation request.\")

RESPOND (claude-agent -> CLI, on stdin): the standard control_response envelope the CLI's bridge parses (`type===\"control_response\" && \"response\" in H`):
```
{"type":"control_response","response":{
  "subtype":"success",
  "request_id":"<echoed id>",
  "response":{"action":"accept"|"decline"|"cancel","content":{...}}  // content optional, only on accept
}}
```
On failure use `{"subtype":"error","request_id":"<id>","error":"<msg>"}`. (Confirmed in binary: every control_response is `{type:\"control_response\",response:{subtype:\"success\",request_id:H.request_id,response:{...}}}`.)

Permissions are NOT relayed from the CLI (it runs `--dangerously-skip-permissions`; claude-agent's policy engine handles them), so this receive-path is genuinely new — there was no `control_request` handling anywhere in claude-agent before this task.

## What
- [x] **Investigate first (subtask):** Determined how the CLI surfaces an elicitation to claude-agent: a stream-json `control_request` with `request.subtype == \"elicitation\"` on the CLI's stdout (the same channel `claude.rs::run_stream_loop` already reads). Documented the exact message shape above (RECEIVE/RESPOND envelopes), derived from the installed CLI binary's embedded zod schemas and `handleElicitation`/control-response builders.
- [x] On receiving a CLI elicitation, build a `CreateElicitationRequest` (form mode; set `tool_call_id` when the CLI provides one; carry `message` + `requested_schema`) and send it to the webview via the stored client connection using the `send_request(...).block_task()` pattern.
- [x] Map the `CreateElicitationResponse` (`ElicitationAction` accept{content}/decline/cancel) back into the control_response the CLI expects.
- [x] Store/honor the client's `elicitation` capability from `initialize` (the webview's `ClientCapabilities`); if not advertised, handle gracefully (decline) rather than hanging.

## Acceptance Criteria
- [x] A CLI-surfaced elicitation results in exactly one ACP `elicitation/create` request to the client.
- [x] The client's accept/decline/cancel response is relayed back to the CLI as the correct control_response.
- [x] No regression to the existing permission round-trip.

## Tests (`crates/claude-agent/...`)
- [x] Unit/integration test with a fake ACP client (see `crates/claude-agent/tests/common/test_client.rs`) asserting: a simulated CLI elicitation triggers an `elicitation/create` request, and a fake accept-with-content response is mapped back correctly; plus decline and cancel cases.
- [x] Run: `cargo nextest run -p claude-agent` — all green.

## Workflow
- Use `/tdd`. Investigate the CLI message shape, write the failing fake-client test, then implement the bridge.

## Review Findings (2026-05-22 17:40)

The implementation is clean, complete, and faithful to the investigation findings. All three acceptance criteria are met and verified: exactly one `elicitation/create` request is sent (asserted in `accept_with_content_round_trips_back_to_cli_response`), accept/decline/cancel map back correctly, and the permission round-trip is untouched (the elicitation handler mirrors its `read().await` + `block_task().await` pattern exactly). `cargo nextest run -p claude-agent` passes (1046 tests, 23 elicitation-specific); `cargo clippy -p claude-agent --all-targets` is clean. The three scrutiny points all resolve favorably: (1) the wire method `elicitation/create` and camelCase params (`sessionId`, `mode`, `message`, `requestedSchema`) are confirmed by the in-process round-trip test and match the TS `unstable_createElicitation` handler; (2) `CliElicitationRequest::parse` is defensive against malformed JSON, wrong type/subtype, and missing fields; (3) no lock nesting and no deadlock risk — `block_task()` runs on the spawned `run_stream_loop` task, not an `on_receive_request` callback, and the capabilities guard is released before the client guard is taken.

### Warnings
- [x] `crates/claude-agent/src/agent_elicitation.rs` — A genuine transport failure relaying the elicitation to the client is mapped to `cancel()`, which `elicitation_response_for_line` then wraps in a `success` control_response (`subtype: \"success\"`, `action: \"cancel\"`). The CLI therefore sees an infrastructure failure as a clean user cancellation, masking the error. RESOLVED: introduced `ElicitationOutcome { Responded(CreateElicitationResponse), Error(String) }` as the `ElicitationHandler` return type. Transport failures, plus encode/decode failures, now return `ElicitationOutcome::Error`, which `CliElicitationRequest::control_response_for_outcome` routes through the existing `error_control_response` helper (the `subtype:"error"` envelope). A real ACP Cancel *action* from the client still maps to `Responded(cancel)` → success envelope. The stream loop (`claude.rs::elicitation_response_for_line`) now calls `control_response_for_outcome` so the error envelope is genuinely wired in. New test `elicitation_response_for_line_emits_error_envelope_on_handler_error` (unit) and `transport_failure_maps_to_cli_error_envelope_not_cancel` (in-process integration, fake client returns a JSON-RPC error) cover the transport-error path; the existing accept/decline/cancel tests were updated for the new return type and still pass.

### Nits
- [x] `crates/claude-agent/src/elicitation_bridge.rs` — `mode` is parsed and stored but never consulted in `to_acp_request`, which always builds form mode. A CLI `url`-mode elicitation would be silently translated into an empty-schema form request. RESOLVED: added `CliElicitationRequest::is_form_mode()` and the bridge handler now declines any non-form (e.g. `url`) elicitation up front — before consulting capabilities or the client — so the CLI is unblocked with a clear non-answer (a `success`/`decline` envelope) rather than presenting an empty form. New test `declines_url_mode_elicitation` covers this. Full url-mode rendering remains out of scope (the task is form-mode scoped); declining is the honest fallback.