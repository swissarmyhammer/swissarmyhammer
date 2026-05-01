---
assignees:
- claude-code
depends_on:
- 01KQD0D883ZW5JAA02913DXM8E
position_column: done
position_ordinal: ffffffffffffffffffffffff9d80
project: acp-upgrade
title: 'ACP 0.11: extras: RecordingAgent'
---
## What

Migrate `agent-client-protocol-extras/src/recording.rs` to ACP 0.11.

`RecordingAgent<A>` wraps an inner agent and records every request/response/notification to a JSONL file for later replay. It serializes ACP schema types verbatim, so it touches every variant of `SessionUpdate`, `ContentBlock`, `ToolCall*`, `StopReason`, `ContentChunk`, `Plan`, `PlanEntry`, etc.

Files:
- `agent-client-protocol-extras/src/recording.rs` (the file is large — ~750 lines added in the avp branch).

Use the wrapper pattern from A1.

## Branch state at task start

`acp/0.11-rewrite` with `d5b5465bd` + A1's commit. (Does NOT depend on A2; recording is independent of hooks.)

## Acceptance Criteria
- [ ] `cargo check -p agent-client-protocol-extras --lib` passes for `recording.rs`.
- [ ] On-disk JSONL format unchanged — wire format is fixture-stable. Spot-check by deserializing a fixture from `acp-conformance/.fixtures/` or `avp-common/tests/fixtures/recordings/` if practical.
- [ ] Public types preserved: `RecordingAgent`, `RecordedSession`, `RecordedEvent`, etc.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `recording.rs` pass.
- [ ] Replay-roundtrip test (one of the existing tests if any) passes.

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html
- If wire format genuinely shifts (e.g. due to `#[non_exhaustive]` field additions), document the new format in task comments and write a migration note for downstream fixture regeneration.

## Depends on
- 01KQD0D883ZW5JAA02913DXM8E (A1: foundation).