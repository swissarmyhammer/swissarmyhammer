---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9880
title: '[WARNING] No unit tests for claude_settings_path helper'
---
The kanban card for this work explicitly lists tests as acceptance criteria:\n- Unit test: `claude_settings_path` returns correct path for each `InitScope` variant\n- Integration test: `init()` with `Local` scope writes Bash deny to `.claude/settings.local.json`\n- Integration test: `deinit()` with `Local` scope removes Bash deny from `.claude/settings.local.json`\n\nNone of these tests exist. The `claude_settings_path` function is pure logic with three branches (Project, Local, User) and is an ideal unit-test target. Without tests, regressions on this exact bug could recur.\n\nFile: `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`, lines 411-422 #review-finding