---
assignees:
- claude-code
depends_on:
- 01KQ367HE0Z8ZSXY90CTT8QYGG
position_column: done
position_ordinal: ffffffffffffffffffffffff9a80
project: acp-upgrade
title: 'ACP 0.11: llama-agent: bulk schema-type import migration'
---
## What

Mechanical pass over `llama-agent/src/` and `llama-agent/tests/` to migrate schema-type imports from `agent_client_protocol::X` → `agent_client_protocol::schema::X` everywhere those types still exist in 0.11.

Note: `Agent` itself is **not** moved to schema in this task — that's part of the per-module API reshape tasks. This task only moves the *schema* types (see B1 for the type list).

Files affected (per spike + post-merge survey, ~504 ACP refs):
- `llama-agent/src/acp/*.rs` (server, session, translation, terminal, filesystem, permissions, commands, plan, error, config, mod, mcp_client_factory, raw_message_manager, test_utils)
- `llama-agent/src/mcp_client_handler.rs`, `mcp.rs`, `agent.rs`
- `llama-agent/src/types/sessions.rs`
- `llama-agent/src/examples/acp_stdio.rs`
- `llama-agent/tests/acp_integration.rs`, `coverage_tests.rs`, `tests/integration/acp_*.rs`

## Branch state at task start

`acp/0.11-rewrite` with commit `d5b5465bd`.

## Acceptance Criteria
- [ ] No `use agent_client_protocol::X` (where X is a schema type) remains in `llama-agent/src/` or `llama-agent/tests/`.
- [ ] After the bulk rename, `cargo check -p llama-agent --all-targets` produces *fewer* errors than before. (Will not pass yet.)
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] No new tests; mechanical rename.

## Workflow
- Mirror of B1 but for llama-agent.

## Depends on
- 01KQ367HE0Z8ZSXY90CTT8QYGG (spike).