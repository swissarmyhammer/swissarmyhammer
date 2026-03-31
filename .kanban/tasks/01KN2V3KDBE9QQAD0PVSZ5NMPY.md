---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffc380
title: '[NIT] claude_settings_path missing doc comment'
---
The new `claude_settings_path` helper function (line 411) has a `///` doc comment but it reads as minimal. Per Rust review guidelines, all public items (and in practice, important private helpers) should document parameters and return values. The function should document what each `InitScope` variant maps to, since that mapping is the entire purpose of the function.\n\nThe existing comment says 'Returns the Claude Code settings file path for the given init scope' which is just restating the function signature. Document the three-way mapping explicitly.\n\nFile: `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`, line 411 #review-finding