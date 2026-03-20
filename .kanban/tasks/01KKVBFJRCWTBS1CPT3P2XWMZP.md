---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffff9580
title: 'Dead code: retain_validator_tools() never called'
---
**swissarmyhammer-tools/src/mcp/tool_registry.rs:retain_validator_tools()**\n\nThe `retain_validator_tools()` method was added to `ToolRegistry` but is never called. The `create_validator_server()` method directly registers the two validator tools into a fresh registry instead. The method and its doc comments suggest a filter-from-existing pattern, but the actual implementation takes the register-fresh approach.\n\n**Why this matters:** Dead public API is a maintenance burden and confuses future readers about the intended usage pattern.\n\n**Fix:** Either remove `retain_validator_tools()` or use it in `create_validator_server()` by registering all tools first then filtering. The current direct-registration approach is actually cleaner, so removing the method is the better option.\n\n**Verification:** `cargo clippy` continues clean after removal. Grep for `retain_validator_tools` confirms no callers.