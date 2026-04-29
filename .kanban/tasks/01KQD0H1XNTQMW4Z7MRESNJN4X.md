---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: todo
position_ordinal: ff8880
project: acp-upgrade
title: 'ACP 0.11: claude-agent: error.rs + protocol_translator.rs + content_block_processor.rs'
---
## What

Migrate the pure type-conversion modules to ACP 0.11. These don't implement Agent — they map between ACP schema types and internal claude-agent types.

Files:
- `claude-agent/src/error.rs`
- `claude-agent/src/protocol_translator.rs`
- `claude-agent/src/content_block_processor.rs`

## Branch state at task start

`acp/0.11-rewrite` with B0 + B1 landed.

## Acceptance Criteria
- [ ] These three files compile under `cargo check -p claude-agent`. Other modules may still fail.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in these three files pass.

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0: housekeeping).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1: schema imports).