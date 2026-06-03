---
assignees:
- claude-code
depends_on:
- 01KT57CYY7P8VXA6JXBNJTNRF4
- 01KT57BTE05BAFGYEJHGC7MBR8
position_column: todo
position_ordinal: '8680'
project: agent-builtins
title: 'wiring tier: build Agent registry from tools, hand to llama-agent unconditionally'
---
The tier that sees both `tools` and `llama-agent` (the `swissarmyhammer-agent` singular crate and/or the CLI) constructs the Agent-tools registry and passes it into llama-agent's required constructor input — every time, regardless of session config.

## Change
- At llama-agent executor construction, build the Agent registry (the `category() == Agent`/Replacement tools: files, web, skill, agent, shell) from `swissarmyhammer-tools` and produce the rmcp handler value llama-agent's mount accepts (per spike).
- Pass it unconditionally. There is no code path that constructs a SAH llama-agent without its Agent tools.
- This is the only tier permitted to depend on both crates; it preserves the acyclic graph (`tools` and `llama-agent` are siblings; content flows down).

## Layering guard
- Do NOT reintroduce a `tools → llama-agent` runtime dependency. The wiring lives above both. (The phantom `tools → llama-agent`/`claude-agent` deps were deleted; keep them gone.)
- Agent orchestration stays above `tools`; `tools` remains a pure provider.

## Depends on
- llama-agent mount card (the constructor input contract).
- Category metadata card (defines the Agent set).

## Done when
- Real llama-agent executor construction (CLI/`swissarmyhammer-agent`) yields an agent whose Agent tools are present with no external MCP servers configured.
- `cargo build` of the workspace stays green; no dependency cycle.