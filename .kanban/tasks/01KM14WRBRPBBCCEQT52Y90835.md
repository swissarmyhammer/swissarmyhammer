---
assignees:
- claude-code
depends_on:
- 01KM14VK8C3ZJ301YEQHZN754W
position_column: done
position_ordinal: ffffffffffb980
title: Hot reload tool config on list_tools calls
---
## What

Make the running MCP server pick up `tools.yaml` changes without restart. When `list_tools()` is called (Claude Code calls this frequently), re-read the config from disk if the file has changed.

**Approach:** Check file mtime on each `list_tools()` call. If changed since last load, re-read and update the disabled set. The mtime check is cheap (single stat syscall). Only re-parse YAML when the file actually changed.

**Implementation:**
- Add `tool_config_mtime: Option<SystemTime>` and `tool_config_path: Option<PathBuf>` to `ToolRegistry` (or a separate `ToolConfigWatcher` struct held by `McpServer`)
- In `ServerHandler::list_tools()` on `McpServer`, before returning tools, check if config file changed and reload if so
- This means `sah tool shell disable` writes the file, and the next `list_tools()` call picks it up

**Files:**
- `swissarmyhammer-tools/src/mcp/tool_config.rs` — add mtime tracking + reload logic
- `swissarmyhammer-tools/src/mcp/server.rs` — call reload check in `list_tools()`

## Acceptance Criteria
- [ ] Modifying tools.yaml while server is running takes effect on next list_tools call
- [ ] No reload when file hasn't changed (mtime check short-circuits)
- [ ] Deleted tools.yaml reverts to all-enabled
- [ ] File parse errors during hot reload are logged as warnings, previous state kept

## Tests
- [ ] Unit test: write config → reload → verify disabled set changed
- [ ] Unit test: same mtime → no re-parse (verify by checking call count or mtime field)
- [ ] `cargo nextest run -p swissarmyhammer-tools` #tool-filter