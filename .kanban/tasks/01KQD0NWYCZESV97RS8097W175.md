---
assignees:
- claude-code
depends_on:
- 01KQD0NS3EFZ6Q7WCN5FME36VY
position_column: todo
position_ordinal: ff9e80
project: acp-upgrade
title: 'ACP 0.11: llama-agent: integration tests'
---
## What

Migrate llama-agent integration tests to ACP 0.11.

Files:
- `llama-agent/tests/acp_integration.rs`
- `llama-agent/tests/coverage_tests.rs`
- `llama-agent/tests/integration/acp_slash_command.rs`
- `llama-agent/tests/integration/acp_read_file.rs`
- `llama-agent/tests/integration/acp_write_file.rs`
- `llama-agent/tests/integration/acp_stdio_transport.rs`
- `llama-agent/tests/integration/acp_error_propagation.rs`
- `llama-agent/tests/integration/acp_config_file.rs`
- Other test files under `llama-agent/tests/integration/` that touch ACP types.

## Branch state at task start

C10 (agent.rs + acp_stdio) landed.

## Acceptance Criteria
- [ ] `cargo check -p llama-agent --tests` passes.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] `cargo nextest run -p llama-agent` — green.

## Depends on
- (C10) llama-agent: agent.rs + acp_stdio example.