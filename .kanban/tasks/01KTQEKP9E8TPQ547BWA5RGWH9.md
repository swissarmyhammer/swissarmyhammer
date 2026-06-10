---
assignees:
- claude-code
position_column: todo
position_ordinal: e080
title: 'Pre-existing spatial vitest breakage beyond card 01KTQ8KRJYX1DPHN76TZ654ZX2: 12 more failing files (49 tests) + focus-layer.test.tsx import failure'
---
## What

While reviewing 01KTQCHWP5T4GS8SPGYVXD2CT9 (layer-op FIFO fix), a full `npx vitest run spatial` in `apps/kanban-app/ui` showed 50 failing tests across 14 files plus 2 import-failed suites. Card `01KTQ8KRJYX1DPHN76TZ654ZX2` covers only TWO of these (perspective-tab-bar.enter-rename test #3 and the `spatial-focus-context.test.tsx` import failure). Everything below is NOT covered by any card.

**Proven pre-existing**: the identical failure set (same files, same per-file counts) reproduces with `spatial-focus-context.tsx` reverted to HEAD (baseline comparison runs 2026-06-09, review of 01KTQCHWP5T4GS8SPGYVXD2CT9). The FIFO fix neither causes nor masks any of them.

## Uncovered failures (file — failing test count)

Import failure (same `SERIALIZE_TO_IPC_FN` mock gap as the covered spatial-focus-context.test.tsx — the static `@tauri-apps/api/window` import needs `@tauri-apps/api/core` mocked with that export; mirror `spatial-focus-context.responders.test.tsx`):
- [ ] `src/components/focus-layer.test.tsx` — fails at import

Assertion failures (likely stale harness expectations from the host-driven nav/drill rework and window-unique root FQs — e.g. `expect(spatialDrillInCalls()).toHaveLength(1)` getting 0, and `spatial_focus.fq must end with filter_editor:p1 (got )`):
- [ ] `src/spatial-nav-end-to-end.spatial.test.tsx` — 5
- [ ] `src/spatial-nav-soak.spatial.test.tsx` — 6
- [ ] `src/components/ai-panel-elicitation.spatial.test.tsx` — 10
- [ ] `src/components/ai-panel.spatial.test.tsx` — 5
- [ ] `src/components/board-view.cross-column-nav.spatial.test.tsx` — 5
- [ ] `src/components/board-view.spatial-nav.test.tsx` — 1
- [ ] `src/components/board-view.spatial.test.tsx` — 3
- [ ] `src/components/column-view.add-task-enter.spatial.test.tsx` — 2
- [ ] `src/components/column-view.spatial.test.tsx` — 5
- [ ] `src/components/entity-card.in-zone-nav.spatial.test.tsx` — 1
- [ ] `src/components/grid-view.keyboard-nav.spatial.test.tsx` — 5
- [ ] `src/components/perspective-tab-bar.filter-enter.spatial.test.tsx` — 1

## Acceptance Criteria
- [ ] Each file either updated to the current production contract (host-driven drill via `dispatch_command nav.drillIn`, window-unique root FQs) or its stale harness helper fixed once and reused — NOT 12 copy-paste fixes if the root cause is shared (probable: a shared `spatialDrillInCalls()`-style helper and a shared mock-setup gap)
- [ ] `npx vitest run spatial` green in `apps/kanban-app/ui` (excluding files owned by 01KTQ8KRJYX1DPHN76TZ654ZX2 if still open)

## Constraints
- Scoped vitest only; no whole-workspace builds.
- Diagnose the shared root cause FIRST (the two sampled failure shapes suggest one or two causes, not twelve).