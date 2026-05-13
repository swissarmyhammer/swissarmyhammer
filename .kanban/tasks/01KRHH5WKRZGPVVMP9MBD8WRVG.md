---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffe180
title: Virtualize group sections in GroupedBoardView
---
## What

When the user groups by a high-cardinality field (the trigger case is **tags** — but `project`, `assignees`, or any other multi-bucket field hits the same wall), `<GroupedBoardView>` mounts every `<GroupSection>` at once. Each section instantiates a full `<BoardView>` (one virtualizer per column). At ~100+ groups the cost of mounting that many `<BoardView>` trees is the bottleneck, even though the card virtualization landed in 01KREWAXSXWY95SJCZTD03J0AJ correctly windows the cards *inside* each section.

Fix: wrap the outer group list in a second `useVirtualizer` so only viewport-visible group sections mount. Internal card virtualization stays as-is.

### Files to modify

- `kanban-app/ui/src/components/grouped-board-view.tsx` — replace the `groups.map(...)` block with a TanStack `useVirtualizer` over the group buckets. The component's root `<div className="flex flex-col flex-1 min-h-0 overflow-y-auto">` is already the scroll ancestor; keep it, and add the standard virtualizer `transform: translateY` row positioning over an absolutely-positioned total-height container.

- `kanban-app/ui/src/components/group-section.tsx` — hoist the per-section `collapsed` state out of the component. With outer virtualization, unmounting a section as it scrolls out of view drops its `useState`, so collapse state must be lifted to the parent (`<GroupedBoardView>`) and keyed by `bucket.value`. Accept `collapsed` + `onToggleCollapsed` props instead of owning the state.

### Approach

1. **Hoist collapse state**: in `<GroupedBoardView>`, hold a `Set<string>` (or `Record<string, boolean>`) of collapsed group values. Pass `collapsed={collapsedSet.has(bucket.value)}` and `onToggleCollapsed={...}` down to each `<GroupSection>`. Default new keys to expanded.

