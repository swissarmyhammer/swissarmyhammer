---
severity: error
tags:
- migration
- cleanup
- mcp-tools
---

# No Memo MCP Tools

## Description

All memo-related MCP tools must be completely removed from the codebase. The memos system has been replaced by rules + todos.

## Acceptance Criteria

### Tool Directory Removed
- `swissarmyhammer-tools/src/mcp/tools/memoranda/` directory does not exist
- All tool implementation files are deleted:
  - `create/mod.rs`
  - `get/mod.rs`
  - `list/mod.rs`
  - `get_all_context/mod.rs`
  - `mod.rs`

### Type Definitions Removed
- `swissarmyhammer-tools/src/mcp/memo_types.rs` file does not exist

### Tool Handlers Removed
- `swissarmyhammer-tools/src/mcp/tool_handlers.rs` contains no memo-related methods:
  - No `handle_memo_create()` method
  - No `handle_memo_get()` method
  - No `handle_memo_list()` method
  - No `handle_memo_get_all_context()` method
  - No `handle_memo_update()` method
- No imports from `memo_types` module

### Registration Removed
- No calls to `register_memo_tools()` anywhere in the codebase
- No export of `register_memo_tools` from `swissarmyhammer-tools/src/lib.rs`
- `register_memo_tools()` function does not exist in `tool_registry.rs`

### Module Declarations Removed
- `swissarmyhammer-tools/src/mcp/tools/mod.rs` does not contain `pub mod memoranda;`
- `swissarmyhammer-tools/src/mcp/mod.rs` does not contain `pub mod memo_types;`

### Tests Updated
- No test files reference `memo_create`, `memo_get`, `memo_list`, or `memo_get_all_context` tools
- All remaining tests pass successfully

### Build Success
- `cargo build` completes without errors
- No warnings about missing memo-related modules
- MCP server starts successfully without memo tools

### No Remaining References
- Search for `memo_create|memo_get|memo_list|memo_get_all_context` in Rust files returns no MCP tool references
- Search for `memoranda` in Rust files returns no tool-related code
- Search for `memo_types` in Rust files returns no imports

## Verification Commands

```bash
# Directory should not exist
! test -d swissarmyhammer-tools/src/mcp/tools/memoranda/

# File should not exist
! test -f swissarmyhammer-tools/src/mcp/memo_types.rs

# No references to memo tools
! rg "register_memo_tools" --type rust
! rg "pub mod memoranda" --type rust swissarmyhammer-tools/src/mcp/tools/mod.rs
! rg "pub mod memo_types" --type rust swissarmyhammer-tools/src/mcp/mod.rs

# Build succeeds
cargo build

# Tests pass
cargo nextest run --fail-fast
```
