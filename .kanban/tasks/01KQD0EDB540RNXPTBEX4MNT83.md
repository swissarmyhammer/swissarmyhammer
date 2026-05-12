---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: done
position_ordinal: ffffffffffffffffffffffff9b80
project: acp-upgrade
title: 'ACP 0.11: avp-common: housekeeping + bulk schema imports + executor.rs StopReason'
---
## What

Mechanical schema-type import migration in `avp-common/`, plus the small `StopReason` use in `validator/executor.rs`.

Files:
- `avp-common/src/validator/executor.rs` — uses `StopReason` (e.g. `StopReason::EndTurn`); imports move to `agent_client_protocol::schema::StopReason`. The construction shape itself is unchanged.
- Bulk `agent_client_protocol::X` → `agent_client_protocol::schema::X` rename across the rest of `avp-common/src/` and `avp-common/tests/` for non-Agent types.

Note: the production `impl Agent for AvpContext` in `src/context.rs` and the mock `impl Agent` in `src/validator/runner.rs` are **out of scope** here — those are separate tasks (D2, D3).

## Branch state at task start

`acp/0.11-rewrite` with commit `d5b5465bd`.

## Acceptance Criteria
- [ ] Bulk schema-type import rename complete in `avp-common/`.
- [ ] `validator/executor.rs` compiles in isolation against the new `StopReason` schema path. (Other modules will still fail until D2/D3 land.)
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] No new tests; mechanical rename.

## Workflow
- Mirror of B1/C1.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike).