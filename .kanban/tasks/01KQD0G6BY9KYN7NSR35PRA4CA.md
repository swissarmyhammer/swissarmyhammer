---
assignees:
- claude-code
depends_on:
- 01KQD0D883ZW5JAA02913DXM8E
position_column: done
position_ordinal: ffffffffffffffffffffffff9e80
project: acp-upgrade
title: 'ACP 0.11: extras: PlaybackAgent'
---
## What

Migrate `agent-client-protocol-extras/src/playback.rs` to ACP 0.11.

`PlaybackAgent` is the inverse of `RecordingAgent` — it loads a recorded JSONL session and replays its events. It must produce the same notification stream and request/response sequence the recording captured.

Files:
- `agent-client-protocol-extras/src/playback.rs`

## Branch state at task start

`acp/0.11-rewrite` with `d5b5465bd` + A1's commit. (Does NOT need A3 to compile, but in practice will be paired with A3 for testing.)

## Acceptance Criteria
- [ ] `cargo check -p agent-client-protocol-extras --lib` passes for `playback.rs`.
- [ ] Public type `PlaybackAgent` preserved.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `playback.rs` pass.
- [ ] Round-trip test (record + playback yields identical event stream) — covered in `avp-common`'s `recording_replay_integration` (handled by D4) but spot-check here is welcome.

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html

## Depends on
- 01KQD0D883ZW5JAA02913DXM8E (A1: foundation).