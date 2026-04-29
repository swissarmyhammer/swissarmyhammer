---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: todo
position_ordinal: fe80
project: acp-upgrade
title: 'ACP 0.11 SDK rewrite: bump + extras + claude-agent + llama-agent + avp-common (atomic)'
---
## What

Atomic rewrite spanning the full SDK migration. The spike (01KQ367HE0Z8ZSXY90CTT8QYGG) discovered ACP 0.11.0 is a complete SDK redesign — the `Agent` trait is gone, replaced by an `Agent` Role marker + `Agent.builder().on_receive_request(...).connect_to(...)` API (per https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html). There is no usable intermediate state where only some crates have been migrated, so the bump and the four core consumers must land together.

## Post-merge update (avp branch merged into mcp on 2026-04-29)

The avp merge (commit `b60ee8d6f`) materially changed the scope:
- `agent-client-protocol-extras/src/recording.rs` grew ~750 lines and now exposes a new `RecordingAgent<A>` wrapper (line 416) — fourth Agent wrapper in the crate alongside HookableAgent, TracingAgent, PlaybackAgent.
- `avp-common` is no longer a "lighter consumer" — it now contains a **production `impl Agent` in `src/context.rs:160`** and a **mock `impl Agent` in `src/validator/runner.rs:2204`** (plus extensive ACP-type-using test files). It must be rewritten in the same atomic block as extras / claude-agent / llama-agent.
- `claude-agent` and `llama-agent` ACP API surface is unchanged (1-line and 0-line delta respectively); internal file growth (`agent_prompt_handling.rs` +600 lines, `acp/server.rs` +156 lines) doesn't change rewrite shape.
- `swissarmyhammer-tools` confirmed still has zero ACP source usage despite avp adding ~1500 lines of new files. Drop-the-dep finding holds (handled by the consumers task).

Use a dedicated feature branch (e.g. `acp/0.11-rewrite`). Cherry-pick `spike/acp-0.11` commit `f206917c8` (the dep bump) as the starting point, or re-do the same diff:

- Edit `Cargo.toml`: `agent-client-protocol = "0.10"` → `agent-client-protocol = "0.11"`.
- Run `cargo update -p agent-client-protocol`. Confirm `Cargo.lock` resolves to `0.11.1`.
- Do **not** opt into the `unstable` feature flag.

Then rewrite, in order, the four core consumers below. The branch will not compile end-to-end until all four are done — that's expected; do not try to land partial states to `mcp`.

### Sub-rewrite 1: agent-client-protocol-extras (~247 ACP refs across 14 files)

Files:
- `agent-client-protocol-extras/src/hookable_agent.rs` — wraps `Agent` trait; rewrite to builder/handler.
- `agent-client-protocol-extras/src/tracing_agent.rs` — same.
- `agent-client-protocol-extras/src/recording.rs` — now ~3× original size; exposes `RecordingAgent<A>` (line 416). Re-implement on the new SDK shape.
- `agent-client-protocol-extras/src/playback.rs` — `PlaybackAgent` impl.
- `agent-client-protocol-extras/src/hook_config.rs`
- `agent-client-protocol-extras/src/lib.rs`
- `agent-client-protocol-extras/tests/e2e_hooks/*.rs` (8 test files using ACP types)

Wrappers' hook fanout / event capture / recording / playback are the valuable behaviors — preserve them. Per-variant handling on `SessionUpdate`, `ContentBlock`, `ToolCall*` re-derives from the new `agent_client_protocol::schema::*` module.

### Sub-rewrite 2: claude-agent (~755 ACP refs, full Agent impl)

Files:
- `claude-agent/src/agent.rs`, `agent_*.rs`
- `claude-agent/src/agent_trait_impl.rs` — old `impl Agent for ClaudeAgent` block; rewrite as builder/handler.
- `claude-agent/src/protocol_translator.rs`
- `claude-agent/src/session.rs`, `session_*.rs`
- `claude-agent/src/server.rs`
- `claude-agent/src/plan.rs`, `tools.rs`, `terminal_manager.rs`, `editor_state.rs`
- `claude-agent/src/agent_validation.rs`, `capability_validation.rs`, `request_validation.rs`, `content_*.rs`
- `claude-agent/src/lib.rs` (fix stale `agent_client_protocol::CollectedResponse` doc reference at line 96).
- `claude-agent/src/agent_prompt_handling.rs` (much larger post-merge; same ACP API surface).
- `claude-agent/tests/integration/*.rs`, `claude-agent/tests/common/*.rs`
- `claude-agent/Cargo.toml`:
  - Delete `[lib] test = false  # ... need ACP 0.9.0 fixes` line.
  - Delete `fix_tests_for_acp_0_9_0 = []` feature.

### Sub-rewrite 3: llama-agent (~504 ACP refs)