2. **Outer virtualizer**: add `useVirtualizer({ count: groups.length, getScrollElement: () => scrollRef.current, estimateSize: (i) => collapsedSet.has(groups[i].value) ? COLLAPSED_HEIGHT_PX : EXPANDED_HEIGHT_PX, overscan: 2 })`. Section heights are dynamic in production (expanded sections are `h-[70vh]` per `GroupSection`'s `EXPANDED_BODY_CLASS`; collapsed sections are just a ~36px header), so `estimateSize` returns the correct branch based on collapsed state, and `measureElement` refines it once the section mounts.

3. **Row positioning**: standard TanStack pattern — outer container has `position: relative` + `height: totalSize`; each rendered row has `position: absolute, top: 0, transform: translateY(${start}px)` and a `ref={virtualizer.measureElement} data-index={virtualRow.index}` for dynamic measurement.

4. **`<GroupSection>` API change**: drop the internal `useState`, add `collapsed: boolean` and `onToggleCollapsed: () => void` props. Update the one call site (`<GroupedBoardView>`).

### Constants

```ts
// Collapsed section = section header only (~36px). Header is the only
// rendered element; safe to undershoot — measureElement refines.
const COLLAPSED_HEIGHT_PX = 40;

// Expanded section = h-[70vh] body + header. estimateSize gets close;
// measureElement corrects after mount.
const EXPANDED_HEIGHT_PX = Math.floor(window.innerHeight * 0.7) + 40;
```

### Why this is safe with the existing perf fix

The card virtualization in 01KREWAXSXWY95SJCZTD03J0AJ relies on the `h-[70vh] min-h-0 flex flex-col` body class giving each column's scroll container a bounded ancestor. That contract is preserved here — every mounted section still emits the same DOM shape (`data-testid="group-section-body"` + the column scroll container with `overflow-y-auto`). Outer virtualization only changes *which* sections are mounted, not what they render once mounted.

### Out of scope

- Changing the per-column virtualization (already correct).
- Persisting collapse state across navigation/reload (current behavior is session-local; keep that).
- Animations on group section enter/exit during virtualizer recycling.
- A `<VariableSizeList>` / `react-window` swap — sticking with TanStack `useVirtualizer` since it already powers the column-level virtualization in `<ColumnView>` and the perf test infrastructure is already set up for it.

## Acceptance Criteria

- [x] `<GroupedBoardView>` renders group sections through `useVirtualizer`; only viewport-visible sections (plus overscan) are present in the DOM.
- [x] `<GroupSection>` is a controlled component for collapse state (`collapsed: boolean` + `onToggleCollapsed: () => void` props); it no longer owns `useState`.
- [x] Collapse state persists across scroll — collapsing a group, scrolling it out of view, and scrolling it back in keeps it collapsed.
- [x] On a 2300-task, 200-group fixture (`tag`-like cardinality), the mounted `<GroupSection>` count is bounded (< 20) regardless of total group count.
- [x] Switching the perspective `groupField` to a high-cardinality field on a 2300-task board completes within the 1000ms test-env budget (matches the existing card-virtualization perf test's `REGROUP_BUDGET_MS`).
- [x] No regression in the existing tests `grouped-board-view.perf.test.tsx` and `grouped-board-view.virtualization.test.tsx` — both still pass.
- [x] No visual regression: expanded sections still occupy `h-[70vh]`, collapsed sections still show just the header, drag-and-drop still functions across sections.

## Tests

### New file `kanban-app/ui/src/components/grouped-board-view.group-virtualization.test.tsx`

Mirror the test harness from `grouped-board-view.perf.test.tsx` (same Tauri mocks, same `installViewportGetterOverride`, same provider stack), but:

- [x] Build a fixture with **200 groups × ~10 tasks per group** (`tag`-style cardinality, ~2000 tasks total). Distribute group values as `bucket.value = "tag-${i % 200}"`.
- [x] `mounted_group_section_count_is_bounded_by_viewport` — after initial render, assert `document.querySelectorAll("[data-group-section]").length` is < 20 (overscan + viewport). The `data-group-section` attribute should be added to the `<GroupSection>` root for test selectability.
- [x] `regrouping_high_cardinality_field_completes_under_budget` — same shape as the existing perf test: flip `groupField` from `undefined` to a 200-group field, measure `performance.now()` delta across `rerender + act` flush. Assert `elapsed < REGROUP_BUDGET_MS` (1000ms) AND `mountedSections < 20` AND `mountedCards < TASK_COUNT / 2`.
- [x] `collapse_state_survives_outer_scroll_recycling` — collapse the first group, programmatically scroll the outer container past the recycle window, scroll back, assert the group is still rendered collapsed (header visible, body absent). This is the regression test for the hoisted-state contract.
- [x] `outer_scroll_container_uses_overflow_y_auto` — sanity check that the outer container still carries `overflow-y-auto` so the existing perf test's `installViewportGetterOverride` selector continues to apply.

### Update `kanban-app/ui/src/components/group-section.test.tsx`

- [x] Rewrite the section's existing collapse test to drive collapse via the new `collapsed` + `onToggleCollapsed` props instead of internal state. Add an assertion that the section root carries `data-group-section`.

### Run

- [x] `npx vitest run kanban-app/ui/src/components/grouped-board-view` and `npx vitest run kanban-app/ui/src/components/group-section` — both green.
- [x] `npx vitest run` (full UI suite) — no regressions; 2155 passing.
- [x] `npx tsc --noEmit` (in `kanban-app/ui`) — clean.

## Workflow

- Use `/tdd` — start by writing the new perf test against a 200-group fixture; it will fail with `mountedSections === 200` (current behavior). Then hoist collapse state and add the outer `useVirtualizer`; the test goes green.
- Reference implementation pattern: `<ColumnView>`'s `useVirtualizer` setup already lives in this codebase — read it before writing the outer one. The two virtualizers nest cleanly (outer scrolls vertically across groups; inner scrolls vertically across cards within a group's column).

#perf #frontend

## Review Findings (2026-05-13 16:16)

### Warnings

- [x] `kanban-app/ui/src/components/grouped-board-view.tsx:158-178` — Collapse state bleeds across `groupField` changes. `collapsedSet` lives in `<GroupedBoardBody>`'s `useState`. When `groupField` flips between two values (e.g. `tag` → `project`) the component instance is reused (React reconciles by position/type), so the `Set<string>` carries forward. Buckets in the new group field whose `value` matches a previously-collapsed value (very likely for the `""` ungrouped bucket — both fields produce one — and possible for arbitrary string collisions across dimensions) appear pre-collapsed unexpectedly. The off→on path (`groupField: undefined → "tag"`) correctly resets because `<GroupedBoardBody>` unmounts on the ungrouped path, but field-to-field switches do not. The review prompt for this task explicitly called out "Acceptable behavior is to reset on groupField change" — the implementation does not. Suggested fix: add `key={groupField}` to `<GroupedBoardBody>` at line 136 of `grouped-board-view.tsx` so changing the group field forces a remount and a fresh empty `collapsedSet`. Add a regression test that collapses a bucket under one group field, switches the group field, and asserts no bucket in the new field is pre-collapsed.

  **Resolution (2026-05-13):** Added `key={groupField}` to the `<GroupedBoardBody>` element in `<GroupedBoardView>` so React remounts the inner component (and resets its `useState<Set<string>>`) on every group-field flip. Added regression test `collapse_state_does_not_bleed_across_group_field` in `grouped-board-view.group-virtualization.test.tsx` that collapses a `tag-N` bucket under `groupField="tag"`, switches to `groupField="project"` (where buckets share the same `tag-N` value space on purpose to force a collision), and asserts that every mounted project section renders its body — none inherit the prior collapse state.

- [x] `kanban-app/ui/src/components/grouped-board-view.tsx` (architectural, no specific line) — Drag-and-drop interaction across recycled sections is undefended and untested. The outer virtualizer recycles `<GroupSection>` instances as the user scrolls; if a drag starts inside a section and the user scrolls that section's source/target out of the overscan window, the inner `useDraggable`/`useDroppable` hooks unmount and dnd-kit's per-element registrations go stale. The global `DragSessionProvider` is rooted above this tree so the session itself survives, but the source/target DOM nodes do not. The task acceptance criterion "drag-and-drop still functions across sections" is unverified — no test exercises mid-drag scroll past the recycle window. Either (a) widen `overscan` enough to keep dnd-kit registrations alive for the maximum reasonable scroll distance during a drag, (b) suspend outer virtualization while a drag is active (read `useDragSession()` and short-circuit to a non-virtualized map when `session !== null`), or (c) accept the gap and add a kanban follow-up task. Recommend (b) as the cleanest fix — drag is a transient mode where virtualization correctness matters less than dnd-kit state preservation.

  **Resolution (2026-05-13):** Implemented option (b). `<GroupedBoardBody>` now reads `useDragSession()` and, when `session !== null`, short-circuits to a plain `groups.map(...)` mount path that wraps every `<GroupSection>` in the same `flex flex-col flex-1 min-h-0 overflow-y-auto` outer container (with a `data-drag-bypass="true"` marker for dev tools and future tests). The virtualizer hooks remain called unconditionally; only the render branch changes. Added regression test `drag_suspends_outer_virtualization` in `grouped-board-view.group-virtualization.test.tsx` that verifies the section count rises from the windowed value to `GROUP_COUNT` when a drag session becomes active.

### Nits

- [x] `kanban-app/ui/src/components/grouped-board-view.tsx:183` — `expandedHeightPx` is memoized with an empty deps array, so the estimate captures `window.innerHeight` once at mount and never refreshes. If the user resizes the window the estimate stays stale until each section actually mounts and `measureElement` reports a real height. In practice `measureElement` covers the lag, but the `estimateSize` callback would be more responsive if it either (a) listened to a `resize` event via a small `useState` + `useEffect`, or (b) re-derived on every call (the function is cheap — `Math.floor(window.innerHeight * 0.7) + 40`). Option (b) is the simplest — drop the `useMemo` and call `estimateExpandedHeight()` inline in the `estimateSize` callback.

  **Resolution (2026-05-13):** Implemented option (b). Dropped the `useMemo` and call `estimateExpandedHeight()` inline in the `estimateSize` callback. Updated the docstring on `estimateExpandedHeight` to explain why inline is the right choice (cheap arithmetic; per-index call keeps the estimate responsive to resizes; `measureElement` still refines once sections mount).