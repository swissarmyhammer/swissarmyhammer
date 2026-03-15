---
position_column: done
position_ordinal: a2
title: Fix triple-initialization race condition
---
## What

`initialize_code_context` is called once per MCP server instance. With Claude Code, multiple MCP connections spawn, resulting in 3 concurrent initializations. Two join as reader, one as leader. All three independently scan and try to write to the DB — wasting work and risking write contention.

**Key files:**
- `swissarmyhammer-tools/src/mcp/server.rs` — `initialize_code_context` (line 277+)
- `swissarmyhammer-code-context/src/workspace.rs` — `CodeContextWorkspace::open`

**Approach:** Use an `OnceLock` or `AtomicBool` static to ensure `initialize_code_context` only runs once across all MCP server instances in the same process. The first caller does the work; subsequent callers skip.

## Acceptance Criteria
- [ ] Only one `initialize_code_context` runs regardless of how many MCP connections open
- [ ] Log shows single \"initializing\" message, not three
- [ ] Second/third connections still get working read access to the DB

## Tests
- [ ] `cargo test -p swissarmyhammer-tools` passes
- [ ] Manual: restart MCP, check log shows exactly one initialization