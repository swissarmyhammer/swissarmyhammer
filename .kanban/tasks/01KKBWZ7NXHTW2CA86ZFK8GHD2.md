---
position_column: done
position_ordinal: b7
title: Move code-context background work out of McpServer constructor
---
## What

`McpServer::new_with_work_dir()` (the constructor) eagerly calls `initialize_code_context()` at line 268. This spawns:
- LSP supervisor (rust-analyzer, typescript-language-server)
- Tree-sitter indexing worker (file discovery, parsing, chunk writing)
- CodeContextWorkspace (SQLite database, leader election)
- File watcher for incremental updates

ALL of this is expensive background work. It's an optimization for serving — pre-warming the index so queries are fast. But it fires automatically in the constructor, so every code path that constructs an `McpServer` pays this cost, including `sah init` and `sah doctor`.

**Fix:** Move `initialize_code_context()` from the constructor into `ServerHandler::initialize()` — the MCP lifecycle method called when a client actually connects. Background work starts explicitly when serving, not automatically on construction.

**CLI tool calls (`sah code-context search`) still work without background indexing.** Every code-context operation already calls `open_workspace(context)` internally, which opens the SQLite DB on demand. Without pre-warming, queries work against whatever's in the DB. The background indexing is an optimization, not a dependency.

**Why this is correct for all paths:**
- `sah serve` → client connects → `ServerHandler::initialize()` fires → background work starts ✓
- `sah init` → constructor runs (cheap) → no client → no background work ✓
- `sah doctor` → constructor runs (cheap) → no client → no background work ✓
- `sah code-context search` (CLI) → `execute_tool()` → `open_workspace()` opens DB on demand → queries existing index ✓
- The `std::sync::Once` guard handles concurrent connections (Claude Code opens ~3) ✓

**Files:**
- EDIT: `swissarmyhammer-tools/src/mcp/server.rs`
  - Add `work_dir: PathBuf` field to `McpServer` struct
  - Remove `Self::initialize_code_context(&work_dir)` from `new_with_work_dir()` (line 268)
  - Add `Self::initialize_code_context(&self.work_dir)` to `ServerHandler::initialize()` (after file watching starts, line ~1053)

**Subsumes existing card 01KKBX2F48TNPDQ3KAY3BA850N.**

## Acceptance Criteria
- [ ] `McpServer::new_with_work_dir()` no longer calls `initialize_code_context()`
- [ ] `ServerHandler::initialize()` explicitly calls `initialize_code_context()` when MCP client connects
- [ ] `work_dir` stored on `McpServer` struct
- [ ] `sah init` — no LSP, no indexing, no code-context logs
- [ ] `sah doctor` — no LSP, no indexing, clean output
- [ ] `sah serve` + client connect — background work starts
- [ ] `sah code-context search` via CLI — works, opens workspace on demand

## Tests
- [ ] `cargo test -p swissarmyhammer-tools` passes
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Existing e2e tests in `swissarmyhammer-tools/tests/code_context_mcp_e2e_test.rs` pass
- [ ] Manual: `sah init` — no LSP logs, fast
- [ ] Manual: `sah doctor` — clean output
- [ ] Manual: `sah serve` + connect Claude Code — code-context starts on connection
- [ ] Manual: `sah code-context get-status` via CLI — works, shows whatever is indexed