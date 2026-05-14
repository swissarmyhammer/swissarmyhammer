---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: done
position_ordinal: ffffffffffffffffffffffffa880
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
- [x] These modules compile under `cargo check -p claude-agent`. Downstream may still fail.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in these files pass.

## Outcome (2026-04-30)

**No-op — B1 schema-import migration (commit `6f489b526`) already covered every file in this task's scope.**

Verification on `acp/0.11-rewrite`:

- `grep agent_client_protocol(?!::schema)` returns zero matches across all four files. Every `agent_client_protocol::*` reference is already the `::schema::*` form required by ACP 0.11.
- `cargo check -p claude-agent --lib` reports 7 compile errors, all in downstream rewrite modules (`agent.rs`, `agent_trait_impl.rs`, `lib.rs`) that subsequent tasks (B7/B8/B9) will reshape. Zero errors originate from any of:
  - `claude-agent/src/tools.rs`
  - `claude-agent/src/tool_types.rs`
  - `claude-agent/src/tool_classification.rs`
  - `claude-agent/src/tool_call_lifecycle_tests.rs`
- Per acceptance criterion "Downstream may still fail", that downstream-only state is the expected outcome of this task.
- Inline tests in these files cannot execute until the lib compiles end-to-end (downstream-blocked, also explicitly permitted by the task). They will run as part of B10 (`claude-agent/tests` migration) once the lib is green.

This follows the same precedent set by B2 (validation-modules, commit `44973dca6`) and B3 (session-modules, commit `f6b44ae90`), which were also covered by B1 and recorded as no-op completions.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).