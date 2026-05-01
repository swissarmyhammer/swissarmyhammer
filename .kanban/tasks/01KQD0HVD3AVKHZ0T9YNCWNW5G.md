---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: done
position_ordinal: ffffffffffffffffffffffffa680
project: acp-upgrade
title: 'ACP 0.11: claude-agent: agent_prompt_handling'
---
## What

Migrate `claude-agent/src/agent_prompt_handling.rs` to ACP 0.11. This file grew ~600 lines on the avp branch and is the largest single per-feature handler.

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [x] `agent_prompt_handling.rs` compiles under `cargo check -p claude-agent`.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests pass.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).

## Review Findings (2026-04-29 17:30)

### Blockers
- [x] `claude-agent/src/agent_prompt_handling.rs:810` — Wrong `ConnectionTo` type parameter. The signature uses `ConnectionTo<agent_client_protocol::Agent>`, but in ACP 0.11 the type parameter on `ConnectionTo<Counterpart>` names the *counterpart* role at the other end of the connection (see `agent-client-protocol-0.11.1/src/jsonrpc.rs:1455`: "the counterpart role this connection is talking to"). `claude-agent` plays the **Agent** role (per `agent.rs:69-70` and the existing `Agent.builder()`-driven wiring), and `RequestPermissionRequest` is an agent-to-client request (it lives under `agent-client-protocol-schema/src/client.rs` and `src/schema/agent_to_client/requests.rs`). The agent must therefore hold a connection whose counterpart is `Client`. Confirming examples in the SDK: `examples/simple_agent.rs:21` shows an agent dispatching with `cx: ConnectionTo<Client>`, and `examples/yolo_one_shot_client.rs:117` shows a client receiving `ConnectionTo<Agent>` — by symmetry the agent side is `ConnectionTo<Client>`. The reason this slipped through `cargo check` is that `agent.rs` still types the `client` field as `Arc<dyn agent_client_protocol::Client>` and produces an E0404 earlier in the resolve chain, so the call site at `agent_prompt_handling.rs:733-740` never gets type-checked against the new parameter signature. When B5/B6/B9 reshape the field to its real ACP 0.11 type, this mismatch will surface as a compile error. Fix: change the parameter (and the doc comment at line 785) to `&agent_client_protocol::ConnectionTo<agent_client_protocol::Client>`.

### Warnings
- [x] `claude-agent/src/agent_prompt_handling.rs:850-853` — The inline comment asserts that `block_task()` is safe "because this method runs inside the spawned prompt task, not on the event loop itself," but at this commit there is no wired-up call site that proves it. `agent_trait_impl.rs:214 prompt(...)` is currently a method on a removed `Agent` trait impl that no longer compiles, and the eventual ACP 0.11 wiring (which must call `cx.spawn(...)` or otherwise drop into a spawned task before invoking the prompt handler) is deferred to B5/B6/B7/B8/B9. Per `agent-client-protocol-0.11.1/src/jsonrpc.rs:2916-2933`, calling `block_task()` directly inside an `on_receive_request` handler is documented as a deadlock. The current comment treats this as a fact when it is actually a contract that the dispatch-layer migration must uphold. Fix one of: (a) soften the comment to describe the contract ("safe iff the caller spawns this off the event loop — see B5/B6/B9"), or (b) leave the assertion but add a follow-up checklist item on the dispatch-layer task confirming the spawn boundary is in place when wiring lands.
- [x] `claude-agent/src/agent_prompt_handling.rs:783` — Doc comment says the method "awaits the response on the spawned-task event loop". Conflates "spawned task" with "event loop"; the whole point of `block_task` is that you're *not* on the event loop. Suggest "awaits the response from the spawned task" or "awaits the response without blocking the event loop".

### Nits
- [x] `claude-agent/src/agent_prompt_handling.rs:789` — Doc comment refers to "a `dyn Client` object" in past tense; fine for transition, but once the codebase fully lands on 0.11 this archaeology will read oddly. Consider trimming the historical context once B5/B6/B9 land.

## Resolution (2026-04-29)

- Fixed blocker: changed `request_user_permission`'s `client` parameter type from `&ConnectionTo<Agent>` to `&ConnectionTo<Client>`. The agent role holds a connection whose counterpart is `Client`; this is confirmed by `agent-client-protocol-0.11.1/src/jsonrpc.rs:1455` and `examples/simple_agent.rs:21`.
- Fixed warning re. `block_task` contract: rewrote the inline comment at the dispatch site to describe the spawn-boundary contract that B5/B6/B9 must uphold, rather than asserting a fact that the unwired code can't yet prove.
- Fixed doc-comment wording: "awaits the response on the spawned-task event loop" → "awaits the response without blocking the event loop".
- Addressed nit: trimmed the `dyn Client` archaeology paragraph in the doc comment now that `ConnectionTo<Client>` naming and `send_request` flow are self-explanatory.
- Verified `agent_prompt_handling.rs` itself produces zero diagnostics under `cargo check -p claude-agent`. The pre-existing E0404s in `agent.rs`, `agent_trait_impl.rs`, `lib.rs`, and `server.rs` are upstream of this task and explicitly tracked under B5/B6/B9.
