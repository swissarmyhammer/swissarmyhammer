---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd480
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

## Spike Findings (2026-06-03)

**rmcp version:** 1.7.0. Source: `~/.cargo/registry/src/index.crates.io-*/rmcp-1.7.0/`.

### Serve a handler over duplex
`tokio::io::DuplexStream` satisfies `IntoTransport<RoleServer, std::io::Error, _>` directly (blanket impl for `AsyncRead+AsyncWrite+Send+'static` at `rmcp-1.7.0/src/transport/async_rw.rs:37`; rmcp splits internally). **Pass the `DuplexStream` whole — no manual split.**
- `rmcp::serve_server(service, transport) -> Result<RunningService<RoleServer, S>, ServerInitializeError>` (`src/service/server.rs:114`), or equivalently `service.serve(transport)` (`ServiceExt`).
- Recipe: `let (server_half, client_half) = tokio::io::duplex(8*1024); let running = serve_server(agent_server, server_half).await?;`

### Connect a client over duplex
Mirror of llama-agent's existing pattern (`crates/llama-agent/src/mcp.rs` uses `handler.clone().serve(transport)` for `RoleClient`). Add one constructor, e.g. `UnifiedMCPClient::with_duplex(client_half, handler, timeout)` doing `handler.clone().serve(client_half).await`. `DuplexStream` is a valid `RoleClient` transport too. No new transport plumbing.

### SAH's ServerHandler type
`swissarmyhammer_tools::mcp::server::McpServer` (`server.rs:79`, `#[derive(Clone)]`, `impl ServerHandler` at `server.rs:1658`). **Build the agent-tools instance via the `create_validator_server` pattern (`server.rs:722`)**: fresh `ToolRegistry` with only the chosen tools, cloned `ToolContext`, new `McpServer`. Add a sibling `create_agent_tools_server` registering the `Agent`-category tools (files incl read_file/glob/grep, web, skill, agent/subagent). `shell` is `Replacement{native:"Bash"}`.

### ⚠️ #1 TRAP — must set `compose_per_client = false` on the mounted instance
`ServerHandler::list_tools` (`server.rs:1792`) branches on `compose_per_client`. The card-#2 `Host::serves` table (`host.rs:80`) returns **`Agent => false` for EVERY host including Llama**. So serving the agent-tools instance with `compose_per_client = true` returns ZERO tools to llama. The mounted instance MUST be `compose_per_client = false` (verbatim, like the validator server).

### Recommended llama-agent constructor input (cycle-free)
Hand llama-agent a `tokio::io::DuplexStream` (client half) already wired to a serve task spawned in the tier above. `DuplexStream` is plain tokio/rmcp — adds ZERO new dep, never names a `swissarmyhammer-tools` type, preserves the legal `tools → llama-agent` direction. (Rejected: passing `Box<dyn DynService<RoleServer>>` / `RunningService` — leaks rmcp role types and still needs the server built somewhere.)

### In-process pairing helper
None in rmcp 1.7.0. Use the manual `tokio::io::duplex(N)` recipe above (rmcp's own tests do this at `async_rw.rs:628`).

### shelltool-cli
`apps/shelltool-cli` — standalone **stdio MCP server exposing ONLY the shell tool** (`ShellToolServer`, `src/commands/serve.rs:25`; identity `Implementation::new("shelltool", …)`). `shelltool serve` = `serve_server(ShellToolServer::new(), stdio())`. `shelltool init/deinit` registers it as an MCP server via mirdan. **Bash-deny ALREADY EXISTS**: `ShellExecuteTool`'s `Initializable::init` calls `mirdan::install::deny_tool(scope, "Bash", reporter)` (`crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:405`; `deinit` → `allow_tool`). Same path as the CLI `DenyBash` component (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:118,151`). → **Card #6 (serve-time deny) must reuse `mirdan::install::deny_tool`** (`crates/mirdan/src/install.rs:1725`), which writes `/permissions/deny` idempotently via per-agent strategy (Claude impl writes the array; no-permission agents are silent no-ops).

### Risks / gotchas for implementers
1. **compose_per_client=false** on the mount (see trap above).
2. **Keep the serve task alive** — `serve_server().await` returns a `RunningService<RoleServer>`; if it/its JoinHandle drops, the transport closes and `tools/list` fails. Store it in the wiring tier; `.cancel()` on shutdown.
3. **Duplex buffer** — use 8–64 KiB (tool payloads can be large). Small buffer = backpressure, not deadlock, as long as both serve+client tasks run concurrently.
4. **Handshake ordering** — spawn the server serve future BEFORE awaiting the client `.serve()`, or `tokio::join!` them, else initialize hangs.
5. **Tool-name collision / precedence** — the mounted client is just one more `UnifiedMCPClient` in llama-agent's aggregation (`MCPClientBuilder`, `mcp.rs:527`). No dedup today. If an ACP-provided server also exposes `shell`/`files`, decide precedence explicitly (relevant to the "shell appears exactly once" invariant — llama gets shell from the mount, so a SAH server it also connects to must NOT also serve shell; card #2 already serves llama Shared-only, so that side is covered).
6. **Server identity** — `list_tools_with_schemas` labels tools by `peer_info().server_info.name` (`mcp.rs:256`); confirm `McpServer::get_info` name is acceptable for the mounted instance.