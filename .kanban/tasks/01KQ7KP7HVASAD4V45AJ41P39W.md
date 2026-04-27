---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: todo
position_ordinal: fe80
project: acp-upgrade
title: 'ACP 0.11 SDK rewrite: bump + agent-client-protocol-extras + claude-agent + llama-agent (atomic)'
---
## What

Atomic rewrite spanning the full SDK migration. The spike (01KQ367HE0Z8ZSXY90CTT8QYGG) discovered ACP 0.11.0 is a complete SDK redesign — the `Agent` trait is gone, replaced by an `Agent` Role marker + `Agent.builder().on_receive_request(...).connect_to(...)` API (per https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html). There is no usable intermediate state where only some crates have been migrated, so the bump and the three core consumers must land together.

Use a dedicated feature branch (e.g. `acp/0.11-rewrite`). Cherry-pick `spike/acp-0.11` commit `f206917c8` (the dep bump) as the starting point, or re-do the same diff:

- Edit `Cargo.toml`: `agent-client-protocol = "0.10"` → `agent-client-protocol = "0.11"`.
- Run `cargo update -p agent-client-protocol`. Confirm `Cargo.lock` resolves to `0.11.1`.
- Do **not** opt into the `unstable` feature flag.

Then rewrite, in order, the three core consumers below. The branch will not compile end-to-end until all three are done — that's expected; do not try to land partial states to `mcp`.

### Sub-rewrite 1: agent-client-protocol-extras (~225 ACP refs across the crate)

Files:
- `agent-client-protocol-extras/src/hookable_agent.rs`
- `agent-client-protocol-extras/src/tracing_agent.rs`
- `agent-client-protocol-extras/src/recording.rs`
- `agent-client-protocol-extras/src/playback.rs`
- `agent-client-protocol-extras/src/hook_config.rs`
- `agent-client-protocol-extras/src/lib.rs`
- `agent-client-protocol-extras/tests/e2e_hooks/*.rs`

The `HookableAgent` and `TracingAgent` proxies need to be rewritten on top of the new builder API rather than the old `impl Agent` trait. The hook fanout / event capture is the valuable behavior — preserve it. Recording/playback consume `SessionUpdate`, `ContentBlock`, `ToolCall*` types whose internal layout changed; re-derive the per-variant handling from the new schema-types module.

### Sub-rewrite 2: claude-agent (~754 ACP refs, full Agent impl)

Files:
- `claude-agent/src/agent.rs`, `agent_*.rs`
- `claude-agent/src/agent_trait_impl.rs` — the old `impl Agent for ClaudeAgent` block; rewrite as a builder/handler.
- `claude-agent/src/protocol_translator.rs`
- `claude-agent/src/session.rs`, `session_*.rs`
- `claude-agent/src/server.rs`
- `claude-agent/src/plan.rs`, `tools.rs`, `terminal_manager.rs`, `editor_state.rs`
- `claude-agent/src/agent_validation.rs`, `capability_validation.rs`, `request_validation.rs`, `content_*.rs`
- `claude-agent/src/lib.rs` (fix the stale `agent_client_protocol::CollectedResponse` doc reference at line 96 — the local `claude_agent::CollectedResponse` stays).
- `claude-agent/tests/integration/*.rs`, `claude-agent/tests/common/*.rs`
- `claude-agent/Cargo.toml`:
  - Delete the `[lib] test = false  # ... need ACP 0.9.0 fixes` line — feature reference is dead workspace-wide (679 inline tests are silently disabled).
  - Delete the `fix_tests_for_acp_0_9_0 = []` feature — referenced nowhere.

### Sub-rewrite 3: llama-agent (~504 ACP refs)

Files:
- `llama-agent/src/acp/server.rs` (top-level `AcpServer`)
- `llama-agent/src/acp/translation.rs` (heavy schema-type usage; the largest single file)
- `llama-agent/src/acp/session.rs`, `permissions.rs`, `commands.rs`, `terminal.rs`, `filesystem.rs`, `plan.rs`, `error.rs`, `config.rs`, `mod.rs`, `mcp_client_factory.rs`, `raw_message_manager.rs`
- `llama-agent/src/mcp_client_handler.rs`, `mcp.rs`, `agent.rs`
- `llama-agent/src/examples/acp_stdio.rs`
- `llama-agent/tests/acp_integration.rs`, `llama-agent/tests/integration/acp_*.rs`

Translation layer is the highest-risk piece — extensive `match` over `SessionUpdate`, `ContentBlock`, `ContentChunk`, `ToolKind`, `ToolCallStatus`. Re-derive against the new schema-types module.

### Working order

1. Branch `acp/0.11-rewrite` from `mcp`.
2. Apply the dep bump (`Cargo.toml` + `Cargo.lock`).
3. Rewrite `agent-client-protocol-extras` until `cargo check -p agent-client-protocol-extras` passes.
4. Rewrite `claude-agent` until `cargo check -p claude-agent` passes; restore `[lib] test = true` (default), drop the dead feature.
5. Rewrite `llama-agent` until `cargo check -p llama-agent` passes.
6. `cargo check --workspace --all-targets` — should now compile (consumers and conformance may still have surface mismatches; those are picked up by their own tasks).
7. Run **all three** crates' nextest suites — must be green before moving the task to review.
8. Squash-commit (or rebase-clean) the branch and merge to `mcp` *only* once everything compiles workspace-wide.

## Acceptance Criteria
- [ ] `Cargo.toml` lists `agent-client-protocol = "0.11"`; `Cargo.lock` resolves to `0.11.1`.
- [ ] `cargo check -p agent-client-protocol-extras --all-targets` passes.
- [ ] `cargo check -p claude-agent --all-targets` passes.
- [ ] `cargo check -p llama-agent --all-targets` passes.
- [ ] `cargo clippy -p agent-client-protocol-extras -p claude-agent -p llama-agent --all-targets -- -D warnings` passes.
- [ ] `claude-agent/Cargo.toml` no longer carries `[lib] test = false` or the `fix_tests_for_acp_0_9_0` feature.
- [ ] Stale `agent_client_protocol::CollectedResponse` doc reference in `claude-agent/src/lib.rs` is fixed.
- [ ] Branch is rebased on `mcp`, conflicts resolved, ready to merge.

## Tests
- [ ] `cargo nextest run -p agent-client-protocol-extras` — green.
- [ ] `cargo nextest run -p claude-agent` — green (including the 679 inline tests previously disabled by the dead feature; if a few were genuinely broken, fix or convert to `#[ignore]` with an explanation in the task comments).
- [ ] `cargo nextest run -p llama-agent` — green.

## Workflow
- Strict adaptation/rewrite — no feature additions in this task.
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html
- Reference 0.11.1 source under `~/.cargo/registry/src/index.crates.io-*/agent-client-protocol-0.11.1/` for the new API surface.
- Each sub-rewrite is large — the implementer can break it into commits per sub-rewrite, but the task only finishes (moves to review) when all three compile and test green together.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike — done).