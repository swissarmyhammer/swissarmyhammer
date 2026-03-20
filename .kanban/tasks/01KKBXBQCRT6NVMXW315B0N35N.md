---
position_column: done
position_ordinal: e780
title: Extract ClaudeLocalScope as Initializable component
---
## What

Extract `install_claude_local_scope()` from monolithic `init.rs` into a standalone `ClaudeLocalScope` struct implementing `Initializable`.

- `init`: Writes MCP server config to `~/.claude.json` under `projects.<project-path>.mcpServers`
- `deinit`: Removes the MCP server entry from the local scope config
- Priority: 11 (right after MCP registration)
- `is_applicable`: Only when scope is `Local`

**Files:**
- NEW: `swissarmyhammer-cli/src/commands/install/components/claude_local_scope.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/components/mod.rs`
- EDIT: `swissarmyhammer-cli/src/commands/install/init.rs` — remove `install_claude_local_scope()`

## Acceptance Criteria
- [ ] `ClaudeLocalScope` implements `Initializable`
- [ ] `is_applicable()` returns false for non-Local scopes
- [ ] `init()` writes same config as current `install_claude_local_scope()`
- [ ] `deinit()` removes the config entry
- [ ] Old function removed from `init.rs`

## Tests
- [ ] `cargo test -p swissarmyhammer-cli` passes
- [ ] Manual: `sah init --target local` writes to `~/.claude.json`