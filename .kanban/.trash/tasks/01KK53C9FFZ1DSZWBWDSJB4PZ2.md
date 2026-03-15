---
position_column: done
position_ordinal: s7
title: Register code_context MCP tool + migrate TS to unified index.db
---
## What
Register `code_context` as an MCP tool alongside the existing `treesitter` tool. Migrate the tree-sitter layer to write into `index.db` instead of `.treesitter-index.db`. Wire all operations through the `McpTool` trait.

Files: `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`, `swissarmyhammer-treesitter/src/unified.rs` (add `with_db()`), `swissarmyhammer-tools/src/mcp/tool_registry.rs`

Spec: `ideas/code-context-architecture.md` — "Migration from treesitter tool" steps 4–7.

## Acceptance Criteria
- [ ] `code_context` tool registered in `ToolRegistry`, responds to all operations in the matrix
- [ ] Tree-sitter `WorkspaceBuilder` gains `with_db()` to write chunks into unified `index.db`
- [ ] Existing `search code`, `query ast`, `find duplicates` work through `code_context` tool
- [ ] `treesitter` tool still works alongside during transition
- [ ] Schema in `code_context` description includes all operations

## Tests
- [ ] Integration test: `code_context` tool responds to `get status`
- [ ] Integration test: `grep code` returns results through the MCP tool interface
- [ ] Integration test: existing `treesitter` tool still works
- [ ] `cargo test -p swissarmyhammer-tools`