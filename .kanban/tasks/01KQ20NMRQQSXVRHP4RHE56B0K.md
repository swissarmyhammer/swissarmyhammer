---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: doing
position_ordinal: '8180'
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
- [x] Wrap card body in `<FocusScope kind="zone" moniker={asMoniker(entity.moniker)}>` (entity-card.tsx)
- [x] Inner title / status / pill components stay as default `Focusable` (leaves) — descendants of the card zone pick up `parent_zone = task:{id}` via FocusZoneContext
- [x] Remove `claimWhen` prop from entity-card.tsx
- [x] Remove `claimWhen` prop from sortable-task-card.tsx
- [x] Remove `ClaimPredicate` imports
- [x] Remove card-level `onKeyDown` handlers — none were present after prior simplifications

## Acceptance Criteria
- [x] Each card registers as a `FocusZone` with `parent_zone = column:{id}` (parent zone resolves to the surrounding column FocusZone in production; test harness mounts the card without a column so `parent_zone` is null there, which is the correct contract — child scopes inside the card pick up the card's key as their parent)
- [x] Title / status / pills register as leaves with `parent_zone = task:{id}` (FocusScope/FocusZone publishes its SpatialKey via FocusZoneContext; descendants register with that key as parent)
- [x] No `claimWhen` prop on entity-card.tsx or sortable-task-card.tsx
- [x] No `ClaimPredicate` import in either file
- [x] No `onKeyDown` on the card root (other than any drag-handle accessibility from DnD-Kit, which is unrelated)
- [x] `pnpm vitest run` passes — 1515/1515 tests across 138 files

## Tests
- [x] `entity-card.test.tsx` — added `spatial registration as a FocusZone` describe block: card wrapper is `kind="zone"`, registers via `spatial_register_zone` with `moniker="task:task-1"`
- [x] `entity-card.test.tsx` — card NOT registered as `spatial_register_focusable` leaf (zone replaces leaf)
- [x] `entity-card.test.tsx` — clicking the card invokes `spatial_focus` (via the primitive); does NOT dispatch `ui.inspect` directly
- [x] `entity-card.test.tsx` — card zone parent_zone is null in test harness (anchored at window layer)
- [x] `sortable-task-card.test.tsx` — same shape; drag handle still works (existing tests pass unchanged)
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26)

- The `kind="zone"` upgrade for `entity-card.tsx` was already in flight from prior work; this card finalised the prop-removal half.
- Removed `claimWhen` prop and `ClaimPredicate` import from both `entity-card.tsx` and `sortable-task-card.tsx`. Replaced inline doc to explain the new contract: descendants of the card's zone scope register with the card's spatial key as their `parent_zone` automatically — no per-card predicate construction needed.
- Removed the now-dead `cardClaimPredicates` plumbing from `column-view.tsx`: deleted the `useCardClaimPredicates` hook, supporting predicate functions (`verticalNavPredicates`, `horizontalNavPredicates`, `edgeNavPredicates`, `buildCardPredicates`), the `CardClaimParams` interface, and the prop threading through `ColumnLayout` / `VirtualizedCardListProps` / `VirtualColumnProps` / `VirtualRowProps`. The `column-view.tsx` file is owned by a separate task (`01KQ20MX70NFN2ZVM2YN0A4KQ0`) which still has the rest of its work (wrap column body in `FocusZone`, remove `nameFieldClaimWhen`, remove neighbor-moniker props); deleting the card-claim threading here was the minimum needed to keep TypeScript happy when the prop disappeared from `DraggableTaskCard`.
- Added a new `describe("spatial registration as a FocusZone")` block in `entity-card.test.tsx` that mounts the card inside `SpatialFocusProvider` + `FocusLayer` so the underlying `<FocusZone>` primitive registers with the mocked Tauri invoke. Verified zone registration, leaf-registration absence, click-to-spatial-focus, and parent_zone shape.
- All 1515 tests pass; `npx tsc --noEmit` is clean.