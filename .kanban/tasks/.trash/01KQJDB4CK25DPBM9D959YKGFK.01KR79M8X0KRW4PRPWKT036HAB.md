---
assignees:
- wballard
position_column: todo
position_ordinal: ab80
title: Keyboard navigation skips some perspective tabs (investigate + fix)
---
## What

Pressing Left/Right (or whatever the perspective-bar nav binding is) does **not** reliably visit every perspective tab — only some tabs are reachable. Reproducible by the user. Cause is not yet identified; this task is investigation-driven and fixes whichever cause the investigation confirms.

### Initial hypothesis ruled out

The user's first hunch was non-unique monikers. Research disproves this:

`kanban-app/ui/src/components/perspective-tab-bar.tsx:421-438` registers each tab as:

```tsx
<FocusScope moniker={asSegment(`perspective_tab:${id}`)}>
  {children}
</FocusScope>
```

where `id` is `perspective.id` (line 394), and `PerspectiveDef.id` (`perspective-context.tsx:34-42`) is a backend-issued ULID — unique by construction. Composed FQMs (`<layer>/.../ui:perspective-bar/perspective_tab:<ULID>`) cannot collide.

We still want a regression test pinning uniqueness so this never silently regresses.

### Candidate causes to investigate (in priority order)

- [ ] **C1. Off-screen tabs / horizontal scroll.** Tab bar is `overflow-x-auto max-w-[60%]` (`perspective-tab-bar.tsx:223`). Tabs scrolled out of the visible window may have `getBoundingClientRect()` outside the layer bounds, so the kernel's beam-search filters them out. Read `swissarmyhammer-focus/src/nav.rs` (or wherever directional nav lives) for how off-rect siblings are treated. **Test**: render N (e.g. 12) tabs forcing horizontal overflow, drive Right repeatedly, assert all N are visited (auto-scroll if needed).
- [ ] **C2. Asymmetric tab rects (active vs inactive).** Active tab renders `<FilterFocusButton>` and `<GroupPopoverButton>` (`perspective-tab-bar.tsx:547-558`); inactive tabs don't. Active tab is materially wider. Beam-search "nearest center along axis" can be biased by asymmetric widths, particularly if it scores by overlap fraction. **Test**: render tabs with varying widths (mock the active-vs-inactive shape on multiple tabs), drive Left/Right, assert sequential visit.
- [ ] **C3. Conditional registration gate.** Lines 422-424 short-circuit registration when `layerKey` or `actions` is null:

  ```tsx
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) return <>{children}</>;
  ```

  If for any tab on initial render the layer ancestor isn't yet available (timing/race during mount or after re-parenting), that tab silently bypasses focus registration entirely. **Test**: rapidly mount/unmount the perspective bar (e.g. layer change) and assert all tabs register.
- [ ] **C4. Stale rect snapshots.** `focus-scope.tsx:319-321` notes that `navOverride` is snapshotted only at registration time; verify whether `rect` is also snapshotted-only or live. If rects are snapshotted on first registration and tabs reflow (e.g. when the active tab grows extras on click), older rects may misrepresent layout. **Test**: switch active tab, then drive Right from a previously-active tab and assert the visited target matches the new layout.
- [ ] **C5. Sanity check the moniker hypothesis.** Add an assertion that all `spatial_register_scope` calls within one render of the perspective bar produce distinct FQMs across tabs. Pins the kept-unique invariant.

The implementer should:

1. Reproduce the bug in a focused test before changing anything (instrument or pause and capture: list of registered FQMs + their rects + which Right→Right→… sequence visits them).
2. Identify which of C1–C5 (or some other cause uncovered during investigation) is the actual culprit.
3. Fix only that root cause. Do not patch all five; do not add per-tab special cases.
4. Add the regression tests for both the confirmed cause AND the moniker-uniqueness invariant.

### Files to investigate / likely modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — tab registration, rendering, scroll container.
- `kanban-app/ui/src/components/focus-scope.tsx:178-181, 319-369` — FQM composition + how rect/navOverride get into the kernel.
- `swissarmyhammer-focus/src/registry.rs:130-146` and `swissarmyhammer-focus/src/nav.rs` (or directional-nav module) — to confirm what beam-search does with rects outside layer bounds.
- `kanban-app/ui/src/components/perspective-tab-bar.spatial-nav.test.tsx` — existing spatial-nav test file; natural home for new regression tests.

### Non-goals

- Do **not** change the moniker scheme (already correct).
- Do **not** rework beam-search semantics globally — fix the one slip that lets a tab be unreachable.
- Do **not** add a "auto-scroll into view before navigating" hack at the perspective-bar level if the kernel itself ought to handle it. Push the fix to the right layer.

## Acceptance Criteria

- [ ] Pressing the nav direction repeatedly from the leftmost tab visits **every** perspective tab in sequence and stops at the rightmost (or wraps, matching existing convention) — no tabs skipped.
- [ ] This holds when the perspective bar overflows horizontally (more tabs than fit in `max-w-[60%]`).
- [ ] This holds regardless of which tab is currently active (active tab has wider rect due to extra child buttons).
- [ ] All `spatial_register_scope` FQMs emitted by the perspective bar in one render are distinct (regression test pins moniker uniqueness).
- [ ] Root cause is documented in the PR/commit message (one of C1–C5 or whatever is found) — no scattershot fixes.

## Tests

- [ ] **TDD: write the failing test first** in `kanban-app/ui/src/components/perspective-tab-bar.spatial-nav.test.tsx`. Render ≥6 tabs (enough to force horizontal overflow given `max-w-[60%]`), drive Right N-1 times from tab 0, and assert focus visits each tab in order. Run, confirm it fails (reproduces bug), then implement.
- [ ] Add a regression test: render a list of perspectives with varied names, capture all `spatial_register_scope` FQMs from one render, assert they are pairwise distinct (pins C5 invariant).
- [ ] If C2 is the culprit: add a test where one tab is "active" (renders extra children, has wider rect) and Right/Left still visits it correctly.
- [ ] If C3 is the culprit: add a test that layer-ancestor availability is robust to mount/re-parent races.
- [ ] If a kernel change ends up needed: add a Rust unit test in `swissarmyhammer-focus/tests/` covering the directional-nav case for siblings with off-rect or asymmetric-rect candidates.
- [ ] Run: `bun test perspective-tab-bar.spatial-nav.test.tsx` — green.
- [ ] Run: `cargo test -p swissarmyhammer-focus` — green.

## Workflow

- Use `/tdd` — instrument and reproduce first, identify root cause, then fix.
- **Stop and call `/plan` if** investigation reveals the fix spans multiple unrelated layers (e.g. kernel directional-nav rework + UI rendering change), making this exceed the single-concern sizing limit.