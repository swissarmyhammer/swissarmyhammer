---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: done
position_ordinal: ffffffffffffffffffffffffa780
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
- [x] These modules compile under `cargo check -p claude-agent`. Downstream may still fail.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in these files pass.

## Outcome (2026-04-30)

**No-op — B1 schema-import migration (commit `6f489b526`) already covered every file in this task's scope.**

Verification on `acp/0.11-rewrite`:

- `grep -E 'agent_client_protocol::[A-Za-z]'` filtered against `::schema::` and `::Error` returns zero non-compliant references across all three files. Every `agent_client_protocol::*` reference is either the `::schema::*` form (schema types relocated under 0.11) or `::Error` (which intentionally stays at the crate root per the B1 commit message).
- `agent_file_operations.rs` has zero `agent_client_protocol` references — it only defines local `ReadTextFileParams` / `ReadTextFileResponse` / `WriteTextFileParams` data carriers.
- `cargo check -p claude-agent --lib --message-format=short` filtered to errors originating from any of:
  - `claude-agent/src/agent_file_handlers.rs`
  - `claude-agent/src/agent_file_operations.rs`
  - `claude-agent/src/agent_terminal_handlers.rs`

  returns zero errors. The whole-crate check still reports 7 errors, all confined to `agent.rs`, `agent_trait_impl.rs`, and `lib.rs` — the architectural rewrite modules deferred to subsequent tasks (B7/B8/B9). Per acceptance criterion "Downstream may still fail", that downstream-only state is the expected outcome of this task.
- `client_capabilities` access (`caps.fs.read_text_file`, `caps.fs.write_text_file`, `caps.terminal`) and `WriteTextFileResponse::default()` continue to compile against the 0.11 `agent_client_protocol::schema::ClientCapabilities` / `FileSystemCapabilities` / `WriteTextFileResponse` types — verified against schema source at `agent-client-protocol-schema-0.12.0/src/client.rs` (the schema crate version pulled in by `agent-client-protocol = "0.11"`).
- These three files contain no inline tests, so the "inline tests pass" criterion is vacuously satisfied. End-to-end execution of the handlers will be exercised under B10 (`claude-agent/tests` migration) once the lib is green end-to-end.

This follows the precedent set by B2 (validation-modules, commit `44973dca6`), B3 (session-modules, commit `f6b44ae90`), and B5 (tool modules, task `01KQD0HE4STQ2K7XJDNC2TN4XJ`), all of which were covered by B1 and recorded as no-op completions.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).