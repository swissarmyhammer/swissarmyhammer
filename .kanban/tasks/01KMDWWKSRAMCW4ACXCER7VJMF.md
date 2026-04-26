---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffed80
title: 'entity-card: clicking fields should select/edit inline, not open inspector'
---
## What

Clicking on fields displayed on entity cards (board view) opens the inspector panel instead of making the field editable inline on the card. Fields on cards should be directly editable with a click — the inspector should only open via explicit inspect action (double-click, context menu, or keyboard shortcut).

### Files to investigate
- `kanban-app/ui/src/components/entity-card.tsx` — click handling, field rendering
- `kanban-app/ui/src/components/focus-scope.tsx` — may be intercepting clicks for inspect

## Acceptance Criteria
- [ ] Clicking a field on an entity card enters edit mode for that field
- [ ] Inspector opens only via explicit inspect action
- [ ] Double-click or context menu still opens inspector

## Tests
- [ ] Manual smoke test
- [ ] Zero type errors"