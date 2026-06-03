---
assignees:
- claude-code
depends_on:
- 01KT57BTE05BAFGYEJHGC7MBR8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd380
project: agent-builtins
title: 'tools: per-client served-set composition at the serve boundary'
---
The SAH MCP server composes the registry it advertises per connecting client, off the MCP `initialize` clientInfo — not a static union, not a config-guessed flag.

## Behavior
Read the client `Implementation` (name) from the rmcp `initialize` handshake on the server side.
- **Claude client** → serve `Shared ∪ Replacement` (i.e. Shared + `Shell`). Agent tools NOT served (Claude has natives).
- **llama-agent client** → serve `Shared` only. Agent + Replacement are llama's in-memory built-ins; serving shell here would duplicate it.
- **Unknown/other client** → default policy (decide: Shared only, conservative).

## Why
`Replacement` is the flag that makes an Agent tool *also* reach the native-host. Shell reaches each host exactly once: built-in for llama, SAH-served for Claude. This card handles the *selection*; the paired Bash-deny card handles suppressing Claude's native.

## Implementation notes
- Server side: rmcp exposes client info via the initialize params / peer info — confirm the access point in `crates/swissarmyhammer-tools/src/mcp/server.rs` / `unified_server.rs`.
- Composition reads each tool's `category()` (from the metadata card).
- Map clientInfo name → host identity (Claude vs llama) — keep the mapping in one place; reused by the Bash-deny card.

## Done when
- A Claude client sees Shared + Shell and none of the other Agent tools.
- A llama client sees Shared only.
- Selection is driven by `category()` + clientInfo, with no `agent_mode`.