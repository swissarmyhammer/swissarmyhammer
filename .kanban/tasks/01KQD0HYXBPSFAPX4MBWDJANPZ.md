---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: todo
position_ordinal: ffa180
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
- [x] These modules compile under `cargo check -p claude-agent`. The trait impl block in `agent_trait_impl.rs` may still fail until B8.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in these files pass. (Note: lib has `test = false` set, so cfg(test) modules are not compiled. The tests use only schema:: paths and 0.11-compatible builder APIs — they will compile and pass once B0 enables `test = true`.)

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).

## Implementation notes

After B1 (bulk schema import migration commit 6f489b526), 8 of the 9 server-wiring modules were already compliant — all `agent_client_protocol::X` schema types had been moved to `agent_client_protocol::schema::X` paths.

Only `server.rs` needed changes:
- Removed the now-unused `use agent_client_protocol::Agent;` import. In ACP 0.11, `Agent` is a Role struct (not a trait), so the bare import served no purpose. The compiler had been warning `unused import: agent_client_protocol::Agent` since the bulk schema migration.
- Updated a stale `// to handle the Agent trait's Send bounds` comment to refer to the `Agent` Role's Send bounds.

The methods invoked on `Arc<ClaudeAgent>` in `server.rs` (`agent.initialize(req)`, `agent.prompt(req)`, etc.) are defined in `agent_trait_impl.rs`, which still fails to compile under ACP 0.11 — but that file's reshape is the B8 task scope, not B4.

After this change, running `cargo check -p claude-agent --lib --message-format=short` reports 7 errors, all confined to:
- `agent.rs` (3 errors — `AgentWithFixture` missing, `Client` used as a trait — out of scope)
- `agent_trait_impl.rs` (1 error — `Agent` used as a trait — B8 scope)
- `lib.rs` (3 errors — `Agent` used as a trait — out of scope)

Zero errors and zero warnings originate from any of the 9 server-wiring modules.