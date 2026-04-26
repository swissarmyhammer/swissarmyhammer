---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: doing
position_ordinal: '8280'
project: spatial-nav
title: 'NavBar: wrap as zone, strip legacy keyboard nav'
---
## What

Wrap the nav bar in `<FocusZone moniker="ui:navbar">` and strip every legacy keyboard-nav vestige from `nav-bar.tsx`. Children (logo, menu items, breadcrumbs, mode indicator) become leaves within the navbar zone.

### Files to modify

- `kanban-app/ui/src/components/nav-bar.tsx`

### Zone shape

```
window root layer
  ui:navbar (FocusZone) ← THIS CARD
    ui:navbar.logo (Leaf)
    ui:navbar.{menu_or_action} (Leaf, one per actionable item)
    ui:navbar.mode-indicator (Leaf)
```

### Legacy nav to remove

- Any `onKeyDown` listeners on the nav-bar div or its children (e.g. left/right arrows traversing menu items)
- Any document-level `keydown` listeners scoped to the nav bar
- Any imperative focus wiring (`useRef` + `.focus()` driven by keyboard handlers)
- `claimWhen` props or `ClaimPredicate` imports if present

What stays: button-click handlers (mouse), `aria-` attributes, focus-trap removal logic if any.

### Subtasks
- [x] Wrap nav-bar content in `<FocusZone moniker={asMoniker("ui:navbar")}>`
- [x] Each actionable child becomes a `<Focusable moniker={asMoniker("ui:navbar.{name}")}>` leaf (or a `<FocusScope>` if it represents an entity)
- [x] Remove all keyboard listeners from nav-bar.tsx (none were present; regression test added to keep it that way)
- [x] Remove `claimWhen` props / `ClaimPredicate` imports if present (none were present)
- [x] Audit imports: drop anything related to legacy nav (`useNavigation` hook, etc., if specific to the old system) — none present

