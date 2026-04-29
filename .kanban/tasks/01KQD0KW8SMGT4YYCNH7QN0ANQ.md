---
assignees:
- claude-code
depends_on:
- 01KQD0EDB540RNXPTBEX4MNT83
- 01KQD0D883ZW5JAA02913DXM8E
- 01KQD0G0N3KDEZAHRJEQT5SS9W
position_column: todo
position_ordinal: ff9780
project: acp-upgrade
title: 'ACP 0.11: avp-common: context.rs production Agent reshape'
---
## What

Migrate the production `impl Agent for AvpContext` block in `avp-common/src/context.rs` (line 160-onwards) to the new builder/handler API. AvpContext wraps an inner agent (uses `RecordingAgent`) and is the entry point used by the validator runner.

Files:
- `avp-common/src/context.rs`

## Branch state at task start

D1 (avp-common imports) + A1 (TracingAgent foundation) + A3 (RecordingAgent) all landed.

## Acceptance Criteria
- [ ] `context.rs` compiles under `cargo check -p avp-common`.
- [ ] AvpContext public surface preserved.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `context.rs` pass.

## Depends on
- 01KQD0EDB540RNXPTBEX4MNT83 (D1).
- 01KQD0D883ZW5JAA02913DXM8E (A1).
- 01KQD0G0N3KDEZAHRJEQT5SS9W (A3).