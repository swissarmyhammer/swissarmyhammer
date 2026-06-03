---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: agent-builtins
title: 'Spike: rmcp in-memory handoff API + shelltool-cli role'
---
De-risk the two unknowns the build depends on. Findings feed the llama-agent mount and wiring-tier cards.

## Investigate
1. **rmcp in-memory duplex serve.** Determine the exact rmcp API for serving a `RoleServer` handler and connecting a `RoleClient` over `tokio::io::duplex` in-process (no subprocess, no port). Confirm what concrete server-handle/handler type `swissarmyhammer-tools` can produce (today it serves registries via rmcp in `crates/swissarmyhammer-tools/src/mcp/unified_server.rs`) and what value type `llama-agent` (`crates/llama-agent/src/mcp.rs`, `UnifiedMCPClient`) must accept to mount it. The goal is a clean *value handoff* of a served registry across the crate boundary, not a socket.
2. **shelltool-cli.** Document what `apps/shelltool-cli` is today (standalone MCP server? what does it expose?) and whether any Bash-suppression wiring for Claude already exists there.

## Done when
- A written note (attach or in card comments) naming the rmcp types/functions for duplex serve+connect, the handler type tools will expose, and the llama-agent constructor input type.
- shelltool-cli role + any existing deny-Bash mechanism documented.
- No code changes required.