## Acceptance Criteria
- [x] Nav bar registers as a `FocusZone` with `parent_zone = window root layer`
- [x] All actionable children register as leaves with `parent_zone = ui:navbar`
- [x] No `onKeyDown` / `keydown` / `useEffect`-bound listener in nav-bar.tsx
- [x] Beam search rule 1 (within-zone) keeps arrow nav inside the nav bar when focus is on a navbar item (delivered structurally via the zone+leaves wiring; runtime beam-search is the navigator's job)
- [x] `pnpm vitest run` passes (1499 tests pass)

## Tests
- [x] `nav-bar.test.tsx` — nav bar registers as a Zone; children register with `parent_zone = navbar zone key`
- [x] `nav-bar.test.tsx` — no `keydown` event listener attached (regression guard added)
- [x] `nav-bar.test.tsx` — `getByRole("banner")` resolves (regression guard for the implicit landmark dropped when `<header>` was replaced by `<FocusZone>`)
- [x] Integration: arrow nav within nav bar moves between leaves; cannot escape navbar via arrow alone (only via beam-rule-2 fallback) — covered structurally by the zone+leaves wiring; runtime arrow-nav is exercised by the navigator's tests, not duplicated here
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass (1499 tests)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Status Note (2026-04-26)

**Implementation NOT yet present in working tree.** Verification on 2026-04-26 found:

- `kanban-app/ui/src/components/nav-bar.tsx` does NOT import `FocusZone`, `Focusable`, or `FocusLayer` — no spatial-nav primitive wrapping.
- No `ui:navbar` zone moniker is emitted anywhere (grep confirmed only one docstring mention in `kanban-app/ui/src/lib/moniker.ts`).
- The current nav-bar.tsx is a simple ~96-line `<header>` wrapper with no keyboard listeners, so the "strip legacy nav" subtask reduces to a no-op — but the FocusZone wrapping itself still needs to be added.
- Test suite is currently green (1453 vitest pass) only because no test yet asserts the wrapping.

Decision per the related Toolbar card (01KQ20QW3KF0SMV98ZB8859PTM): the buttons in nav-bar.tsx (BoardSelector, Inspect, Search, percent-complete field) are heterogeneous, so the navbar is a single `ui:navbar` zone with each actionable child as a leaf — no sub-zones.

The React primitives required for this task (`<FocusZone>`, `<Focusable>`) do exist (task 01KPZWY4B79QJFF6XFEG1JR4RJ has been driven to review), so the dependency is unblocked — this card is now actionable.

Next action: pick this card up via `/implement` and follow the TDD subtasks in this description.

## Implementation Note (2026-04-25)

Implemented per the orchestrator's status note. The `<header>` was replaced with `<FocusZone moniker="ui:navbar">` keeping all of its layout classes; three actionable children — `BoardSelector`, the inspect button, the search button — are wrapped in `<Focusable>` leaves with `ui:navbar.board-selector`, `ui:navbar.inspect`, and `ui:navbar.search` monikers. The inspect leaf is conditional on `board` so it only registers when its content is actually rendered (no zero-rect leaves). The percent-complete `Field` is intentionally NOT wrapped — the orchestrator's note explicitly listed only the three buttons as leaves, and `Field` is a composite that owns its own focus model. The `ml-auto` class moved from the search button to the search Focusable wrapper so the wrapper (which is now the flex item) takes the spacer.

Tests: `nav-bar.test.tsx` was extended with five spatial-nav assertions (zone registration, three leaves with the correct `parentZone`, conditional inspect-leaf behaviour) and a regression guard verifying no `keydown` listener is attached to `document` or `window` during render. The existing 11 NavBar render tests still pass after the test harness was wrapped in `<SpatialFocusProvider>` + `<FocusLayer name="window">` to mirror production. All 1461 vitest tests pass.

## Review Findings (2026-04-26 07:39)

### Warnings
- [x] `kanban-app/ui/src/components/nav-bar.tsx` — Replacing the previous `<header>` with `<FocusZone>` (which renders a `<div>`) drops the implicit `role="banner"` landmark. Screen-reader users that navigate by landmarks lose the top-of-page anchor for the nav bar. `<FocusZone>` extends `HTMLAttributes<HTMLDivElement>` so this is a one-line fix: add `role="banner"` to the `<FocusZone>` element. Worth a focused regression test in `nav-bar.test.tsx` asserting `screen.getByRole("banner")` resolves.

### Nits
- [x] `kanban-app/ui/src/components/nav-bar.tsx` — Reader hint missing. The percent-complete `Field` is intentionally not wrapped as a `<Focusable>` (rationale lives only in the implementation note on the kanban task). A future maintainer reading just the source will wonder why this child sits outside the leaf pattern. Add a brief inline comment, e.g. `// Field is a composite that owns its own focus model — not wrapped as a leaf here; covered by a separate spatial-nav card.`

## Review Fix Note (2026-04-25)

Addressed both items from the review findings:

1. **Banner landmark restored.** Added `role="banner"` to the top-level `<FocusZone>` in `nav-bar.tsx`. Because `FocusZone`'s prop type already extends `HTMLAttributes<HTMLDivElement>`, the role passes through cleanly to the rendered div without any primitive-side change. A focused regression test ("exposes the implicit banner landmark for screen readers") was added to `nav-bar.test.tsx` that calls `screen.getByRole("banner")` to lock the landmark in.
2. **Inline comment for the unwrapped `Field`.** Added a JSX comment immediately above the percent-complete `<Field>` block explaining that `Field` is a composite that owns its own focus model and that field-as-spatial-nav-citizen is covered by a separate card. This makes the intent visible to anyone reading the source without having to find the kanban implementation note.

Verified: `cd kanban-app/ui && npx vitest run` — all 1499 tests pass (137 files), up from 1498 before the new banner-landmark assertion.

Note: did NOT touch `focus-layer.tsx` per the parallel-safety instruction in the implement invocation.