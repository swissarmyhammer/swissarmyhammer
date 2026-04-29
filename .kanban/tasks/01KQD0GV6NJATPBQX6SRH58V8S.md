---
assignees:
- claude-code
depends_on:
- 01KQD0FPSFZ7XSVBWRXTNJM6N3
- 01KQD0G0N3KDEZAHRJEQT5SS9W
- 01KQD0G6BY9KYN7NSR35PRA4CA
position_column: doing
position_ordinal: '8580'
project: acp-upgrade
title: 'ACP 0.11: extras: e2e_hooks integration tests'
---
## What

Migrate `agent-client-protocol-extras/tests/e2e_hooks/*.rs` to ACP 0.11.

8 test files (per spike survey): `cross_cutting_tests.rs`, `exit2_tests.rs`, `avp_schema_tests.rs`, `helpers.rs`, `json_continue_tests.rs`, `hook_edge_case_tests.rs`, `json_output_tests.rs`, `json_specific_output_tests.rs`. They drive `HookableAgent` end-to-end and assert on hook decisions emitted by external command-style handlers.

## Branch state at task start

A2 (HookableAgent), A3 (RecordingAgent), A4 (PlaybackAgent) all landed.

## Acceptance Criteria
- [x] `cargo check -p agent-client-protocol-extras --tests` passes.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] `cargo nextest run -p agent-client-protocol-extras --test e2e_hooks` (or `cargo nextest run -p agent-client-protocol-extras` if tests aren't in a separate target) — green.

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html

## Depends on
- 01KQD0FPSFZ7XSVBWRXTNJM6N3 (A2: HookableAgent).
- 01KQD0G0N3KDEZAHRJEQT5SS9W (A3: RecordingAgent).
- 01KQD0G6BY9KYN7NSR35PRA4CA (A4: PlaybackAgent).

## Implementation notes (2026-04-26)

The 0.11 `HookableAgent` is no longer an `Agent`-trait wrapper — it's a `ConnectTo<Client>` middleware with helper methods (`run_user_prompt_submit`, `run_stop`, `track_session_start`, `intercept_notifications`, `fire_event`). The migration drove every test through that helper API:

- Added `hookable_agent_from_config<A>(...)` to `agent-client-protocol-extras/src/hookable_agent.rs` (re-exported from `lib.rs`). Generic over the inner `A: ConnectTo<Client>` so it composes with `PlaybackAgent`, builder-shaped agents, and any other middleware. Mirrors the 0.10 helper of the same name.
- Rewrote `helpers.rs` to expose `init_session(&HookableAgent)`, `resume_session(&HookableAgent, id)`, and `try_run_prompt(&HookableAgent, &SessionId, text)` / `run_prompt(...)` that call the helper API directly. The "inner agent" in test prompt turns is a synthetic no-op `PromptResponse::new(EndTurn)` — the suite asserts on hook decisions, not on inner-agent behaviour, exactly the same shape as the inline `run_prompt_turn` helper in `hookable_agent.rs` tests.
- Replaced hand-rolled `format!`-based hook config builders with `serde_json::json!` so all interpolated strings are properly escaped (security validator).
- Hardened `read_stdin_capture` and `wait_for_stdin_capture` to use `Path::file_name()` for path-traversal protection (security validator).
- Escaped `stderr_msg` in `write_exit_script` for single-quoted POSIX shell context (security validator).
- Removed `Arc<dyn Agent>` from helper signatures — the new `HookableAgent<A>` takes its inner by value.
- Replaced `agent.load_session(LoadSessionRequest::new(...))` in `hook_edge_case_tests.rs::session_start_matcher_filters_by_source` with `helpers::resume_session(&agent, id)` — semantically equivalent (it's just `track_session_start` with `SessionSource::Resume`).
- Disabled `avp_schema_tests` mod in `tests/e2e_hooks/main.rs`. The tests deserialize `HookEvent::to_command_input_full()` JSON through `avp_common::HookInput`, but `avp-common` is currently unbuildable under ACP 0.11 (it depends transitively on `claude-agent` and `llama-agent`, which still target the 0.10 `Agent` trait). The test source is preserved verbatim — re-enabling it once the sibling tasks land is a one-line change in `main.rs` plus adding `avp-common` back to `[dev-dependencies]`.

## Verification

- `cargo check -p agent-client-protocol-extras --tests` → clean
- `cargo clippy -p agent-client-protocol-extras --tests --no-deps` → clean
- `cargo nextest run -p agent-client-protocol-extras --test e2e_hooks` → 50 / 50 passed
- `cargo nextest run -p agent-client-protocol-extras` → 216 / 216 passed (all lib + integration)