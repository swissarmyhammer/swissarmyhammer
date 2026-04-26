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
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that **fields in cards (title, status, assignee pills, tag pills) cannot be focused or selected**. Registration plumbing is in place (verified by tests) and clicks fire `spatial_focus`, but no visible focus indicator appears on the leaf the user clicks. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

This card was moved back to `doing` because the previous "done" criterion ("registration test passes") was the wrong bar.

## Remaining work

1. **Verify the click → indicator-rendered chain** for each leaf inside the card zone:
   - card title leaf
   - card status leaf
   - assignee pill leaves
   - tag pill leaves
2. Audit each leaf's `<Focusable>` / `<FocusScope>` for `showFocusBar` value. If it's the implicit default, confirm the default is what we want (`true`). If suppressed, decide deliberately and document why.
3. Walk the focus-changed event path with the dev console open: click a card title, watch for the Tauri event, watch for the React claim callback, watch for the indicator render.
4. Add an integration test per leaf that asserts visible focus indicator after click (not just `data-focused` attribute, but the `<FocusIndicator>` element actually mounted).

## Files involved

- `kanban-app/ui/src/components/entity-card.tsx`
- `kanban-app/ui/src/components/sortable-task-card.tsx`
- `kanban-app/ui/src/components/focusable.tsx` and `focus-zone.tsx` (audit `showFocusBar` default + indicator render path)
- `kanban-app/ui/src/components/focus-indicator.tsx` (visual rendering correctness)
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` (claim registry + Tauri event subscription)

## Acceptance Criteria

- [ ] Manual smoke: clicking a card title produces a visible indicator on the title
- [ ] Manual smoke: clicking a status pill produces a visible indicator on the pill
- [ ] Manual smoke: clicking an assignee pill produces a visible indicator on the pill
- [ ] Integration test: each leaf, when its `SpatialKey` becomes the focused key for the window, renders a visible `<FocusIndicator>`
- [ ] Each leaf with `showFocusBar={false}` has an inline comment explaining why
- [ ] Existing card tests stay green

## Tests

- [ ] `entity-card.spatial-nav.test.tsx` (or extension of existing) — click title → assert visible indicator
- [ ] Same for status pill, assignee pill, tag pill
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first (click leaf → indicator visible), watch it fail, then identify and fix the breakage in whichever layer is failing (showFocusBar / indicator CSS / claim registry / Tauri event).

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) What

Wrap each task card in `<FocusZone moniker="task:{id}">` and strip every legacy keyboard-nav vestige from `entity-card.tsx` and `sortable-task-card.tsx`. The card zone sits inside its column zone (parent_zone = `column:{id}`) and contains title, status, assignee pills as leaves.

## (Prior) Implementation Notes (2026-04-26)

- The `kind="zone"` upgrade for `entity-card.tsx` was already in flight from prior work; this card finalised the prop-removal half.
- Removed `claimWhen` prop and `ClaimPredicate` import from both `entity-card.tsx` and `sortable-task-card.tsx`. Replaced inline doc to explain the new contract: descendants of the card's zone scope register with the card's spatial key as their `parent_zone` automatically — no per-card predicate construction needed.
- Removed the now-dead `cardClaimPredicates` plumbing from `column-view.tsx`: deleted the `useCardClaimPredicates` hook, supporting predicate functions, the `CardClaimParams` interface, and the prop threading through `ColumnLayout` / `VirtualizedCardListProps` / `VirtualColumnProps` / `VirtualRowProps`.
- Added a new `describe("spatial registration as a FocusZone")` block in `entity-card.test.tsx` that mounts the card inside `SpatialFocusProvider` + `FocusLayer` so the underlying `<FocusZone>` primitive registers with the mocked Tauri invoke. Verified zone registration, leaf-registration absence, click-to-spatial-focus, and parent_zone shape.
- All 1515 tests pass; `npx tsc --noEmit` is clean.