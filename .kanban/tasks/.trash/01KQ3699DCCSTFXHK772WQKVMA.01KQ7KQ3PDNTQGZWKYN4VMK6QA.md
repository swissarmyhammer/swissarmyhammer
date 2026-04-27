---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: f880
project: acp-upgrade
title: Adapt claude-agent to ACP 0.11
---
## What

**REWRITE** `claude-agent/` against the new ACP 0.11.1 SDK design. The old `impl Agent` pattern is gone (see spike findings on task 01KQ367HE0Z8ZSXY90CTT8QYGG and migration guide https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html). This is the **largest** crate in the upgrade — 754 lines reference `agent_client_protocol`, including a full `Agent` trait impl in `agent_trait_impl.rs`.

## Spike-confirmed scope

### The full Agent impl is gone
- `claude-agent/src/agent_trait_impl.rs` is a complete `#[async_trait::async_trait(?Send)] impl Agent` with all 9 methods (initialize, authenticate, new_session, load_session, set_session_mode, prompt, cancel, ext_method, ext_notification). **Rewrite the whole file** as a builder graph (`Agent.builder().on_receive_request(...).on_receive_dispatch(...)`) keyed on the typed message variants.
- The supporting modules (`agent.rs`, `agent_*.rs`, `protocol_translator.rs`, `session.rs`, `session_*.rs`, `plan.rs`, `tools.rs`, `terminal_manager.rs`, `editor_state.rs`, `agent_validation.rs`, `capability_validation.rs`, `request_validation.rs`, `content_*.rs`) all need import-path updates (`agent_client_protocol::X` → `agent_client_protocol::schema::X`) plus possibly behavioral changes wherever they consume the old trait shape.

### Match-arm rewrites
- All `match` blocks over `SessionUpdate`, `ContentBlock`, `StopReason`, `ToolKind`, `ToolCallStatus` continue to need `#[non_exhaustive]` handling — explicit variant arms or documented catch-alls.
- `RequestPermissionOutcome` (Cancelled, Selected) and `PermissionOptionKind` (AllowAlways, AllowOnce, RejectAlways, RejectOnce) still exist; verify variants still match.

### Stale references
- `claude-agent/src/lib.rs` line 96 has a doc comment referencing `agent_client_protocol::CollectedResponse` — that type does not exist in 0.11. Drop the cross-reference (the local `claude_agent::CollectedResponse` stays).
- The local `CollectedResponse` struct is fine; only the comment is stale.

### Tests
- `claude-agent/tests/integration/*.rs` and `claude-agent/tests/common/*.rs` define mock Agents and helpers. **Rewrite mocks** in the new builder/handler style.
- `claude-agent/tests/integration/coverage_tests.rs:2324` constructs `claude_agent::CollectedResponse { ... }` — that's our local type, still fine.

### Cleanup (from spike — high priority)

**`claude-agent/Cargo.toml` `[lib] test = false` + `fix_tests_for_acp_0_9_0` feature must go.**

The spike confirmed:
- `[lib] test = false  # Disable lib cfg(test) modules - need ACP 0.9.0 fixes` references ACP **0.9.0** (current dep is 0.10.4, target is 0.11.1) — two major versions stale.
- `[features] fix_tests_for_acp_0_9_0 = []` is declared but **never referenced anywhere in the workspace** (`grep -r "fix_tests_for_acp_0_9_0" --include="*.rs"` returns zero matches).
- `claude-agent/src/**/*.rs` contains **728 inline `#[test]` / `#[tokio::test]` cases** all currently disabled by `lib.test = false`. That's a large coverage hole.

Drop both lines, re-enable lib tests, and fix any test that was actually broken by the ACP rewrite. Use the inline tests to validate the rewrite — they're the best safety net we have.

## Acceptance Criteria
- [ ] `cargo check -p claude-agent --all-targets` passes.
- [ ] `cargo clippy -p claude-agent --all-targets -- -D warnings` passes.
- [ ] `[lib] test = false` and `[features] fix_tests_for_acp_0_9_0 = []` are removed from `claude-agent/Cargo.toml`.
- [ ] All 728 inline lib tests compile and pass under the new ACP API.
- [ ] No `Agent` trait impl remains; agent assembly uses the 0.11 builder/handler pattern.
- [ ] Stale `agent_client_protocol::CollectedResponse` comment in `lib.rs` is gone.

## Tests
- [ ] `cargo nextest run -p claude-agent --lib` — green (this is the big re-enable).
- [ ] `cargo nextest run -p claude-agent --tests` — green (integration tests).

## Workflow
- Read the migration guide first: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html
- Read `examples/simple_agent.rs` in the 0.11.1 source for the canonical builder pattern.
- This is a **rewrite**, not a refactor. Don't try to patch the old trait shape — replace it.
- Sequence: re-enable lib tests on the OLD ACP first (separate commit) so we have a baseline of what's actually broken vs what was just stale, then do the 0.11 rewrite. (Optional — but strongly recommended for clean diff hygiene.)

## Depends on
- 01KQ367XFMW2CP7GWM4GJ41BNR (version bump).
- 01KQ368MJDZNHXZ5HM9QQ0JGBK (extras crate rewritten — claude-agent depends on its new wrapper API).
- Spike findings: 01KQ367HE0Z8ZSXY90CTT8QYGG.
