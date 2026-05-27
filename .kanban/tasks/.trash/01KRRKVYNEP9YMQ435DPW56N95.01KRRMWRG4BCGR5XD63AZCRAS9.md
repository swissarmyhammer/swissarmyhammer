---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8480'
project: ai-panel
title: Implement the ACP Client role (Client::builder + handlers + capabilities)
---
## What
Make the Tauri backend a proper ACP `Client`. This task builds the Client surface only — the session lifecycle and prompt streaming come in later tasks.

- Add `swissarmyhammer-agent` and `agent-client-protocol` to `apps/kanban-app/Cargo.toml` (workspace deps).
- Create `apps/kanban-app/src/ai/client.rs`. Build the ACP `Client` via `agent_client_protocol::Client::builder()`, registering a real handler for EVERY client-side method:
  - `fs/read_text_file`, `fs/write_text_file` — implemented, return the ACP "capability not supported" error for v1.
  - `terminal/*` — implemented, return "capability not supported".
  - `session/request_permission` — wired to a channel/callback (consumed by a later task).
  - `session/update` notification receipt — wired to a channel/callback (consumed by a later task).
  - Any remaining `Client`-role methods — implemented, never `unimplemented!`.
- Send honest `ClientCapabilities` at `initialize`: `fs` = false, `terminal` = false, streaming/updates = true.
- Expose a constructor that, given an agent component (`DynConnectTo<Client>` from `swissarmyhammer_agent::create_agent`) plus the permission/notification callbacks, runs `Client::builder()...connect_with(...)` and returns the `ConnectionTo<Agent>`.

Hard rule (spec): the ONLY channel to the agent is this ACP connection. Do NOT use `swissarmyhammer_agent::execute_prompt` or read `AcpAgentHandle::notification_rx` — those are the broadcast side channel.

Spec: `ideas/kanban/ai_panel.md` — Phase 2 "Building the Client", "Client capabilities". Reference: `crates/swissarmyhammer-agent/src/lib.rs` (documents the `Client::builder().connect_with(handle.agent, ...)` path).

## Acceptance Criteria
- [ ] `apps/kanban-app/src/ai/client.rs` builds an ACP `Client` with a handler registered for every client-side method — none stubbed with `unimplemented!`.
- [ ] `ClientCapabilities` reports `fs` = false and `terminal` = false.
- [ ] `fs/*` and `terminal/*` handlers return the ACP capability-not-supported error.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Unit test: the constructed `ClientCapabilities` has `fs` = false, `terminal` = false.
- [ ] Unit test: calling the `fs/read_text_file` and `terminal` handlers returns the capability-not-supported ACP error.
- [ ] Integration test: connect the Client to an in-process test ACP Agent component (or a `claude-agent` fixture) and complete `initialize`, asserting the negotiated capabilities.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the capability and handler tests first.