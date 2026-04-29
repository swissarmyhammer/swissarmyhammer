---
assignees:
- claude-code
depends_on:
- 01KQD0KW8SMGT4YYCNH7QN0ANQ
- 01KQD0M132AJMXT4ZFYKW9Y15H
position_column: todo
position_ordinal: ff9f80
project: acp-upgrade
title: 'ACP 0.11: avp-common: integration tests'
---
## What

Migrate avp-common's integration tests to ACP 0.11.

Files:
- `avp-common/tests/recording_replay_integration.rs` (drives RecordingAgent + PlaybackAgent round-trips)
- `avp-common/tests/stop_hook_prompt_content_integration.rs`
- `avp-common/tests/stop_hook_code_quality_regression.rs`
- `avp-common/tests/validator_block_e2e_integration.rs`
- `avp-common/tests/validator_tools_partial_integration.rs`
- `avp-common/tests/test_helpers.rs`
- `avp-common/tests/model_config_integration.rs`

Recording fixtures under `avp-common/tests/fixtures/recordings/*.json` should still deserialize. If they don't, document the wire-format change in this task and regenerate.

## Branch state at task start

D2 (context.rs) + D3 (runner.rs) landed.

## Acceptance Criteria
- [ ] `cargo check -p avp-common --tests` passes.
- [ ] Recording fixtures deserialize cleanly, or are regenerated with documented diff.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] `cargo nextest run -p avp-common` — green.

## Depends on
- (D2) avp-common: context.rs production Agent reshape.
- (D3) avp-common: validator/runner.rs mock Agent + RecordingAgent wiring.