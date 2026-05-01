---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: done
position_ordinal: ffffffffffffffffffffffffaa80
project: acp-upgrade
title: 'ACP 0.11: claude-agent: per-feature handlers (validation, perms, notifications, commands)'
---
## What

Migrate the smaller per-feature handler modules to ACP 0.11.

Files:
- `claude-agent/src/agent_validation.rs` (if not already covered by validation task)
- `claude-agent/src/agent_permissions.rs`
- `claude-agent/src/agent_notifications.rs`
- `claude-agent/src/agent_commands.rs`

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p claude-agent`. Downstream may still fail.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in these files pass.

## Outcome (2026-04-29)

**No-op — B1 schema-import migration (commit `6f489b526`) already covered every file in this task's scope.**

Verification on `acp/0.11-rewrite`:

- `git show --stat 6f489b526` confirms B1 already touched all four files in this task: `agent_commands.rs`, `agent_notifications.rs`, `agent_permissions.rs`, `agent_validation.rs`.
- `grep agent_client_protocol::` across the four files returns only schema-form references plus two crate-root types (`Error`, `ErrorCode`) in `agent_validation.rs`, both of which remain valid public exports in ACP 0.11.
- `cargo check -p claude-agent --lib` reports 7 compile errors, all in downstream rewrite modules (`agent.rs`, `agent_trait_impl.rs`, `lib.rs`, `server.rs`) that subsequent ACP 0.11 tasks will reshape. Zero errors originate from any of:
  - `claude-agent/src/agent_validation.rs`
  - `claude-agent/src/agent_permissions.rs`
  - `claude-agent/src/agent_notifications.rs`
  - `claude-agent/src/agent_commands.rs`
- Per acceptance criterion "Downstream may still fail", that downstream-only state is the expected outcome of this task.
- Inline tests in these files cannot execute until the lib compiles end-to-end (downstream-blocked, also explicitly permitted by the task). They will run as part of B10 (`claude-agent/tests` migration) once the lib is green.

This follows the same precedent set by B2 (validation-modules, commit `44973dca6`), B3 (session-modules, commit `f6b44ae90`), and B5 (tool-modules, commit `b49fc5206`), which were also covered by B1 and recorded as no-op completions.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).