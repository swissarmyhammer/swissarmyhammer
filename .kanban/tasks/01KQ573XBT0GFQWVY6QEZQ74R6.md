---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb980
project: spatial-nav
title: 'Focus state: single-source-of-truth from Rust, no dual data-attrs, single decorator'
---
## What

After the spatial-nav refactor, clicking a focusable (e.g. clicking a column header) does NOT show any visible focus marker. The visual decoration is still wired through legacy `data-focused` attributes and `data-moniker` matching, with React reading state from the DOM rather than the Rust-side `SpatialState`.

This is an architectural problem, not just a missing class. Address it as a cleanup pass once the rest of the spatial-nav epic lands so we don't bake the antipattern in further.

## Architectural rules

1. **Rust is authoritative for focus state.** `SpatialState::focus_by_window` is the single source of truth. The frontend never reads or writes focus state through DOM attributes.

2. **No dual data attributes.** Today we have `data-moniker`, `data-focused`, `data-zone-moniker`, `data-cell-cursor`, etc. — drop the ones that exist for the renderer to read state. They're acceptable as renderer outputs (debugging hooks, e2e selectors) but NOT as state inputs.

3. **One decorator, one place.** The visible focus indicator (ring, highlight, cursor) renders in exactly one component. Right now we have multiple: `FocusHighlight` (deleted but referenced by stale comments), `data-focused` styling on `Focusable`/`FocusZone`, `cursor-ring` on grid cells, `FocusScopeBody` chrome on the entity-aware `FocusScope`, etc. Pick the right layer and consolidate.

4. **State in React, not the DOM.** `useFocusClaim(key, callback)` already gives React the focus state per primitive. Render `data-focused` (output-only) from that React state. Don't have CSS rules that read `[data-moniker="..."][data-focused]` — that's a covert state channel.

## Concrete subtasks

- [x] Audit `kanban-app/ui/src/` for CSS / Tailwind selectors that read `[data-moniker]`, `[data-focused]`, `[data-zone-moniker]`, `[data-cell-cursor]` — every one is a candidate for consolidation
- [x] Identify the "decorator" component — likely the existing `<Focusable>` / `<FocusZone>` primitives, since they own `useFocusClaim`. They render the visible focus indicator from React state, not from DOM-read attrs
- [x] Delete duplicate decorator paths: legacy `FocusScopeBody` chrome, separate `cursor-ring` class application in `data-table.tsx`, any `data-focused`-driven CSS rules
- [x] Verify clicking a column header / card / pill / cell shows a visible focus indicator
- [x] Verify nav between focusables updates the indicator without reading the DOM
- [x] Verify the decorator is exactly one place: grep finds one place that renders ring/outline/highlight on focus

## Acceptance Criteria

- [x] Clicking any focusable shows a visible focus indicator
- [x] React, not the DOM, drives focus visuals — `useFocusClaim` callback flips a React state that controls the className
- [x] `data-focused` and `data-moniker` are output-only (used by tests / e2e selectors, not by CSS or React)
- [x] Exactly one component renders the visual focus indicator (one place to change ring color / shape / animation)
- [x] No CSS rules in `*.css` or Tailwind utilities read `[data-focused]` / `[data-cell-cursor]` etc.
- [x] Spatial-nav arrow-key navigation still updates the visible focus correctly
- [x] `pnpm vitest run` passes

## Why this matters

The current shape couples React rendering to DOM-attribute mutations driven by the spatial-nav events. That's two state stores (React state + DOM attrs) and two renderers (React → attrs → CSS) for the same fact. Bugs in the wiring (like the current "click column shows no focus" symptom) are inevitable.

The clean shape is:
- Rust event → React state (via `useFocusClaim`) → React className → visible decoration.
- DOM attrs are emitted by React for tests/debugging, never read back as state.

## Tests

- [x] Click a focusable, assert React state observably reflects "focused" before any CSS / DOM-attr check
- [x] Spatial nav (Rust event) updates the focused element's React state; visible decoration follows
- [x] No CSS rule depends on `[data-focused]` / `[data-cell-cursor]` (grep test)
- [x] Exactly one component owns `ring` / `outline` / focus-highlight class application (grep test)

## Workflow

- Use `/tdd` — write the grep guard tests first (they will reveal the antipatterns), then consolidate.

## Origin

User direction during `/finish $spatial-nav` recovery, 2026-04-26: "Focus state from the rust side needs to be authoritative — no additional or dual required css data attributes nested inside the focus scope. The decorator needs to be in one place, and only modify state in one place. Don't rely on data attributes for state — we have react thanks."

This is intentionally tagged for "check last" — it's a cleanup pass after the rest of the spatial-nav epic lands, not a blocker for in-flight cards.

