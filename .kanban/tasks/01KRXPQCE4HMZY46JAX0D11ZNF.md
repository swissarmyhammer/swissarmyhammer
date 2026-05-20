---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff880
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

## Resolution (implemented)

### Deleted
- `crates/claude-agent/src/server.rs` — the entire `ClaudeAgentServer` + `JsonRpcNotification` + `ConnectionManager`/`ConnectionInfo` module, including all of its `#[cfg(test)]` tests.
- `crates/claude-agent/src/lib.rs` — removed `pub mod server;` and `pub use server::ClaudeAgentServer;`.
- `crates/claude-agent/tests/integration/integrations.rs` — was 100% `ClaudeAgentServer::new` smoke tests of the dead code; deleted and removed `mod integrations;` from `tests/integration/mod.rs`.
- Fixed stale doc reference in `crates/acp-conformance/README.md` (architecture box listed `claude-agent::ClaudeAgentServer`; now `claude-agent::ClaudeAgent` served via `swissarmyhammer-agent`).

Confirmed nothing outside `server.rs`'s own tests referenced `ClaudeAgentServer` / `start_with_streams` / `ConnectionManager` before deleting. The `start_with_streams` calls in `apps/swissarmyhammer-cli/.../agent/acp.rs` and `llama-agent` are on llama's own `AcpServer`, unrelated.

### Dispatch-consolidation decision: NOT consolidated into a shared helper (deliberate, proportionate)
Surviving SDK-builder dispatchers: `dispatch_claude_request`/`dispatch_claude_notification` and `dispatch_llama_request`/`dispatch_llama_notification` (both in `swissarmyhammer-agent/src/lib.rs`), plus `AcpServer::dispatch_client_request`/`dispatch_client_notification` in `llama-agent/src/acp/server.rs`.

A shared generic helper in `agent-client-protocol-extras` was evaluated and rejected as disproportionate:
1. The dispatch targets are unrelated concrete types (`claude_agent::ClaudeAgent`, `llama_agent::AcpServer`) with no shared trait. Unifying them needs an invented ~8-method `AcpDispatchTarget` trait plus impls in two crates — more concepts than the `match`-arm skeleton it removes.
2. The variant sets differ: llama's `AcpServer` handles `ResumeSessionRequest` and `ListSessionsRequest`; `ClaudeAgent` does not. A shared helper would keep per-agent branching anyway.

Instead, verified the surviving dispatchers are behaviorally identical, which was the stated minimum bar:
- Unknown-method handling: all three return `Error::method_not_found()` via `respond_with_error` for unmodeled `ClientRequest` variants (and `tracing::debug!`-log unmodeled notifications). The dead `ClaudeAgentServer` was the ONLY one that routed unknowns to `ext_method` — deleting it removes that divergence.
- Error-code fidelity: all three use `responder.respond_with_result(...)`, which forwards the handler's real `agent_client_protocol::Error` code. The dead `ClaudeAgentServer` was the ONLY one that flattened every error to a hardcoded `-32603` — deleting it removes that divergence too.

So deleting the dead server is exactly what made the live paths consistent; no further code change was needed for behavioral parity.

### Verification
- `cargo build -p claude-agent` — clean.
- `cargo clippy -p claude-agent --all-targets` — zero warnings.
- `cargo nextest run -p claude-agent` — 304 tests run, 304 passed, 0 failed. (Note: `crates/claude-agent/Cargo.toml` has a pre-existing `[lib] test = false`, out of scope; nextest runs the integration binaries.)
- `cargo build -p swissarmyhammer-agent -p swissarmyhammer-cli` — clean (the live `sah agent acp` path compiles with no `ClaudeAgentServer`).