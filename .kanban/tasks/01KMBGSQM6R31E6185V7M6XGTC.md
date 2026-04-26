---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffd480
title: 'entity-card.tsx: CardFieldDispatch uses anonymous inline prop type'
---
**File:** `kanban-app/ui/src/components/entity-card.tsx:126`\n\n`CardFieldDispatch` has 5 props defined inline. Extract to `interface CardFieldDispatchProps`. #props-slop