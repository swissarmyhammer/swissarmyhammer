---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: Replace title= with shadcn Tooltip in entity-card.tsx
---
## What\n\nReplace HTML `title=` on inspect button with shadcn Tooltip.\n\n**MANDATORY: TDD — write/update tests FIRST (RED), then implement (GREEN).**\n\n**Files to modify:**\n- `kanban-app/ui/src/components/entity-card.test.tsx` — update selectors from `button[title='Inspect']` to `button[aria-label='Inspect']` FIRST\n- `kanban-app/ui/src/components/entity-card.tsx` — replace `title=` with Tooltip + `aria-label`\n\n## Acceptance Criteria\n- [ ] Tests updated FIRST (RED), then code changed (GREEN)\n- [ ] Styled tooltip on hover\n- [ ] No HTML `title=` in entity-card.tsx\n\n## Tests\n- [ ] Updated entity-card.test.tsx selectors (RED first)\n- [ ] `cd kanban-app/ui && pnpm vitest run entity-card` — all pass"