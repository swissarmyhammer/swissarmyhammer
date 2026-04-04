---
assignees:
- claude-code
position_column: todo
position_ordinal: be80
title: Replace title= with shadcn Tooltip in entity-card.tsx
---
## What

`kanban-app/ui/src/components/entity-card.tsx` uses HTML `title=` attribute on the inspect button:

- Line 180: `title="Inspect"` on the info button

Replace with `<Tooltip><TooltipTrigger asChild>...<TooltipContent>` from `@/components/ui/tooltip`.

**Files to modify:**
- `kanban-app/ui/src/components/entity-card.tsx` — import Tooltip components, wrap the inspect button
- `kanban-app/ui/src/components/entity-card.test.tsx` — update test selectors that query `button[title='Inspect']` (lines 179, 203) to use the new markup

## Acceptance Criteria
- [ ] Inspect button shows styled Radix tooltip on hover
- [ ] No HTML `title=` attributes remain in entity-card.tsx
- [ ] Existing tests updated and passing

## Tests
- [ ] Update `entity-card.test.tsx` selectors for the new Tooltip markup
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: hover over inspect button on a task card, verify styled tooltip #tooltip