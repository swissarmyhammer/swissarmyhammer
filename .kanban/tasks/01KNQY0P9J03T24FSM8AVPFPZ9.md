---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: done
position_ordinal: ffffffffffffffffffffffdd80
project: spatial-nav
title: Remove manual claimWhen predicates from inspector and badge-list pills
---
## What

Delete manual predicate construction from the inspector field rows and badge-list pill navigation. These are both linear (1D) layouts that spatial nav handles naturally — fields are stacked vertically, pills are laid out horizontally.

### Files modified

1. **`kanban-app/ui/src/components/entity-inspector.tsx`**:
   - [x] Deleted `useFieldClaimPredicates` hook
   - [x] Deleted `predicatesForField` function
   - [x] Deleted `edgePredicates` function
   - [x] Deleted `isInspectorField` helper
   - [x] Replaced `fieldMonikers` memo with `firstMoniker` (only needed for initial focus)
   - [x] Removed `claimWhen` prop from `FieldRow` and `FocusScope` inside it
   - [x] Removed `claimPredicates` from `InspectorSections` props
   - [x] Removed `ClaimPredicate` import
   - [x] Updated docstrings

2. **`kanban-app/ui/src/components/mention-view.tsx`**:
   - [x] Deleted `buildListClaimPredicates` function
   - [x] Removed `listClaimPredicates` memo
   - [x] Removed `claimWhen` from `MentionViewProps`, `SingleMentionProps`, `SingleMention`, `MentionViewSingle`, `MentionViewList`
   - [x] Removed `ClaimPredicate` import
   - [x] Removed `useParentFocusScope` import (no longer needed)
   - [x] Updated docstrings

3. **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`**:
   - [x] Updated stale comment referencing claimWhen

### Subtasks
- [x] Delete `claimPredicates` memo from entity-inspector.tsx
- [x] Remove `claimWhen` prop from FieldRow and its FocusScope
- [x] Delete `pillClaimPredicates` / `buildListClaimPredicates` from mention-view.tsx
- [x] Remove `claimWhen` from MentionView pill components
- [x] Updated tests to verify FocusScope registration instead of predicate-based nav

## Results
- 146 net lines of predicate code removed from production files
- 1109 tests pass (only pre-existing board-integration.browser.test.tsx failure)
- Tests rewritten to verify FocusScope moniker registration (spatial nav discovers these)