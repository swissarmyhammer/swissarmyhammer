---
assignees:
- claude-code
position_column: todo
position_ordinal: 8a80
title: Remove dead ClaudeAgentServer and converge on a single ACP dispatch path
---
There are three ACP-server implementations in the tree; one is dead and buggy.

## Current state
- `llama_agent::acp::AcpServer` — llama serves itself; the live `sah agent acp` binary runs this (`apps/swissarmyhammer-cli/src/commands/agent/acp.rs:76`).
- `claude_agent::ClaudeAgentServer` (`crates/claude-agent/src/server.rs`) — DEAD: `start_with_streams` is called only from its own `#[cfg(test)]` tests. Doc comments still describe ACP 0.10. It also has bugs the live paths do not: it serializes every error with a hardcoded `-32603` (discarding the handler's real code), and routes unknown methods to `ext_method`, so an unknown method gets a SUCCESS "Extension method not implemented" response instead of `method_not_found`.
- `swissarmyhammer-agent::dispatch_claude_request` — the live claude path; wraps `ClaudeAgent` with the ACP SDK builder.

## Target
- Delete `crates/claude-agent/src/server.rs` (`ClaudeAgentServer`), its `pub use server::ClaudeAgentServer` in `lib.rs`, and its tests. Confirm nothing outside its own tests references it.
- Both agents then reach handlers exclusively through the ACP SDK builder (`on_receive_request` / `on_receive_notification`).
- Evaluate consolidating the SDK-builder dispatch wiring — `swissarmyhammer-agent::dispatch_claude_request` (claude) and `AcpServer::dispatch_client_request` (llama) are the same `ClientRequest`/`ClientNotification` demux — into one shared helper in `agent-client-protocol-extras`. If full consolidation is disproportionate, the two must at minimum stay behaviorally identical (same unknown-method handling, same error-code fidelity).

## Verify
- claude-agent and llama-agent build and serve with no `ClaudeAgentServer`.
- An unknown method returns `method_not_found` from both agents.