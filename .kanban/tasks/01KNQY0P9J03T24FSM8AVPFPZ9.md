---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a680
project: spatial-nav
title: Remove manual claimWhen predicates from inspector and badge-list pills
---
## What

Delete manual predicate construction from the inspector field rows and badge-list pill navigation. These are both linear (1D) layouts that spatial nav handles naturally — fields are stacked vertically, pills are laid out horizontally.

### Files to modify

1. **`kanban-app/ui/src/components/entity-inspector.tsx`**:
   - Delete the `claimPredicates` memo that builds per-field up/down/first/last predicates (~30 lines)
   - Delete `fieldMonikers` memo (only used for predicate neighbor references)
   - Remove `claimWhen` prop from `<FieldRow>` and `<FocusScope>` inside FieldRow
   - Remove `ClaimPredicate` import
   - The `isInspectorField` helper may no longer be needed

2. **`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`**:
   - Delete `pillClaimPredicates` memo that builds per-pill left/right predicates (~30 lines)
   - Delete `pillMonikers` memo if only used for predicates (check — may still be needed for focusMoniker prop)
   - Remove `claimWhen` prop from `<MentionPill>`

3. **`kanban-app/ui/src/components/mention-pill.tsx`**:
   - Remove `claimWhen` prop — no longer needed
   - Remove `ClaimPredicate` import

### Subtasks
- [ ] Delete `claimPredicates` memo from entity-inspector.tsx
- [ ] Remove `claimWhen` prop from FieldRow and its FocusScope
- [ ] Delete `pillClaimPredicates` memo from badge-list-display.tsx
- [ ] Remove `claimWhen` from MentionPill
- [ ] Verify inspector field nav: up/down between fields, first/last to extremes, pill left/right within a field

## Acceptance Criteria
- [ ] Inspector field navigation works via spatial nav (up/down moves between field rows)
- [ ] Pill navigation works via spatial nav (left/right moves between pills within a badge list)
- [ ] `nav.left` from first pill returns focus to parent field (spatial nav: pill is to the right of field label, so left goes to field)
- [ ] `nav.right` from last pill advances to next field or is a no-op (spatial nav resolves this naturally)
- [ ] ~60 lines of predicate code removed
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `kanban-app/ui/src/components/entity-inspector.test.tsx` — field navigation tests pass without predicates
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` — pill navigation tests pass
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx` — existing pill nav tests pass
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.