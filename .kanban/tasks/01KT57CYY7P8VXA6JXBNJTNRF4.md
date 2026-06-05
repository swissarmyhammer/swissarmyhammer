---
assignees:
- claude-code
depends_on:
- 01KT57BGTASD8W45HE708FM01R
- 01KT57BTE05BAFGYEJHGC7MBR8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd580
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

## Review Findings (2026-06-03 08:36)

Overall: high-quality, well-documented work. The four hardest risk areas verified correct against source — `compose_per_client = false` is set and load-bearing (`Host::serves(Agent)` is `false` for every host, so per-client filtering would serve zero tools); `tokio::join!` drives the duplex handshake concurrently; `MountedClient` ties the `RunningService` serve-task lifetime to the client (confirmed in rmcp 1.7.0 that `RunningService` holds a `DropGuard` that cancels the serve task on drop, so teardown is clean with no leaked task); and cycle-freedom holds (`swissarmyhammer-tools` is a `[dev-dependencies]` entry in llama-agent, no concrete tools type named in production source). Builds clean, clippy clean (incl. tests), and the real-path test `agent_tools_mount_lists_intrinsic_tools_with_no_external_servers` passes. One genuine warning and two nits below.

### Warnings
- [x] `crates/llama-agent/src/acp/server.rs:1725-1736` — Mount `connect()` failure is swallowed: the `Err` arm only logs `tracing::error!` and falls through, so `new_session` still succeeds and creates a session with ZERO tools. This contradicts the card's load-bearing invariant ("an empty `session/new` server list MUST still yield a fully-tooled agent") AND the code's own comment two lines above it ("The mount is intrinsic; failing to mount it leaves the agent without its base tools, so this is an error, not a warning"). The comment names the right principle; the code does the opposite. The mount being required at *construction* (non-Option `Arc<dyn AgentToolsMount>`, CLI/wiring fail hard if `build_agent_tools_mount()` fails) does not cover a *runtime per-session* connect failure. The author appears to have mirrored the external-server loop's "log and continue" pattern (correct for optional external servers, wrong for the intrinsic mount). Fix: on the mount's `connect()` error, `return Err(Self::convert_error(...))` (or the ACP equivalent) so session creation fails loudly rather than yielding a tool-less agent. In practice an in-process duplex connect is very unlikely to fail, but the code asserts a contract it does not enforce.

### Nits
- [x] `crates/llama-agent/src/acp/server.rs:1742-1746` (and `:108`) — The `compose_per_client` field doc on `McpServer` (server.rs in the tools crate, struct def) still describes only "the full server" vs "the validator server" and does not mention the new agent-tools server, which is the third `compose_per_client = false` producer. Add a clause so the doc enumerates all verbatim-served registries. (Stale-by-omission documentation touched by this change.)
- [x] `crates/llama-agent/tests/integration/mod.rs:4` — `mod agent_tools_mount;` is inserted between `acp_agentic_loop` and `acp_config_file`, breaking the otherwise-alphabetical module list. Minor ordering nit. Separately, the tool-aggregation in `new_session` (`all_tools.extend(...)`) has no name-collision dedup across clients; benign today because llama gets shell only from the mount and external servers serve Shared-only, but worth a one-line comment noting the assumption so a future external server exposing a duplicate tool name doesn't silently double-register.