## Implementation summary (2026-04-26)

- Added `<FocusIndicator>` (`kanban-app/ui/src/components/focus-indicator.tsx`) — the single visible focus decorator. Renders an absolutely-positioned bar from React state when `focused` is true, renders nothing otherwise. `pointer-events-none` so it never intercepts clicks; `aria-hidden` so screen readers don't announce a duplicate signal.
- Updated `<Focusable>` and `<FocusZone>` to:
  - Subscribe to `useFocusClaim` (FocusZone now does this, matching Focusable).
  - Render `data-focused` from the React `focused` state — output-only, never read by CSS.
  - Render `<FocusIndicator focused={focused} />` as the first child.
  - Merge `relative` className so the absolutely-positioned indicator positions against the primitive.
  - Accept a `showFocusBar` prop (default `true`) that suppresses the visible indicator without affecting `data-focused`.
- Removed CSS rules in `index.css` that read `[data-focused]` (`[data-focused]::before`, `.column-header-focus[data-focused]::before`, `.mention-pill-focus[data-focused]::before`).
- Deleted `<FocusHighlight>` and its test (`kanban-app/ui/src/components/ui/focus-highlight.{tsx,test.tsx}`) — the legacy duplicate decorator path.
- Removed dead className references: `column-header-focus` (column-view.tsx), `mention-pill-focus` (mention-view.tsx), `entity-card-focus` (entity-card.tsx).
- Threaded `showFocusBar` from `<FocusScope>` through `FocusScopeChrome` to the underlying primitive — so callers' opt-out now actually suppresses the visual decoration in production (previously it only suppressed the entity-focus scrollIntoView effect).
- Set `showFocusBar={false}` on the container zones that don't want a bar around their entire body: `ui:board`, `ui:grid`, `ui:view`, `ui:perspective`, `ui:navbar`, `ui:perspective-bar`, the column body, and the inspector entity scope.
- Architectural guard tests (`kanban-app/ui/src/components/focus-architecture.guards.node.test.ts`):
  - No CSS file reads `[data-focused]` / `[data-cell-cursor]` / `[data-moniker]` / `[data-zone-moniker]`.
  - Only the spatial primitives may render `<FocusIndicator>`, and BOTH must render it (so a regression deleting it from one path is caught).
  - `FocusHighlight` references stay deleted.
- React-state-driven test: `<Focusable>` and `<FocusZone>` tests verify that a Rust-side `focus-changed` event flips React state which renders `<FocusIndicator>` as a child of the primitive's div — closing the loop end-to-end without any DOM-attr read.

Tests: 144 files, 1578 tests, 0 failures (was 143 / 1571 baseline; +12 new tests, −5 from FocusHighlight deletion).

Files touched:
- `kanban-app/ui/src/components/focus-indicator.tsx` (new)
- `kanban-app/ui/src/components/focus-indicator.test.tsx` (new)
- `kanban-app/ui/src/components/focus-architecture.guards.node.test.ts` (new)
- `kanban-app/ui/src/components/focusable.tsx` (renders indicator, merges relative class, adds showFocusBar)
- `kanban-app/ui/src/components/focusable.test.tsx` (added React-state and showFocusBar tests)
- `kanban-app/ui/src/components/focus-zone.tsx` (subscribes to useFocusClaim, renders data-focused + indicator, adds showFocusBar)
- `kanban-app/ui/src/components/focus-zone.test.tsx` (added focus claim, indicator, and showFocusBar tests)
- `kanban-app/ui/src/components/focus-scope.tsx` (forwards showFocusBar to primitive)
- `kanban-app/ui/src/components/board-view.tsx`, `grid-view.tsx`, `view-container.tsx`, `perspective-container.tsx`, `nav-bar.tsx`, `perspective-tab-bar.tsx` (showFocusBar={false} on container zones)
- `kanban-app/ui/src/components/column-view.tsx`, `mention-view.tsx`, `entity-card.tsx` (removed dead css classes; column body opts out of focus bar)
- `kanban-app/ui/src/components/data-table.tsx` (updated stale docstring about cursor-ring CSS — there is no CSS, the ring is React-state-driven)
- `kanban-app/ui/src/index.css` (deleted [data-focused] rules)
- `kanban-app/ui/src/components/ui/focus-highlight.tsx` (deleted)
- `kanban-app/ui/src/components/ui/focus-highlight.test.tsx` (deleted)

## Review Findings (2026-04-26 13:45)

