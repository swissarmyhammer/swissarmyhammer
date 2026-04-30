---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: review
position_ordinal: '8180'
project: acp-upgrade
title: 'ACP 0.11: claude-agent: agent_prompt_handling'
---
## What

Migrate `claude-agent/src/agent_prompt_handling.rs` to ACP 0.11. This file grew ~600 lines on the avp branch and is the largest single per-feature handler.

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [ ] `agent_prompt_handling.rs` compiles under `cargo check -p claude-agent`.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests pass.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).