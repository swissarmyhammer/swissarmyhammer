---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8780
title: Real Doctorable impl for ShellExecuteTool
---
Replace `impl_empty_doctorable!(ShellExecuteTool)` with a real impl that returns meaningful health checks.\n\n## Health checks\n- Builtin config parses successfully\n- All deny/permit regex patterns compile\n- User config (~/.shell/config.yaml) loads if present\n- Project config (.shell/config.yaml) loads if present\n- Bash is denied in .claude/settings.json permissions.deny\n- Shell skill is deployed (check symlink exists in agent skill dirs)\n\n## Files\n- swissarmyhammer-tools/src/mcp/tools/shell/mod.rs\n\n## Acceptance\n- `sah doctor` shows shell-specific health checks\n- Bad config produces Warning/Error checks with fix suggestions