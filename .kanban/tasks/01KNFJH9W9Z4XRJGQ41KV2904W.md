---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffc380
title: Extend DropZoneDescriptor with optional group context
---
## What

Add an optional `groupValue` field to `DropZoneDescriptor` and update `computeDropZones` to accept and propagate it. This is the data plumbing that enables group-aware drops — no UI changes yet.

### Files to modify

1. **`kanban-app/ui/src/lib/drop-zones.ts`**:
   - Add `groupValue?: string` to `DropZoneDescriptor` interface
   - Add optional `groupValue?: string` parameter to `computeDropZones(taskIds, columnId, groupValue?)`
   - When `groupValue` is provided, every generated descriptor carries it
   - When `groupValue` is undefined (ungrouped board), descriptors have no `groupValue` — backward compatible

2. **`kanban-app/ui/src/lib/drop-zones.test.ts`** (or create if it doesn't exist):
   - Test that `computeDropZones` without groupValue produces descriptors with no groupValue
   - Test that `computeDropZones` with groupValue propagates it to all descriptors

### Design decision

`groupValue` is a single string, not an array. For multi-value fields like tags, each group section represents ONE tag value. When a card is dropped into the \"bug\" group, the drop handler will add \"bug\" to the task's tags. The `groupValue` carries which specific value this drop zone's group section represents.

The drop zone itself doesn't know the field name — it only carries the value. The field name comes from the perspective's `groupField` and is resolved at drop-handling time in BoardView.

## Acceptance Criteria

- [ ] `DropZoneDescriptor` has optional `groupValue?: string`
- [ ] `computeDropZones(taskIds, columnId)` still works without groupValue (backward compat)
- [ ] `computeDropZones(taskIds, columnId, \"bug\")` propagates \"bug\" to all descriptors
- [ ] Existing tests still pass
- [ ] No UI changes — this is pure data plumbing

## Tests

- [ ] Test: `computeDropZones` without groupValue — descriptors have no groupValue property
- [ ] Test: `computeDropZones` with groupValue — all descriptors carry it
- [ ] Test: empty column with groupValue — single descriptor carries groupValue
- [ ] Existing drop-zone tests pass unchanged
- [ ] `npm test -- drop-zones` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.">
</invoke>