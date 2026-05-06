---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff980
project: spatial-nav
title: Filter formula bar lacks a FocusScope — not navigable in spatial graph
---
## What

The filter formula bar in the perspective tab bar (the always-visible CM6 filter expression editor on the right side) is not registered as a leaf in the spatial-nav graph, so beam-search arrow-key navigation skips it entirely. The user cannot reach the filter editor from the perspective tabs (or any other peer leaf) via `nav.left` / `nav.right`.

### Current state

`kanban-app/ui/src/components/perspective-tab-bar.tsx`:

- The bar is wrapped in `<PerspectiveBarSpatialZone>` → `<FocusZone moniker={asSegment("ui:perspective-bar")}>` (perspective-tab-bar.tsx:300, 307).
- Each perspective tab is wrapped in `<PerspectiveTabFocusable>` → `<FocusScope moniker={asSegment(`perspective_tab:${id}`)}>` (perspective-tab-bar.tsx:444, 457).
- `<FilterFormulaBar>` (perspective-tab-bar.tsx:822) is rendered as a sibling of the tabs inside the same `PerspectiveBarSpatialZone` — but it has **no** `<FocusScope>` wrapper. Its outer `<div data-testid="filter-formula-bar">` is a plain DOM node with `onClick` that calls `editorRef.current?.focus()`. Without a `<FocusScope>`, no `spatial_register_scope` IPC fires for this region, so the kernel's beam-search has nothing to land on.

### Fix shape

Mirror the `PerspectiveTabFocusable` pattern. Add a small `FilterFormulaBarFocusable` wrapper in `kanban-app/ui/src/components/perspective-tab-bar.tsx` that:

1. Reads `useOptionalEnclosingLayerFq()` and `useOptionalSpatialFocusActions()` (same conditional guard the tab focusable uses).
2. When the spatial-nav stack is mounted, wraps its children in `<FocusScope moniker={asSegment(`filter_editor:${perspectiveId}`)}>`. The per-perspective segment matches the existing `key={activePerspective.id}` remount pattern on `<FilterFormulaBar>` (perspective-tab-bar.tsx:269) so the kernel sees a distinct leaf per perspective rather than a shared one whose identity flips on perspective change.
3. When no spatial stack is mounted, render `children` unwrapped (matches the tab focusable's narrow-test fallback).

Apply the wrapper around `<FilterFormulaBar>` at perspective-tab-bar.tsx:267–274, parallel to how `<ScopedPerspectiveTab>` wraps its inner `<PerspectiveTab>` in `<PerspectiveTabFocusable>`.

### Out of scope (call out, do not solve)

- Keyboard handling **inside** the CM6 editor (ArrowLeft / ArrowRight moving the cursor vs. firing `nav.left` / `nav.right`) is its own concern. This task only adds the leaf so the editor becomes a navigable target. If a follow-up task is needed for in-editor arrow-key handoff to spatial nav, file it separately.
- The existing `onClick={() => editorRef.current?.focus()}` on the bar's outer div should keep working because `<FocusScope>`'s click handler skips clicks that land on `INPUT`/`TEXTAREA`/`[contenteditable]` (focus-scope.tsx:412–415). Verify this still routes the click into CM6 and does not steal caret placement.

### Files to modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — add `FilterFormulaBarFocusable` (mirror of `PerspectiveTabFocusable`, perspective-tab-bar.tsx:444), wrap the `<FilterFormulaBar>` instance at line ~268.
- `kanban-app/ui/src/components/perspective-bar.spatial.test.tsx` — add tests below.
- (Likely) `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts` — add a guard line asserting the source contains the new `<FocusScope moniker={asSegment(`filter_editor:${...}`)}` pattern. Mirror the existing tab-pattern guard at line 61.

## Acceptance Criteria

- [x] Mounting the perspective tab bar inside the spatial provider stack produces exactly one additional `spatial_register_scope` call whose `segment` is `filter_editor:{activePerspectiveId}`, with `parentZone` equal to the `ui:perspective-bar` zone's FQM and the same `layerFq` as the tabs.
- [x] The filter-formula-bar DOM node carries `data-segment="filter_editor:{activePerspectiveId}"` and a composed `data-moniker` attribute.
- [x] Driving a `focus-changed` event whose `next_fq` matches the filter editor leaf's FQM flips `data-focused="true"` on that node and renders the `<FocusIndicator>` inside it.
- [x] No regression: each `perspective_tab:{id}` leaf still registers, and clicking a tab still dispatches exactly one `spatial_focus` for that tab's FQM (perspective-bar.spatial.test.tsx:344 stays green).
- [x] Switching active perspective unregisters the previous `filter_editor:{prevId}` leaf and registers a new `filter_editor:{nextId}` leaf (the existing `key={activePerspective.id}` remount drives this — assert via `spatial_unregister_scope` / `spatial_register_scope` IPC counts).

## Tests

- [x] In `kanban-app/ui/src/components/perspective-bar.spatial.test.tsx`, add `it("registers a filter_editor:{activePerspectiveId} scope as a peer of the perspective tabs", …)` that mounts the bar with two perspectives, finds the active one's id, asserts the register-scope args contain `segment === \`filter_editor:${activeId}\`` exactly once, and asserts its `parentZone` equals the bar zone's FQM and its `layerFq` equals the bar zone's `layerFq`.
- [x] Add `it("driving focus-changed to the filter_editor leaf flips data-focused on the formula bar", …)` that, after registration, calls `fireFocusChanged({ next_fq: filterLeaf.fq })`, then asserts `[data-segment="filter_editor:..."][data-focused]` is present and `[data-testid="focus-indicator"]` lives inside it.
- [x] Add `it("switching perspectives unregisters the previous filter_editor leaf and registers the next", …)` that switches the active perspective via `setActivePerspectiveId` (or the equivalent test helper), then asserts an `spatial_unregister_scope` for `filter_editor:{prevId}` and a fresh `spatial_register_scope` for `filter_editor:{nextId}`.
- [x] In `kanban-app/ui/src/components/perspective-spatial-nav.guards.node.test.ts`, add a guard `it("wraps the filter formula bar in FocusScope with moniker filter_editor:${activePerspectiveId}", …)` modeled on the existing tab guard at line 61, regex-matching the source for the new wrapper.
- [x] Run `pnpm -C kanban-app/ui test perspective-bar.spatial perspective-spatial-nav.guards` and confirm all new tests pass and none of the existing tests in those files regress.

## Workflow

- Use `/tdd` — write the three new spatial tests + the source guard first (RED), then add the `FilterFormulaBarFocusable` wrapper in `perspective-tab-bar.tsx` and apply it around the `<FilterFormulaBar>` instance (GREEN). Re-run the full perspective-bar suite to catch regressions in the existing tab-leaf tests.