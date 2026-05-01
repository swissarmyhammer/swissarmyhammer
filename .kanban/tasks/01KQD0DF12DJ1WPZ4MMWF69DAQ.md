---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: done
position_ordinal: ffffffffffffffffffffffff9880
project: acp-upgrade
title: 'ACP 0.11: claude-agent: housekeeping (drop dead feature, fix stale doc)'
---
## What

Pre-rewrite hygiene that doesn't depend on any other ACP migration work.

Edits:
- `claude-agent/Cargo.toml`: delete the `[lib] test = false  # ... need ACP 0.9.0 fixes` line, and delete the `fix_tests_for_acp_0_9_0 = []` feature. Both are dead workspace-wide (no `*.rs` references). Removing them re-enables ~679 inline tests in `claude-agent/src/`.
- `claude-agent/src/lib.rs` line 96: fix the stale doc reference to `agent_client_protocol::CollectedResponse` (that type doesn't exist in 0.11; the local `claude_agent::CollectedResponse` stays).

## Branch state at task start

`acp/0.11-rewrite` with commit `d5b5465bd` (dep bump). This task can land before any other claude-agent rewrite task — it's independent.

## Acceptance Criteria
- [ ] `claude-agent/Cargo.toml` no longer carries `[lib] test = false` or `fix_tests_for_acp_0_9_0`.
- [ ] No remaining `agent_client_protocol::CollectedResponse` references in `claude-agent/src/lib.rs`.

## Tests
- [ ] No new tests added in this task. The previously-disabled tests will start running under subsequent tasks (and will mostly fail because the rest of the crate hasn't been migrated yet — that's expected).

## Workflow
- Pure hygiene. Don't migrate any ACP API usage in this task.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike).