---
assignees:
- claude-code
depends_on:
- 01KQD0FPSFZ7XSVBWRXTNJM6N3
- 01KQD0G0N3KDEZAHRJEQT5SS9W
- 01KQD0G6BY9KYN7NSR35PRA4CA
- 01KQD0KW8SMGT4YYCNH7QN0ANQ
- 01KQD0M132AJMXT4ZFYKW9Y15H
position_column: todo
position_ordinal: ffa180
project: acp-upgrade
title: 'ACP 0.11: extras: e2e_hooks integration tests'
---
## What

Migrate `agent-client-protocol-extras/tests/e2e_hooks/*.rs` to ACP 0.11.

8 test files (per spike survey): `cross_cutting_tests.rs`, `exit2_tests.rs`, `avp_schema_tests.rs`, `helpers.rs`, `json_continue_tests.rs`, `hook_edge_case_tests.rs`, `json_output_tests.rs`, `json_specific_output_tests.rs`. They drive `HookableAgent` end-to-end and assert on hook decisions emitted by external command-style handlers.

## Branch state at task start

A2 (HookableAgent), A3 (RecordingAgent), A4 (PlaybackAgent) all landed.

> **Reopened 2026-04-30**: previous "done" claim was a test-skip workaround (`mod avp_schema_tests;` was commented out in `main.rs` because `avp-common` did not compile under ACP 0.11). The validator `test-integrity:no-test-cheating` correctly flagged the disable. Task is now properly gated on the avp-common reshape (D2 + D3) so when it finishes, every `e2e_hooks` test actually runs.

## Acceptance Criteria
- [ ] `cargo check -p agent-client-protocol-extras --tests` passes.
- [ ] `mod avp_schema_tests;` is **enabled** in `tests/e2e_hooks/main.rs` (no comment-out, no `#[cfg]` gate).
- [ ] `avp-common` is in `agent-client-protocol-extras/Cargo.toml` `[dev-dependencies]`.
- [ ] All schema-type imports inside `tests/e2e_hooks/*.rs` use `agent_client_protocol::schema::*` paths.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] `cargo nextest run -p agent-client-protocol-extras --test e2e_hooks` — all 50+ tests green, including `avp_schema_tests`.

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html
- The `avp_schema_tests` are a regression suite that catches field-name mismatches between `HookEvent::to_command_input_full()` JSON and `avp_common::HookInput`. They MUST be enabled and passing.

## Depends on
- 01KQD0FPSFZ7XSVBWRXTNJM6N3 (A2: HookableAgent).
- 01KQD0G0N3KDEZAHRJEQT5SS9W (A3: RecordingAgent).
- 01KQD0G6BY9KYN7NSR35PRA4CA (A4: PlaybackAgent).
- 01KQD0KW8SMGT4YYCNH7QN0ANQ (D2: avp-common context.rs production Agent reshape — required for `avp-common` to compile).
- 01KQD0M132AJMXT4ZFYKW9Y15H (D3: avp-common runner.rs mock Agent + RecordingAgent wiring — same reason).