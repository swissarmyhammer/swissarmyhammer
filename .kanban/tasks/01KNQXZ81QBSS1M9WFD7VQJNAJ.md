---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: doing
position_ordinal: '8780'
project: spatial-nav
title: 'Board view: wrap as zone, strip legacy keyboard nav'
---
## What

Wrap the **board view container** in `<FocusZone moniker="ui:board">` and strip every legacy keyboard-nav vestige from `board-view.tsx`. The board zone is the root of the column/card subtree on the kanban view; columns and cards each become zones in their own dedicated cards (column zone, card zone).

### Files to modify

- `kanban-app/ui/src/components/board-view.tsx`

### Zone shape

```
window root layer
  ...other shell zones (navbar, toolbar, perspective)...
  ui:board (FocusZone) ← THIS CARD
    column:{id} (FocusZone) ← separate "column as zone" card
      column header (Leaf)
      task:{id} (FocusZone) ← separate "card as zone" card
        title (Leaf), status (Leaf), pills (Leaves)
```

### Legacy nav to remove from board-view.tsx

Sweep the file for everything that bypasses spatial nav:
- The `cardClaimPredicates` / `nameFieldClaimWhen` memo construction (these were in board-view.tsx for cross-column moniker plumbing)
- The neighbor-moniker arrays (`leftColumnTaskMonikers`, `rightColumnTaskMonikers`, `aboveTaskMonikers`, etc.) — they exist solely to feed claimWhen predicates
- Any `onKeyDown` handlers wired to the outer board div (if any)
- Any `useEffect` listening for `keydown` at the document level scoped to the board view
- `ClaimPredicate` import and threading

What stays: drag-drop handlers (DnD-Kit `useDndMonitor` etc.) — those are unrelated to keyboard nav.

### Subtasks
- [x] Wrap board content in `<FocusZone moniker={Moniker("ui:board")}>` (the moniker is non-entity chrome — stable per window)
- [x] Delete `cardClaimPredicates` / `nameFieldClaimWhen` memos from board-view.tsx
- [x] Delete neighbor-moniker prop plumbing into ColumnView
- [x] Remove `ClaimPredicate` import
- [x] Remove any board-level keyboard listeners that are now redundant

## Acceptance Criteria
- [x] Board view registers exactly one Zone (`ui:board`) at its root; columns appear as children in `parent_zone`
- [x] Zero predicate / neighbor-moniker plumbing remains in board-view.tsx
- [x] No `ClaimPredicate` import in board-view.tsx
- [x] No `onKeyDown` / `keydown` listener in board-view.tsx (other than DnD-Kit's own internals)
- [x] All existing board-view tests pass after changes
- [x] `pnpm vitest run` passes

## Tests
- [x] `board-view.test.tsx` — board container registers as a Zone with `parent_zone = window_root`
- [x] `board-view.test.tsx` — no `claimWhen` / `ClaimPredicate` props accepted
- [x] `board-view.test.tsx` — no neighbor-moniker props (`leftColumnTaskMonikers`, etc.)
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-04-26 10:55)

### Warnings
- [x] `kanban-app/ui/src/components/board-view.tsx` — JSDoc on `BoardView` still claims "Navigation is pull-based: each card and column header FocusScope declares claimWhen predicates". This contradicts the entire purpose of this card — board navigation now flows through the spatial-nav `<FocusZone>` graph, not pull-based claimWhen. Update the doc to describe the actual model (zone-based spatial navigation) so future maintainers do not reintroduce claim machinery believing it is the design.
- [x] `kanban-app/ui/src/components/board-view.tsx` — JSDoc on `useInitialBoardFocus` says "Subsequent focus is driven by pull-based claimWhen predicates — we only need to seed the initial selection." Same staleness as above; the board view no longer has any claimWhen predicates, so this comment misleads readers about how subsequent focus is actually driven (it is the spatial navigator, not pull-based claims). Update to reflect the spatial-nav model.

### Nits
- [x] `kanban-app/ui/src/components/board-view.tsx` — `BoardSpatialZone` uses an inline anonymous prop type (`{ children }: { children: React.ReactNode }`). Every other sub-component in this file (`BoardDragOverlay`, `BoardColumnItem`, `BoardColumnStrip`, `BoardDndWrapper`) defines a named `*Props` interface immediately above. Project JS/TS style guide ("Named prop interfaces ... Even for 2-prop components") and the file's own convention both call for an `interface BoardSpatialZoneProps`. Add the named interface to match.
- [x] `kanban-app/ui/src/components/board-view.tsx` — `useColumnTaskMonikers` builds a full `Map<columnId, taskMonikers[]>` for every column on every board change, but its sole consumer (`useInitialBoardFocus`) only ever reads `monikers[0]` from the first non-empty column. Now that the cross-column neighbor sets are gone, the data structure is over-built. Consider replacing with a hook that resolves only the initial focus moniker (a single string), or keep the bucket map and document that it is intentionally retained for future use. Either way, the current shape is a leftover of the deleted neighbor plumbing.

## Round 2 Implementation Notes (2026-04-26)

All four review findings addressed in `kanban-app/ui/src/components/board-view.tsx`:

- **Warning 1** (`BoardView` JSDoc): Rewrote the docstring to describe the spatial-nav zone model — single `ui:board` zone at the root, columns/cards mount their own zones underneath, direction keys routed by the spatial navigator against the zone tree. Explicitly notes there are no claimWhen predicates and no document-level keydown listeners, and that `useInitialBoardFocus` only seeds the initial selection.
- **Warning 2** (`useInitialBoardFocus` JSDoc): Rewrote to describe the spatial-nav layer's ownership of subsequent focus moves — the hook only fires the initial `setFocus` and stays out of the way. Also documents the `null` case (board has no columns).
- **Nit 1** (`BoardSpatialZone` inline prop type): Extracted to a named `interface BoardSpatialZoneProps` immediately above the component, matching the convention of every other sub-component in the file (`BoardDragOverlayProps`, `BoardColumnItemProps`, `BoardColumnStripProps`, `BoardDndWrapperProps`).
- **Nit 2** (`useColumnTaskMonikers` over-built): Replaced with `useInitialFocusMoniker` that resolves a single `string | null` (the first task moniker, falling back to first column moniker, or null when the board has no columns). Renamed `BoardLayoutResult.columnTaskMonikers` to `initialFocusMoniker` and simplified `useInitialBoardFocus` to take that single value. The full bucket map is gone — it was a leftover of the deleted neighbor plumbing.

Verification: `cd kanban-app/ui && npx vitest run` — 1553 tests pass across 143 files. `npx tsc --noEmit` clean.