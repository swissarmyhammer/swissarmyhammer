---
assignees:
- claude-code
depends_on:
- 01KQD0EA8Q367A70AWY8MMZ79D
position_column: todo
position_ordinal: ff9680
project: acp-upgrade
title: 'ACP 0.11: llama-agent: mcp_client_handler + mcp'
---
## What

Migrate llama-agent's MCP wiring modules to ACP 0.11.

Files:
- `llama-agent/src/mcp_client_handler.rs`
- `llama-agent/src/mcp.rs`

## Branch state at task start

C1 landed.

## Acceptance Criteria
- [ ] These modules compile under `cargo check -p llama-agent`.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests pass.

## Depends on
- 01KQD0EA8Q367A70AWY8MMZ79D (C1).