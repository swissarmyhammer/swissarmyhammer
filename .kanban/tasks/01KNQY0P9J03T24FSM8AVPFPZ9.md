---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: a680
project: spatial-nav
title: 'Inspector and badge-list: wrap field rows as zones, delete claimWhen predicates'
---
## What

Migrate inspector field rows and badge-list pills to the spatial-nav zone model and delete the manual `claimWhen` predicate construction. Field rows become Zones; labels and pills are Leaves. The three-rule beam search then handles both pill-within-field nav (rule 1) and field-to-field nav (rule 2).

### Zone hierarchy inside one panel

```
inspector layer
  panel (Zone)
    field_row_0 (Zone, parent_zone = panel)
      label_0 (Leaf, parent_zone = field_row_0)
      editor_0 OR pill_0a, pill_0b, ... (Leaf each, parent_zone = field_row_0)
    field_row_1 (Zone, parent_zone = panel)
      label_1 (Leaf, parent_zone = field_row_1)
      pill_1a (Leaf)
      pill_1b (Leaf)
    ...
```

### How nav behaves

- Focused on `pill_0a` (inside field_row_0): `nav.right` → `pill_0b` (in-zone beam). `nav.left` → `label_0` (in-zone beam). `nav.down` → no in-zone candidate → rule 2 → nearest leaf below in layer → `label_1` (aligned) or `pill_1a` depending on rect geometry.
- Focused on `label_0`: `nav.down` → rule 1 empty → rule 2 → `label_1`. `nav.right` → `pill_0a`.
- Drill-out onto `field_row_0` (zone level): `nav.down` → `field_row_1` (sibling zone).

### Files to modify

1. **`kanban-app/ui/src/components/entity-inspector.tsx`**
   - Wrap each field row in `<FocusScope kind="zone" moniker={fieldMoniker}>`
   - Delete the `claimPredicates` memo that builds per-field up/down/first/last predicates (around 30 lines)
   - Delete `fieldMonikers` memo (used only for predicate neighbor references)
   - Remove `claimWhen` prop from `<FieldRow>` and any inner `<FocusScope>`
   - Remove `ClaimPredicate` import
   - The `isInspectorField` helper may no longer be needed — remove if unused after this change

2. **`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`**
   - Delete `pillClaimPredicates` memo (around 30 lines)
   - Delete `pillMonikers` memo if only used by predicates (keep if still needed for `focusMoniker` prop elsewhere)
   - Remove `claimWhen` prop from `<MentionPill>`
   - Ensure pills render inside the parent field row's Zone — pills stay as Leaves (default `kind`)

3. **`kanban-app/ui/src/components/mention-pill.tsx`**
   - Remove `claimWhen` prop
   - Remove `ClaimPredicate` import

### Subtasks
- [ ] Wrap each field row in `<FocusScope kind="zone">`
- [ ] Delete `claimPredicates` memo from entity-inspector.tsx
- [ ] Delete `pillClaimPredicates` memo from badge-list-display.tsx
- [ ] Remove `claimWhen` from MentionPill
- [ ] Verify: pill left/right stays in field row; field up/down jumps to next row via rule 2

## Acceptance Criteria
- [ ] Field row is a Zone containing label + pills/editor as Leaves
- [ ] Within-field pill nav works via beam rule 1
- [ ] Across-field nav works via beam rule 2 (cross-zone leaf fallback)
- [ ] Drill-out onto a field row zone + arrow-down goes to next field row (sibling zone)
- [ ] Roughly 60 lines of predicate code removed
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `entity-inspector.test.tsx` — field row wrapper is `kind="zone"`; existing field navigation tests pass without predicates
- [ ] `badge-list-display.test.tsx` — pills render as Leaves inside the parent field row zone; nav tests pass
- [ ] `badge-list-nav.test.tsx` — existing pill nav tests pass
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.