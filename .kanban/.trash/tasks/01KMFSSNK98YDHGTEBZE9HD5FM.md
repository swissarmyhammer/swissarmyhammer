---
assignees:
- claude-code
position_column: todo
position_ordinal: c380
title: Focus bar position — move left of field icon
---
## What\n\nThe `[data-focused]::before` target bar currently overlaps field content. It needs to sit to the left of the field icon/label, not inside the content area.\n\nAdjust the CSS in `index.css` so the bar is positioned outside the icon column in the inspector layout.\n\n### Files to modify\n- `kanban-app/ui/src/index.css`\n\n## Acceptance Criteria\n- [ ] Focus bar visually appears to the left of the field icon\n- [ ] No content overlap or clipping"