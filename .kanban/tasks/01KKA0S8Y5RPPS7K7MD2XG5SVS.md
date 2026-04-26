---
position_column: done
position_ordinal: b380
title: Wire LspSupervisor into code-context startup
---
## What

`swissarmyhammer-lsp` has a fully implemented `LspSupervisorManager` and `LspDaemon` with health checks and exponential backoff restart. BUT nothing in the code-context startup path uses them. The supervisor is never started, so no LSP servers are ever spawned.

LSP servers like rust-analyzer take 30-120s to warm up. They MUST be launched at MCP startup in parallel with tree-sitter indexing, not after it. By the time tree-sitter finishes, the LSP servers should be ready for symbol collection.

**Key files:**
- `swissarmyhammer-lsp/src/supervisor.rs` — `LspSupervisorManager` (exists, works)
- `swissarmyhammer-lsp/src/daemon.rs` — `LspDaemon` with spawn/health/restart (exists, works)
- `swissarmyhammer-tools/src/mcp/server.rs` — `initialize_code_context` needs to start the supervisor
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — needs to use supervisor for symbol collection

**Approach:**
1. In `initialize_code_context`, start `LspSupervisorManager` FIRST (before tree-sitter scan)
2. Store supervisor handle in a `OnceLock<Arc<LspSupervisorManager>>` static for query-time use
3. Tree-sitter indexing and LSP server warmup run concurrently
4. After tree-sitter scan completes AND LSP servers are ready, run LSP symbol collection
5. Graceful shutdown on process exit

## Acceptance Criteria
- [ ] `LspSupervisorManager` starts at MCP init, in parallel with tree-sitter scan
- [ ] rust-analyzer spawned for Rust workspaces (if installed)
- [ ] `get status` shows LSP server state (Running/Failed/NotFound)
- [ ] LSP warmup overlaps with tree-sitter indexing (no sequential wait)
- [ ] Supervisor shutdown on process exit

## Tests
- [ ] `cargo test -p swissarmyhammer-lsp` passes
- [ ] Manual: restart MCP, verify rust-analyzer process spawns within seconds
- [ ] Manual: `get status` shows LSP server info"
