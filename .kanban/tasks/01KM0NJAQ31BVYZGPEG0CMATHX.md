---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffca80
title: 'nit: `max_lines` default changed silently from 200 to 32'
---
swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs (line 167)\n\nThe code reads:\n```rust\nlet raw_max_lines = request.max_lines.unwrap_or(32);\n```\n\nThe description.md says the default is 200. The schema `ParamMeta` description also says \"default: 200\". But the actual runtime default baked into the code is 32. This discrepancy is a correctness issue for callers who don't supply `max_lines` and expect 200 lines per the documented API.\n\nThis appears to be a pre-existing bug carried through the refactoring, not introduced by it — but it should be flagged because the refactoring touched this code and did not fix it.\n\nSuggestion: Change the fallback to `unwrap_or(200)` to match the documented and schema-advertised default, or update the description.md and ParamMeta description to say 32." #review-finding