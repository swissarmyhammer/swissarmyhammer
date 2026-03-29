---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff9f80
title: 'entity-card: show field caption icons on card view fields'
---
## What

Fields on entity cards in board view should show their field caption icons (from the YAML field definition `icon` property). Currently fields render without icons — just the value.

### Files to modify
- `kanban-app/ui/src/components/entity-card.tsx` or Field display components

## Acceptance Criteria
- [ ] Each field on the card shows its configured icon
- [ ] Icons match what's shown in the inspector

## Tests
- [ ] Manual smoke test
- [ ] Zero type errors"