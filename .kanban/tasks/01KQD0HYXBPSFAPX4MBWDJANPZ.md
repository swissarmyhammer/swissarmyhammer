---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: todo
position_ordinal: ff8f80
project: acp-upgrade
title: 'ACP 0.11: claude-agent: server wiring'
---
## What

Migrate the top-level server-wiring modules to ACP 0.11.

Files:
- `claude-agent/src/server.rs`
- `claude-agent/src/conversation_manager.rs`
- `claude-agent/src/claude.rs`
- `claude-agent/src/claude_process.rs`
- `claude-agent/src/terminal_manager.rs`
- `claude-agent/src/editor_state.rs`
- `claude-agent/src/plan.rs`
- `claude-agent/src/permissions.rs`
- `claude-agent/src/mcp_error_handling.rs`

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [ ] These modules compile under `cargo check -p claude-agent`. The trait impl block in `agent_trait_impl.rs` may still fail until B8.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in these files pass.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).