Files:
- `llama-agent/src/acp/server.rs` (top-level `AcpServer`; ~156 lines larger post-merge)
- `llama-agent/src/acp/translation.rs` (highest-risk; extensive `match` over schema types)
- `llama-agent/src/acp/session.rs`, `permissions.rs`, `commands.rs`, `terminal.rs`, `filesystem.rs`, `plan.rs`, `error.rs`, `config.rs`, `mod.rs`, `mcp_client_factory.rs`, `raw_message_manager.rs`, `test_utils.rs`
- `llama-agent/src/mcp_client_handler.rs`, `mcp.rs`, `agent.rs`
- `llama-agent/src/types/sessions.rs`
- `llama-agent/src/examples/acp_stdio.rs`
- `llama-agent/tests/acp_integration.rs`, `llama-agent/tests/coverage_tests.rs`, `llama-agent/tests/integration/acp_*.rs`

### Sub-rewrite 4: avp-common (~144 ACP refs across 8 files — NEW per merge)

Files:
- `avp-common/src/context.rs` (line 160-onwards: production `impl Agent`; ~26 ACP refs).
- `avp-common/src/validator/runner.rs` (line 2204-onwards: mock `impl Agent`; ~96 ACP refs — also includes `RecordingAgent` wiring).
- `avp-common/src/validator/executor.rs` (StopReason use).
- `avp-common/tests/recording_replay_integration.rs` (NEW — drives `PlaybackAgent`/`RecordingAgent` round-trips).
- `avp-common/tests/stop_hook_prompt_content_integration.rs` (NEW).
- `avp-common/tests/test_helpers.rs` (NEW).
- `avp-common/tests/model_config_integration.rs`.

Both `impl Agent` blocks (production + mock) need to move to the new builder/handler shape. Recording-related tests will need `RecordingAgent` to be available from the rewritten extras crate (sub-rewrite 1) before they can compile.

### Working order

1. Branch `acp/0.11-rewrite` from `mcp`.
2. Apply the dep bump (`Cargo.toml` + `Cargo.lock`).
3. Rewrite `agent-client-protocol-extras` until `cargo check -p agent-client-protocol-extras` passes.
4. Rewrite `claude-agent` until `cargo check -p claude-agent` passes; restore `[lib] test = true` (default), drop the dead feature.
5. Rewrite `llama-agent` until `cargo check -p llama-agent` passes.
6. Rewrite `avp-common` until `cargo check -p avp-common` passes (needs steps 3 + 5 to be in place — depends on extras + llama-agent for recording fixtures).
7. `cargo check --workspace --all-targets` — should now compile (consumers and conformance may still have surface mismatches; those are picked up by their own tasks).
8. Run all four crates' nextest suites — must be green before moving the task to review.
9. Squash-commit (or rebase-clean) the branch and merge to `mcp` only once everything compiles workspace-wide.

## Acceptance Criteria
- [ ] `Cargo.toml` lists `agent-client-protocol = "0.11"`; `Cargo.lock` resolves to `0.11.1`.
- [ ] `cargo check -p agent-client-protocol-extras --all-targets` passes.
- [ ] `cargo check -p claude-agent --all-targets` passes.
- [ ] `cargo check -p llama-agent --all-targets` passes.
- [ ] `cargo check -p avp-common --all-targets` passes.
- [ ] `cargo clippy -p agent-client-protocol-extras -p claude-agent -p llama-agent -p avp-common --all-targets -- -D warnings` passes.
- [ ] `claude-agent/Cargo.toml` no longer carries `[lib] test = false` or the `fix_tests_for_acp_0_9_0` feature.
- [ ] Stale `agent_client_protocol::CollectedResponse` doc reference in `claude-agent/src/lib.rs` is fixed.
- [ ] avp-common's recording-replay fixtures (`avp-common/tests/fixtures/recordings/*.json`) still deserialize under the new schema crate, or are regenerated with documented diff.
- [ ] Branch is rebased on `mcp`, conflicts resolved, ready to merge.

## Tests
- [ ] `cargo nextest run -p agent-client-protocol-extras` — green.
- [ ] `cargo nextest run -p claude-agent` — green (including the inline tests previously disabled by the dead feature; if a few were genuinely broken, fix or convert to `#[ignore]` with an explanation in the task comments).
- [ ] `cargo nextest run -p llama-agent` — green.
- [ ] `cargo nextest run -p avp-common` — green (incl. `recording_replay_integration`, `stop_hook_*`, `validator_*_integration`).

## Workflow
- Strict adaptation/rewrite — no feature additions in this task.
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html
- Reference 0.11.1 source under `~/.cargo/registry/src/index.crates.io-*/agent-client-protocol-0.11.1/` for the new API surface.
- Each sub-rewrite is large — the implementer can break it into commits per sub-rewrite, but the task only finishes (moves to review) when all four compile and test green together.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike — done).