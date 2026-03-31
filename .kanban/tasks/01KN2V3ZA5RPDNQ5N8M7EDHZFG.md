---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: '[NIT] Duplicate scope-aware settings logic between shell tool and CLI'
---
There is now a `claude_settings_path` function in both:\n- `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` (new, scope-aware)\n- `swissarmyhammer-cli/src/commands/install/settings.rs` (existing, project-only)\n\nThe CLI version only handles project scope. The tool version handles all three scopes. This is duplicated domain knowledge about where Claude settings files live. Consider extracting the scope-aware version to `swissarmyhammer-common` so both call sites share one source of truth.\n\nThis is a nit because the duplication is small and contained, but it matters because the file-path mapping is a correctness invariant -- if it drifts between the two locations, init/deinit in CLI vs tool will target different files. #review-finding