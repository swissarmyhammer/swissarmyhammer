---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
title: Replace title= with shadcn Tooltip in board-selector.tsx
---
## What\n\nReplace HTML `title=` on tear-off button with shadcn Tooltip.\n\n**MANDATORY: TDD — write/update tests FIRST (RED), then implement (GREEN).**\n\n**Files to modify:**\n- `kanban-app/ui/src/components/board-selector.test.tsx` — add test asserting aria-label and tooltip FIRST\n- `kanban-app/ui/src/components/board-selector.tsx` — replace `title=` with Tooltip + `aria-label`\n\n## Acceptance Criteria\n- [ ] Tests updated FIRST (RED), then code changed (GREEN)\n- [ ] Styled tooltip on hover\n- [ ] No HTML `title=` in board-selector.tsx\n\n## Tests\n- [ ] Test written/updated FIRST (RED)\n- [ ] `cd kanban-app/ui && pnpm vitest run board-selector` — all pass"