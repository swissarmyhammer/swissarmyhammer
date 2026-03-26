---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffba80
title: Merge shell skill into tool description
---
Replace `execute/description.md` with the full content from `.skills/shell/SKILL.md` (minus the YAML frontmatter). This makes the tool self-describing — the MCP tool description carries all behavioral guidance (operation docs, usage patterns, timeout/max_lines advice, search vs grep). The current `description.md` is 17 lines; the merged version will include all operation documentation and guidance from the SKILL.md.

**Files changed:**
- `swissarmyhammer-tools/src/mcp/tools/shell/execute/description.md` — rewritten with SKILL.md content

**Verify**: `cargo nextest run -p swissarmyhammer-tools` (description is loaded at compile time via `get_tool_description`)