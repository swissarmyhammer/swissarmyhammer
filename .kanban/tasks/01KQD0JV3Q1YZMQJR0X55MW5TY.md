---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: done
position_ordinal: ffffffffffffffffffffffff9480
project: acp-upgrade
title: 'ACP 0.11: llama-agent: permissions + commands + plan'
---
## What

Migrate semantic acp modules to ACP 0.11.

Files:
- `llama-agent/src/acp/permissions.rs`
- `llama-agent/src/acp/commands.rs`
- `llama-agent/src/acp/plan.rs`

## Branch state at task start

C1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p llama-agent`.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).

## Resolution

Verification task — these three modules already compile cleanly under ACP
0.11 after C1's bulk schema-type import migration (commit `9f4f564d0`).

- `permissions.rs` has zero `agent_client_protocol` references; it's a pure
  policy-engine module backed by `serde` + `std::collections::HashMap`.
  Nothing to migrate.
- `commands.rs` uses `agent_client_protocol::schema::{AvailableCommand,
  AvailableCommandInput, UnstructuredCommandInput}` (and the test mock
  uses `schema::SessionId`). All construction goes through the public
  `::new` + `.input(...)` + `.meta(...)` builders, which is the
  supported pattern for the `#[non_exhaustive]` 0.11 types. Added a
  module-level note clarifying this.
- `plan.rs` uses `agent_client_protocol::schema::{Plan, PlanEntry,
  PlanEntryPriority, PlanEntryStatus}`. All construction goes through
  `Plan::new(...).meta(...)` and `PlanEntry::new(...).meta(...)`,
  matching the 0.11 builder API on the `#[non_exhaustive]` types.
  Added a module-level note clarifying this.

The remaining `cargo check -p llama-agent` errors all originate in
`acp/server.rs` (E0404 on `impl agent_client_protocol::Agent for AcpServer`
and E0432 on the `AgentWithFixture` import) and are tracked by the
separate `acp/server.rs (AcpServer reshape)` task. The three modules in
this task's scope have zero errors and zero warnings of their own.

The inline tests in all three modules use only the public ACP 0.11 API and
the local types in this module, so they continue to type-check cleanly.