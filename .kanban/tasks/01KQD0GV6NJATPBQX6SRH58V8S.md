---
assignees:
- claude-code
depends_on:
- 01KQD0FPSFZ7XSVBWRXTNJM6N3
- 01KQD0G0N3KDEZAHRJEQT5SS9W
- 01KQD0G6BY9KYN7NSR35PRA4CA
position_column: todo
position_ordinal: ff8780
project: acp-upgrade
title: 'ACP 0.11: extras: e2e_hooks integration tests'
---
## What

Migrate `agent-client-protocol-extras/tests/e2e_hooks/*.rs` to ACP 0.11.

8 test files (per spike survey): `cross_cutting_tests.rs`, `exit2_tests.rs`, `avp_schema_tests.rs`, `helpers.rs`, `json_continue_tests.rs`, `hook_edge_case_tests.rs`, `json_output_tests.rs`, `json_specific_output_tests.rs`. They drive `HookableAgent` end-to-end and assert on hook decisions emitted by external command-style handlers.

## Branch state at task start

A2 (HookableAgent), A3 (RecordingAgent), A4 (PlaybackAgent) all landed.

## Acceptance Criteria
- [ ] `cargo check -p agent-client-protocol-extras --tests` passes.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] `cargo nextest run -p agent-client-protocol-extras --test e2e_hooks` (or `cargo nextest run -p agent-client-protocol-extras` if tests aren't in a separate target) — green.

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html

## Depends on
- 01KQD0FPSFZ7XSVBWRXTNJM6N3 (A2: HookableAgent).
- 01KQD0G0N3KDEZAHRJEQT5SS9W (A3: RecordingAgent).
- 01KQD0G6BY9KYN7NSR35PRA4CA (A4: PlaybackAgent).