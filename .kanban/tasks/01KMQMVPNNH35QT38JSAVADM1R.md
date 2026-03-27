---
assignees:
- claude-code
depends_on:
- 01KMQMV1NK8EA6AC7NRQC15T7V
position_column: done
position_ordinal: ffffffffffffee80
title: Pill navigation via claimWhen
---
## What

BadgeListDisplay computes pill monikers and passes `claimWhen` predicates to each MentionPill's FocusScope. No pill-specific commands — just `nav.left`/`nav.right` with predicates that check parent field or sibling pill focus.

### How it works

For a tags field with pills `[tag:a, tag:b, tag:c]` and field moniker `task:id.tags`:

**First pill (tag:a)**:
- `nav.right` claims when `task:id.tags` is focused (enter from field)
- `nav.left` claims when `tag:b` is focused (receive from right sibling)

**Middle pill (tag:b)**:
- `nav.right` claims when `tag:a` is focused
- `nav.left` claims when `tag:c` is focused

**Last pill (tag:c)**:
- `nav.right` claims when `tag:b` is focused
- (no nav.left claim — clamp at end)

### Files to modify

- **`kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`** — compute pill monikers (already knows entity type + entity IDs), build `claimWhen` arrays, pass to MentionPill
- **`kanban-app/ui/src/components/mention-pill.tsx`** — accept and forward `claimWhen` to its FocusScope
- **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`** — remove `inspector.pillLeft`/`inspector.pillRight` commands (replaced by global `nav.left`/`nav.right` + claimWhen)

### Field moniker availability

BadgeListDisplay needs to know its parent field moniker (e.g. `task:id.tags`) so the first pill can claim on `nav.right` when the field is focused. This comes from the FieldRow's FocusScope moniker. Pass it as a prop through Field, or derive from `entity.entity_type + entity.id + field.name` using `fieldMoniker()`.

## Acceptance Criteria

- [ ] `l`/ArrowRight on a focused tags field → first pill gets `data-focused`
- [ ] `l`/ArrowRight on a focused pill → next pill gets `data-focused`
- [ ] `h`/ArrowLeft on a focused pill → previous pill gets `data-focused`
- [ ] Clamps at first/last pill (no wrap)
- [ ] `j`/`k` from a focused pill moves to adjacent field (nav.up/nav.down still works — pill is inside field scope chain)
- [ ] Works for tags, assignees, depends_on — any badge-list field

## Tests

- [ ] `badge-list-display.test.tsx` — render with two pills, broadcast nav.right from field moniker, first pill claims focus
- [ ] `badge-list-display.test.tsx` — broadcast nav.right from first pill, second pill claims focus
- [ ] `badge-list-display.test.tsx` — broadcast nav.right from last pill, focus unchanged (clamp)
- [ ] `badge-list-display.test.tsx` — broadcast nav.left from first pill, focus unchanged (clamp)
- [ ] `pnpm vitest run` passes"