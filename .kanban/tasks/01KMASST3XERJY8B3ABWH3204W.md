---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8880
title: 'cm-tag-*.ts: hardcoded # prefix in 4 CodeMirror plugins'
---
**Files:**\n- `kanban-app/ui/src/lib/cm-tag-autocomplete.ts:16`\n- `kanban-app/ui/src/lib/cm-tag-decorations.ts:7`\n- `kanban-app/ui/src/lib/cm-tag-tooltip.ts:7`\n- `kanban-app/ui/src/lib/tag-finder.ts:20`\n\nAll hardcode `\"#\"` as the tag mention prefix. Entity definitions already declare `mention_prefix` — these should read from schema. #field-special-case