---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: todo
position_ordinal: ff8b80
project: acp-upgrade
title: 'ACP 0.11: claude-agent: tool modules'
---
## What

Migrate tool-related modules to ACP 0.11.

Files:
- `claude-agent/src/tools.rs`
- `claude-agent/src/tool_types.rs`
- `claude-agent/src/tool_classification.rs`
- `claude-agent/src/tool_call_lifecycle_tests.rs`

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