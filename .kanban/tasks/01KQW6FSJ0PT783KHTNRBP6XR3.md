---
assignees:
- wballard
depends_on:
- 01KQW6EJ2T76X5HH2MRWEGAVQ2
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffad80
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
- `kanban-app/ui/src/lib/rect-validation.ts` — slim down #stateless-nav

## Review Findings (2026-05-07 10:59)

### Nits
- [x] `kanban-app/ui/src/lib/spatial-focus-context.tsx:478` — Stale doc comment. The block comment in the `onDeleted` listener says "The cached rect is refreshed on every register / ResizeObserver / ancestor-scroll fire, so it always reflects the most recent live geometry." After this step, the only writers to `lastKnownRect` are (1) the initial mount-time `updateRect` in the registration `useEffect` and (2) the new `useLayoutEffect` cleanup just before unmount. ResizeObserver and ancestor-scroll triggers no longer exist. Update the comment to reflect the two surviving writers (mount-time seed + pre-unmount layoutEffect cleanup), or just say "refreshed at mount and immediately before unmount" — drop the obsolete mechanism names.
- [x] `kanban-app/ui/src/lib/layer-scope-registry-context.tsx:84-87` and `:207-214` — Stale doc comments on `ScopeEntry.lastKnownRect` and `LayerScopeRegistry.updateRect`. Both still claim `updateRect` is called from "every `ResizeObserver` fire" and "every ancestor-scroll-driven resample." Same fix as above — those code paths are gone in this step. The comments should describe the two remaining call sites: initial mount-time seed and the `useLayoutEffect` cleanup that captures fresh geometry just before the bound ref is nullified.
- [x] `kanban-app/ui/src/components/focus-scope.tsx:387-406` — The `useLayoutEffect`-cleanup approach correctly reads `ref.current` while it's still attached, but the doc comment only explains the ref-nullification half of the invariant. The other half — that `useLayoutEffect` cleanups run BEFORE the `useEffect` cleanup that calls `layerRegistry.delete(fq)` (which fires the deletion listener that reads `lastKnownRect`) — is implicit. Add one sentence: "Because layout-effect cleanups run before useEffect cleanups, this updateRect writes the fresh rect before `delete(fq)` fires the deletion listener that consumes it." This makes the load-bearing ordering explicit so a future refactor (say, swapping the two effects' ordering, or merging them) is less likely to silently regress the focus-lost IPC.