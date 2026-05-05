---
assignees:
- wballard
depends_on:
- 01KQW6EJ2T76X5HH2MRWEGAVQ2
position_column: todo
position_ordinal: d980
project: spatial-nav
title: 'spatial-nav redesign step 10: cutover (1/4) — remove ResizeObserver and useTrackRectOnAncestorScroll'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**. First of four cutover steps.

## Goal

Stop pushing rect updates to the kernel. The snapshot path reads rects fresh at decision time, so continuous rect sync is dead weight.

## What to delete

### `<FocusScope>` rect tracking

In `kanban-app/ui/src/components/focus-scope.tsx`:

- Remove the `ResizeObserver` block at lines 371–387
- Remove the `useTrackRectOnAncestorScroll(ref, fq, updateRect)` call at line 405
- Remove the `updateRect` destructure from `useSpatialFocusActions()` (line 332)

The remaining useEffect (lines 345-403) becomes register-only — it just calls `registerSpatialScope` on mount and `unregisterScope` on unmount. (Those will be deleted in step 11.)

### Hook deletion

Delete `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` entirely. Remove its tests too.

### Rect-validation slimming

`kanban-app/ui/src/lib/rect-validation.ts`: remove the `register_scope` / `register_zone` / `update_rect` op string handling. The validator will only be called from snapshot building going forward (per-snapshot validation, no IPC association).

## What MUST keep working

- Nav: arrow keys still focus the right targets — the snapshot path provides fresh rects at nav time
- Click focus: still works — snapshot built at click time
- Focus-lost fallback: still works — snapshot built at delete time

## Tests

- All snapshot-path nav and focus tests still pass (no rect sync needed since snapshots are fresh)
- Drag-drop scenario: the original "two entries share (x, y)" overlap warning that motivated this redesign no longer fires (it's emitted from the rect-update hot path which is now gone)
- Sibling-reflow: card moves because a sibling reordered, then user navigates from the moved card — lands correctly because snapshot reads fresh rect
- Removed file's tests are removed cleanly with no broken imports elsewhere

## Out of scope

- Removing register/unregister IPCs (step 11)
- Removing `SpatialRegistry::scopes` (step 12)

## Acceptance criteria

- ResizeObserver and useTrackRectOnAncestorScroll are gone
- Drag-drop overlap warning eliminated
- All e2e nav and focus tests green
- `pnpm -C kanban-app/ui test` and `cargo test` green

## Files

- `kanban-app/ui/src/components/focus-scope.tsx` — strip rect-tracking
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — DELETE
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.test.ts` (if exists) — DELETE
- `kanban-app/ui/src/lib/rect-validation.ts` — slim down #01KQTC1VNQM9KC90S65P7QX9N1