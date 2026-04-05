---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: Replace title= with shadcn Tooltip in perspective-tab-bar.tsx
---
## What\n\nReplace HTML `title=` on add-perspective button with shadcn Tooltip.\n\n**MANDATORY: TDD — write/update tests FIRST (RED), then implement (GREEN).**\n\n**Files to modify:**\n- `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — add test asserting tooltip on hover FIRST\n- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — replace `title=` with Tooltip + `aria-label`\n\n## Acceptance Criteria\n- [ ] Tests updated FIRST (RED), then code changed (GREEN)\n- [ ] Styled tooltip on hover\n- [ ] No HTML `title=` in perspective-tab-bar.tsx\n\n## Tests\n- [ ] Test written/updated FIRST (RED)\n- [ ] `cd kanban-app/ui && pnpm vitest run perspective-tab-bar` — all pass"