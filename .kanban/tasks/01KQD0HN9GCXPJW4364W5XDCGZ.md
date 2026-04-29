---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: todo
position_ordinal: ff8d80
project: acp-upgrade
title: 'ACP 0.11: claude-agent: file & terminal handlers'
---
## What

Migrate file/terminal handler modules to ACP 0.11.

Files:
- `claude-agent/src/agent_file_handlers.rs`
- `claude-agent/src/agent_file_operations.rs`
- `claude-agent/src/agent_terminal_handlers.rs`

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [ ] These modules compile under `cargo check -p claude-agent`. Downstream may still fail.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in these files pass.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).