### Warnings
- [x] `kanban-app/ui/src/components/data-table.tsx:886` — Grid-cell `<Focusable>` does not pass `showFocusBar={false}`. When a grid cell gains spatial focus, the `useFocusClaim`-driven `<FocusIndicator>` bar renders inside the `<Focusable>` AT THE SAME TIME the cursor ring (`ring-2 ring-primary ring-inset`, driven by `isCursor` on the outer `<TableCell>`) renders. The task's third architectural rule — "One decorator, one place" — explicitly listed `cursor-ring on grid cells` as a duplicate decorator to consolidate. The implementation kept the ring but did not suppress the bar on the cell-level primitive, so production grid cells now show two simultaneous focus decorations driven by two different state stores (entity-focus → cursor ring, spatial-focus → focus bar). Fix: pass `showFocusBar={false}` to the `<Focusable>` in `GridCellFocusable` since the cell ring is the canonical "cell focus" visual — or alternatively, delete the `isCursor && "ring-2 ring-primary ring-inset"` ring application and rely on the focus bar alone (the original task wording suggests the latter). Add a guard test that asserts a focused grid cell renders exactly one focus visual (one ring OR one bar, not both).

### Nits
- [x] `kanban-app/ui/src/components/focus-scope.tsx:374-388` — The no-spatial-context fallback emits `data-focused` but does not render `<FocusIndicator>`. With `[data-focused]` CSS rules deleted, this means tests mounting `<FocusScope>` without a `<FocusLayer>` get no visible focus decoration at all. The existing comment explains the path is for tests, but does not note that the focus bar is intentionally absent in this branch. Add one sentence: "Visible focus bar is intentionally not rendered here — this path runs only in unit tests that don't stand up the spatial provider stack; the test asserts `data-focused` directly."

## Review-finding fixes (2026-04-26 second pass)

### Warning: removed cell-spanning cursor ring (Option B per the review)

The task explicitly listed `cursor-ring on grid cells` as a duplicate decorator to consolidate (architectural rule 3). The reviewer offered two choices and noted the task wording suggested deleting the ring. Chose Option B: the canonical `<FocusIndicator>` is now the sole decorator on grid cells, identical to every other `<Focusable>` in the app. Specifically:

- **Removed** the `isCursor && "ring-2 ring-primary ring-inset"` className from `cellClasses` in `DataBodyCell` (`kanban-app/ui/src/components/data-table.tsx`). The selection-range tint (`isSel && !isCursor && "bg-primary/10"`) is unrelated to focus and stays.
- **Updated** stale jsdoc / comments in `data-table.tsx`, `grid-view.tsx`, and `lib/moniker.ts` that referenced the cursor-ring as the visible decoration. The `data-cell-cursor` attribute remains as an output-only debug/e2e selector — its docstrings now make clear no CSS reads it and the visible bar comes from `<FocusIndicator>`.
- **Added** a guard test in `kanban-app/ui/src/components/grid-view.cursor-ring.test.tsx` (new `describe("GridView -- single-focus-visual on a focused cell")` block) that mounts the full provider stack, drives a `focus-changed` event for a target cell's `SpatialKey`, and asserts:
  1. Exactly one `<FocusIndicator>` (`[data-testid='focus-indicator']`) is rendered, and it is a descendant of the focused cell's `<Focusable>`.
  2. No element carries the removed `.ring-2.ring-primary.ring-inset` classes.

This requires the `listen()` mock in that test file to capture handlers (it previously returned a no-op); the existing `data-cell-cursor` tests don't touch `listenHandlers` so they are unaffected.

### Nit: documented no-spatial-context fallback in focus-scope

Added a paragraph to the no-spatial-context branch of `FocusScopeChrome` (`focus-scope.tsx`) explicitly noting that the visible focus bar is intentionally NOT rendered there, that the branch runs only in unit tests that don't stand up the spatial provider stack, and that production never enters this branch.

### Tests

`pnpm vitest run`: 144 files, 1579 tests, 0 failures (was 1578 before this pass; +1 for the new single-focus-visual guard).
`pnpm tsc --noEmit`: clean.

### Files touched (this pass)

- `kanban-app/ui/src/components/data-table.tsx` — removed cell ring, updated docstrings
- `kanban-app/ui/src/components/focus-scope.tsx` — documented no-spatial-context fallback
- `kanban-app/ui/src/components/grid-view.cursor-ring.test.tsx` — added single-focus-visual guard, captured `listen` handlers
- `kanban-app/ui/src/components/grid-view.tsx` — updated cursor-derivation comment to reference `<FocusIndicator>` instead of cursor ring
- `kanban-app/ui/src/lib/moniker.ts` — updated `parseGridCellMoniker` jsdoc to reference `data-cell-cursor` (debug attr) instead of "cursor-ring derivation"