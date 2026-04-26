---
position_column: done
position_ordinal: e980
title: Extract McpRegistration as Initializable component
---
## What

Extract `install_mcp_all_agents()` and its legacy fallback from the monolithic `init.rs` into a standalone `McpRegistration` struct implementing `Initializable`.

- `init`: Registers `sah serve` as MCP server in all detected agent configs (Claude Code, Cursor, Windsurf, etc.) via mirdan. Falls back to legacy `.mcp.json` if no agents detected.
- `deinit`: Unregisters `sah` from all agent MCP configs.
- Priority: 10 (first — everything else depends on MCP being registered)
- `is_applicable`: Always true (all scopes)

**Files:**
- NEW: `swissarmyhammer-cli/src/commands/install/components/mod.rs`
- NEW: `swissarmyhammer-cli/src/commands/install/components/mcp_registration.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — remove `install_mcp_all_agents()` and `install_project_legacy()`, register `McpRegistration` in registry instead

## Acceptance Criteria
- [ ] `McpRegistration` struct implements `Initializable`
- [ ] `init()` produces same MCP config files as current `install_mcp_all_agents()`
- [ ] `deinit()` removes MCP config entries
- [ ] Legacy `.mcp.json` fallback preserved
- [ ] Old functions removed from `init.rs`

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init` still writes MCP config to detected agents