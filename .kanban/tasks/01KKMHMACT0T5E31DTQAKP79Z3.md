---
assignees:
- assistant
position_column: done
position_ordinal: ffffb080
title: 'Update all markdown/yaml/description.md refs: .swissarmyhammer → .sah'
---
## What
115 occurrences across 22 files. Update README.md, doc/, ideas/, CLI description.md files, statusline config YAML. Leave `com.swissarmyhammer.kanban` bundle ID as-is.

## Files (non-exhaustive)
- `README.md`
- `swissarmyhammer-cli/README.md`
- `doc/src/**/*.md`
- `ideas/**/*.md`
- `swissarmyhammer-cli/src/commands/serve/description.md`
- `swissarmyhammer-cli/src/commands/validate/description.md`
- `swissarmyhammer-cli/src/commands/model/description.md`
- `swissarmyhammer-tools/src/mcp/tools/questions/ask/description.md`
- `swissarmyhammer-statusline/builtin/config.yaml`
- `builtin/statusline/config.yaml`
- `.skills/plan/PLANNING_GUIDE.md`

## Acceptance Criteria
- [ ] `grep -r '.swissarmyhammer' --include='*.md' --include='*.yaml'` returns only `com.swissarmyhammer.kanban`