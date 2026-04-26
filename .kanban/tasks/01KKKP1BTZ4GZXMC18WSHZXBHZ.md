---
position_column: done
position_ordinal: ffad80
title: Remove js MCP tool registration and module
---
## What

Delete the `js` MCP tool and remove all its wiring from the tool registration system. The `swissarmyhammer-js` crate stays — only the MCP tool exposure is removed.

**Files to delete:**
- `swissarmyhammer-tools/src/mcp/tools/js/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/js/description.md`

**Files to edit (remove `register_js_tools` / `pub mod js`):**
- `swissarmyhammer-tools/src/mcp/tools/mod.rs` — remove `pub mod js;`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` — remove `register_tool_category!(register_js_tools, js, ...)` and call in test setup
- `swissarmyhammer-tools/src/mcp/server.rs` — remove import and `register_js_tools()` call
- `swissarmyhammer-tools/src/mcp/mod.rs` — remove `register_js_tools` from re-exports
- `swissarmyhammer-tools/src/lib.rs` — remove `register_js_tools` from re-exports
- `swissarmyhammer-tools/src/health_registry.rs` — remove import and `register_js_tools()` call

**Cargo.toml:** Check if `swissarmyhammer-tools/Cargo.toml` still needs the `swissarmyhammer-js` dep after removal. If nothing else in swissarmyhammer-tools imports it directly, remove the dep line (fields has its own).

## Acceptance Criteria
- [ ] `pub mod js` no longer exists in tools/mod.rs
- [ ] `register_js_tools` no longer exists anywhere in the codebase
- [ ] `js` tool directory is deleted
- [ ] `cargo build` succeeds with no warnings related to this change
- [ ] `cargo test` passes

## Tests
- [ ] `cargo build -p swissarmyhammer-tools` compiles cleanly
- [ ] `cargo test -p swissarmyhammer-tools` passes — no test references JsTool
- [ ] MCP tool list no longer includes "js"