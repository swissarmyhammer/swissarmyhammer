---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: Replace title= with shadcn Tooltip in column-view.tsx
---
## What\n\nReplace HTML `title=` on add-task button with shadcn Tooltip.\n\n**MANDATORY: TDD — write/update tests FIRST (RED), then implement (GREEN).**\n\n**Files to modify:**\n- `kanban-app/ui/src/components/column-view.test.tsx` — add test asserting aria-label and tooltip FIRST\n- `kanban-app/ui/src/components/column-view.tsx` — replace `title=` with Tooltip + `aria-label`\n\n## Acceptance Criteria\n- [ ] Tests updated FIRST (RED), then code changed (GREEN)\n- [ ] Styled tooltip with column name on hover\n- [ ] No HTML `title=` in column-view.tsx\n\n## Tests\n- [ ] Test written/updated FIRST (RED)\n- [ ] `cd kanban-app/ui && pnpm vitest run column-view` — all pass"