---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: ff9480
project: spatial-nav
title: 'Card: wrap as zone, strip legacy keyboard nav from entity-card'
---
## What

Wrap each task card in `<FocusZone moniker="task:{id}">` and strip every legacy keyboard-nav vestige from `entity-card.tsx` and `sortable-task-card.tsx`. The card zone sits inside its column zone (parent_zone = `column:{id}`) and contains title, status, assignee pills as leaves.

### Files to modify

- `kanban-app/ui/src/components/entity-card.tsx`
- `kanban-app/ui/src/components/sortable-task-card.tsx`

### Zone shape (parent context)

```
column:{id} (FocusZone)
  task:{id} (FocusZone) ← THIS CARD
    title (Leaf)
    status (Leaf)
    assignee_pill (Leaf, one per assignee)
    tag_pill (Leaf, one per tag)
    ...other displayed fields
```

### Legacy nav to remove

- `claimWhen` prop and any predicate construction in entity-card.tsx
- `claimWhen` prop on sortable-task-card.tsx and its passthrough
- `ClaimPredicate` import in both files
- Any `onKeyDown` handler on the card root (e.g., Enter-to-inspect — that becomes the `nav.drillIn` / `ui.inspect` keybinding handled at app level per card `01KPZS4RG0...`)
- Any imperative focus stealing (e.g., `ref.current?.focus()`) wired to keyboard events

What stays: drag handle bindings (DnD-Kit `useDraggable`), click-to-inspect handlers (mouse), status-toggle click handlers.

### Subtasks
- [ ] Wrap card body in `<FocusZone moniker={Moniker(`task:${id}`)}>` (or whatever the entity-type-prefixed moniker is)
- [ ] Inner title / status / pill components stay as default `Focusable` (leaves)
- [ ] Remove `claimWhen` prop from entity-card.tsx
- [ ] Remove `claimWhen` prop from sortable-task-card.tsx
- [ ] Remove `ClaimPredicate` imports
- [ ] Remove card-level `onKeyDown` handlers — Enter/Space behavior moves to global drill-in / inspect commands

## Acceptance Criteria
- [ ] Each card registers as a `FocusZone` with `parent_zone = column:{id}`
- [ ] Title / status / pills register as leaves with `parent_zone = task:{id}`
- [ ] No `claimWhen` prop on entity-card.tsx or sortable-task-card.tsx
- [ ] No `ClaimPredicate` import in either file
- [ ] No `onKeyDown` on the card root (other than any drag-handle accessibility from DnD-Kit, which is unrelated)
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `entity-card.test.tsx` — card wrapper is `kind="zone"`, no `claimWhen` prop accepted
- [ ] `entity-card.test.tsx` — title and status register as leaves with `parent_zone = card_zone_key`
- [ ] `entity-card.test.tsx` — clicking the card invokes `spatial_focus` (via the primitive); does NOT open the inspector directly (inspect now Space-bound at app level)
- [ ] `sortable-task-card.test.tsx` — same shape; drag handle still works
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.