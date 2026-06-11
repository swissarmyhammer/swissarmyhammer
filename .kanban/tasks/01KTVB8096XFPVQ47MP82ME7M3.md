---
assignees:
- claude-code
position_column: todo
position_ordinal: ec80
project: ui-command-cleanup
title: Fix 2 pre-existing failures in grid-view.cursor-ring.test.tsx (cursor ring never renders in browser-mode tests)
---
## What
2 pre-existing test failures in `apps/kanban-app/ui/src/components/grid-view.cursor-ring.test.tsx` have no tracking card. Surfaced (not introduced) during review of Card C (`01KTED6YMERJHTS7QDSTV5MZYG`).

## Exact failing tests
1. `GridView -- cursor-ring suppression outside ui:grid > renders exactly one [data-cell-cursor] when focus is on a grid_cell moniker`
2. `GridView -- click-to-cursor regression (spatial path) > clicking a cell sets entity-focus and lights the cursor ring on that cell`

## Error output (both identical shape)
```
AssertionError: expected +0 to be 1 // Object.is equality
- Expected: 1
+ Received: 0
```
- Test 1 at `src/components/grid-view.cursor-ring.test.tsx:396` — `expect(ringedCells.length).toBe(1)` on `container.querySelectorAll("[data-cell-cursor]")`
- Test 2 at `src/components/grid-view.cursor-ring.test.tsx:516` — same assertion after a click

Zero `[data-cell-cursor]` cells render — the cursor ring never lights.

## Repro
```
cd apps/kanban-app/ui && npx vitest run src/components/grid-view.cursor-ring.test.tsx
```
Result: `Tests 2 failed | 3 passed (5)` in `|browser (chromium)|`.

## Proof pre-existing
Run captured on HEAD `7c5015141764423b008b51d6c9d898d603b32288` BEFORE any Card C review-fix changes (2026-06-11): identical 2-failure set (`expected +0 to be 1` at lines 396 and 516). The Card C review also reproduced the identical failure set at the same HEAD in a clean worktree.

## Likely related
Card `01KTS1C4EX8W6GZYPAYB1T431K` describes the same symptom family (synthetic `focus-changed` emission not reaching the entity-focus store in browser mode) but enumerates only `focus-scope.test.tsx` (9) + `attachment-display.test.tsx` (1). This card covers the orphaned `grid-view.cursor-ring.test.tsx` pair; fix likely shares a root cause — coordinate with that card.