---
assignees:
- claude-code
depends_on:
- 01KT57BGTASD8W45HE708FM01R
- 01KT57BTE05BAFGYEJHGC7MBR8
position_column: todo
position_ordinal: '8480'
project: agent-builtins
title: 'llama-agent: mandatory in-memory Agent-tools mount over tokio duplex'
---
llama-agent always mounts its Agent tools as a first-class in-process rmcp server — unconditional, separate from the session's external MCP server list.

## Invariant (load-bearing)
llama-agent has its Agent tools (files, web, skill, agent, **shell**) even when provided ZERO external MCP servers. The Agent tools are intrinsic to being a llama-agent, not "servers it connects to". An empty `session/new` server list MUST still yield a fully-tooler agent.

## Change
- Add a mounting mechanism in `crates/llama-agent/src/mcp.rs`: serve a provided rmcp `RoleServer` handler over `tokio::io::duplex` in-process and connect a `RoleClient` to it; surface its tools in the aggregated tool list as an always-on entry, distinct from the ACP-provided servers (`MCPClientBuilder`/`Vec<(String, UnifiedMCPClient)>`).
- Make the Agent-tools handler a **required construction input** (not Option) so a llama-agent cannot be built without its Agent tools. The external MCP server list stays separate and nullable.
- llama-agent depends ONLY on rmcp's handler trait — never on `swissarmyhammer-tools` (no cycle). It mounts whatever handler it's handed; it does not know the concrete tools.

## Depends on
- Spike card for the exact rmcp duplex serve/connect API and the handler value type.
- Category metadata card (defines what's Agent).

## Done when
- Constructing a llama-agent with an empty external server list still lists files/web/skill/agent/shell.
- The Agent mount is in-memory (no subprocess/port) and goes through rmcp `tools/list` like any MCP server.