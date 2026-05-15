---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd080
project: spatial-nav
title: 'Regression suite: click on every component produces a visible focus indicator'
---
## What

Clicking a perspective tab does not produce a visible focus indicator. The release-blocker `01KQ5PEHWTEVTKPS2JHSZTXNBE` explicitly listed perspective tabs in its verification scope, was closed, and the bug is real. The existing per-component tests (`perspective-tab-bar.spatial-nav.test.tsx`, `column-view.spatial.test.tsx`, etc.) verify spatial **registration** but never click and assert the indicator. That's the gap.

This card commissions a single browser-mode regression suite that clicks every component class in the production app and asserts the click produces (a) a `spatial_focus` invoke against the registered key, (b) a `data-focused="true"` attribute on the clicked element after the focus-changed event fires, and (c) a `<FocusIndicator>` rendered as a child. The suite is the source of truth for "click works." It is independent of the unified-policy work in `01KQ7S6WHK9RCCG2R4FN474EFD` (which is about navigation); it ships and gates nothing.

### Why a regression suite, not a tactical "fix perspective tab click" card

The user has reported click-related focus failures three times across the project. Each time we've closed a per-component card without an integration test for click→indicator. Filing another tactical card for the perspective tab does not address the systemic gap: there is no test that proves clicks work everywhere. This card adds that test in one place, then fixes whichever components fail it.

### Components in scope

For each, the suite mounts the component (or its enclosing container) inside the standard provider stack, clicks the element matched by `[data-moniker="<expected>"]`, and asserts the indicator chain. Component classes (each gets one named test):

- **Task / tag card** (`task:<id>`, `tag:<id>`).
- **Column body** (`column:<id>`) — clicking on whitespace inside the column, not on a card.
- **Column name leaf** (header text — moniker `<column>.name` or whatever the production wiring uses).
- **Perspective tab** (`perspective_tab:<id>`) — the bug the user just reported.
- **Perspective bar background** (`ui:perspective-bar`) — should focus the bar zone.
- **Nav bar button** (`ui:navbar.search`, `ui:navbar.inspect`, `ui:navbar.board-selector`).
- **Toolbar action** (whatever the production wiring registers under `ui:toolbar.*`).
- **Inspector field row** (`field:task:<id>.<name>` inside an open inspector panel).
- **Inspector panel background** (`panel:task:<id>`).

For each, also assert the **negative** case: clicking on a parent zone whose child caught the click does NOT focus the parent (regression guard for `e.stopPropagation()` semantics).

### What this card does NOT do

- Does not change any navigation behavior. Click handling is independent.
- Does not assert anything about which component "should" be focused when the click lands on a Field zone inside a card — that's the click-target architectural question, separate from "does click produce ANY focus indicator." This card just asserts whatever the click-target rule is, the chosen target's indicator becomes visible.
- Does not test double-click, right-click, drag — those are separate concerns.

### Files in scope

**New file:**
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` — the regression suite. Contains one `describe` block per component class with one named `it` per class.

**Likely fixes (per failing component class):**

The implementer runs the new suite, sees which classes fail, and fixes each in the same PR. Common fix patterns:

- Inner button captures the click without bubbling: add `e.stopPropagation()` removal or rely on natural bubble. (Likely cause for perspective tab.)
- Spatial wrapper's onClick early-return rejects valid click targets: tighten the early-return.
- `<FocusIndicator>` not rendering: confirm `showFocusBar` is not false where it should be true; confirm the indicator's positioning isn't clipped.
- The registered SpatialKey doesn't match the key the click handler dispatches against: usually a remount-keyed-by-ref-loss issue.

If a fix grows beyond ~200 LOC for a single component class, file a separate dependent card and leave the suite test marked `it.skip("...component X — see card YYYY")` with the linked card ID.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` exists.
- [ ] One named test per component class above. Each test:
  - Mounts the component in production-shaped providers.
  - Clicks the `[data-moniker="<expected>"]` element via real `userEvent.click()` or `fireEvent.click()`.
  - Asserts exactly one `mockInvoke("spatial_focus", { key })` call against the captured registered key.
  - After firing `focus-changed` for that key, asserts `[data-moniker="<expected>"]` carries `data-focused="true"` and contains a `[data-testid="focus-indicator"]` descendant.
  - Asserts no parent zone's onClick fired (negative guard for stopPropagation).
- [ ] Every test in the suite passes. If a component is broken today, its fix lives in this card's PR (or in a dependent card linked from a `it.skip` placeholder).
- [ ] Specifically: clicking a perspective tab fires `spatial_focus(perspective_tab:<id>)` and renders the indicator. (The user-reported bug.)
- [ ] `cd kanban-app/ui && npm test` is green.

## Tests

This card IS the test. The acceptance criterion is that the new file exists and every named test passes against the current production code (with whatever fixes that requires).

### Setup

- Mock `@tauri-apps/api/core` and `@tauri-apps/api/event` per the canonical `vi.hoisted` pattern in `grid-view.nav-is-eventdriven.test.tsx`.
- Use a small fixture (1 board, 1 column with 2 cards, 2 perspectives, 1 inspector panel open) — does not need the full 3×3 fixture.
- Render inside `<UIStateProvider><PerspectivesProvider><ViewsProvider><BoardDataProvider><SpatialFocusProvider><FocusLayer name="window">…</FocusLayer></SpatialFocusProvider>…</UIStateProvider>` plus the providers each component needs.
- For each component class, capture the registered SpatialKey from the corresponding `mockInvoke("spatial_register_*", ...)` call so the test can fire `focus-changed` against the right key.

### How to run

```
cd kanban-app/ui && npm test -- focus-on-click.regression
```

Headless on CI.

## Workflow

- Use `/tdd`. Order:
  1. Build the suite skeleton with all named tests `it.todo`.
  2. Implement the perspective tab test first (the user-reported bug). Run it. Diagnose the failure mode.
  3. Fix the perspective tab. Move the test from `it.todo` → green.
  4. Implement and pass the rest of the component classes one at a time. For each failing class, either fix in this PR or skip with a linked dependent card.
  5. Confirm all named tests are green or explicitly skipped with linked cards.
- This card ships independently. It is NOT blocked by the unified-policy card, the `<Inspectable>` refactor, or the `<Focusable>` deletion. It tests current production code; whatever the click-handling architecture is today, the suite asserts the user-visible result.

## Review Findings (2026-04-27 12:55)

### Blockers
- [x] `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx:234` — Unused `FocusScope` import. `pnpm tsc --noEmit` fails on this file with `error TS6133: 'FocusScope' is declared but its value is never read.` The project's `kanban-app/ui/tsconfig.json` enables `noUnusedLocals: true`, so this is a hard typecheck failure, not just lint noise. The file uses `<FocusZone>` directly in `renderColumnInBoard` and `renderInspectorPanelZone`; `FocusScope` is referenced only in JSDoc and inline comments (lines 25, 63, 548, 778, 817, 844, 1039, 1055, 1071), never as a JSX element. Drop the import. Also drop the import-alias references from line 234 only — the JSDoc/inline-comment references can stay as documentation.

### Nits
- [x] `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx:1033` — Toolbar `it.skip` title reads `"clicking a toolbar action focuses it and renders the indicator"` with no signal that it's intentionally skipped because production has no toolbar yet. The inline comment in the body explains the reason, but a reader scanning vitest's skip output (or the test file's structure) will not see why. Consider either (a) appending `" — production has no toolbar component today"` to the `it.skip` title, or (b) filing a tiny tracking card for "wire up toolbar focus contract when toolbar lands" and putting its ID in the skip title per the card-description protocol.