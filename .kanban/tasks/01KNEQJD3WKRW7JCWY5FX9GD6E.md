---
assignees:
- claude-code
position_column: todo
position_ordinal: '8880'
title: Replace title= with shadcn Tooltip in avatar.tsx
---
## What\n\nReplace HTML `title=` on both avatar variants with shadcn Tooltip.\n\n**MANDATORY: TDD — write/update tests FIRST (RED), then implement (GREEN).**\n\n**Files to modify:**\n- `kanban-app/ui/src/components/avatar.test.tsx` — update selectors from `title` to `aria-label`, add tooltip hover assertion FIRST\n- `kanban-app/ui/src/components/avatar.tsx` — replace `title=` with Tooltip + `aria-label`\n\n## Acceptance Criteria\n- [ ] Tests updated FIRST (RED), then code changed (GREEN)\n- [ ] Styled tooltip with actor name on hover\n- [ ] No HTML `title=` in avatar.tsx\n- [ ] Avatar layout not disrupted\n\n## Tests\n- [ ] Test written/updated FIRST (RED)\n- [ ] `cd kanban-app/ui && pnpm vitest run avatar` — all pass"