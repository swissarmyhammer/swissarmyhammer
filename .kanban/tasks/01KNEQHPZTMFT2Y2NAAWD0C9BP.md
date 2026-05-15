---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffff9c80
title: Replace title= with shadcn Tooltip in nav-bar.tsx
---
## What\n\nReplace HTML `title=` on inspect and search buttons with shadcn Tooltip.\n\n**MANDATORY: TDD — write/update tests FIRST (RED), then implement (GREEN).**\n\n**Files to modify:**\n- `kanban-app/ui/src/components/nav-bar.test.tsx` — update selectors from `getByTitle` to `getByLabelText`, add hover tooltip assertion\n- `kanban-app/ui/src/components/nav-bar.tsx` — replace `title=` with Tooltip + `aria-label`\n\n## Acceptance Criteria\n- [ ] Tests updated FIRST (RED), then code changed (GREEN)\n- [ ] Styled tooltips on hover\n- [ ] No HTML `title=` in nav-bar.tsx\n\n## Tests\n- [ ] Updated nav-bar.test.tsx selectors (RED first)\n- [ ] `cd kanban-app/ui && pnpm vitest run nav-bar` — all pass"