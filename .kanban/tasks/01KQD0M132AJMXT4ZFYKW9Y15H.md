---
assignees:
- claude-code
depends_on:
- 01KQD0EDB540RNXPTBEX4MNT83
- 01KQD0G0N3KDEZAHRJEQT5SS9W
- 01KQD0MMR7W64307S03XBV69BH
- 01KQD0NS3EFZ6Q7WCN5FME36VY
position_column: todo
position_ordinal: ff9880
project: acp-upgrade
title: 'ACP 0.11: avp-common: validator/runner.rs mock Agent + RecordingAgent wiring'
---
## What

Migrate `avp-common/src/validator/runner.rs` to ACP 0.11. Includes:
- The mock `impl Agent` block at line 2204-onwards.
- All `RecordingAgent` wiring (recording fixtures, replay-driven assertions).
- 96 ACP refs across this file alone.

Files:
- `avp-common/src/validator/runner.rs`

## Branch state at task start

D1 (avp-common imports) + A3 (RecordingAgent) landed.

## Acceptance Criteria
- [ ] `runner.rs` compiles under `cargo check -p avp-common`.
- [ ] Mock Agent's behavior preserved (used by inline + integration tests).
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `runner.rs` pass.

## Depends on
- 01KQD0EDB540RNXPTBEX4MNT83 (D1).
- 01KQD0G0N3KDEZAHRJEQT5SS9W (A3).