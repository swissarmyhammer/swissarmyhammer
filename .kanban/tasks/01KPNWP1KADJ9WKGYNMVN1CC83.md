---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff580
project: spatial-nav
title: 'Card: spatial nav into card sub-parts (tags, assignees, title, fields on a card)'
---
## Design decisions (documented 2026-04-20)

1. **Enter key**: `l` (right) from a focused card enters its first sub-part. No new command — composes with existing board nav contract.
2. **Registration strategy**: always-registered, with `parent_scope` threading through `FocusScope`. Container-first search in Rust keeps nav inside the focused card's sub-parts. Profile later if 500+ pills on large boards proves costly.
3. **h/j/k/l semantics while pill focused**: Rust container-first search handles siblings first, then falls through to full-layer beam test. `h` from first sibling on a card falls through to the card body (no sibling to the left); `j` from a card's last pill falls through to another card's body.

## What

Cards in the board view show a title plus zero or more sub-parts — tag pills, assignee pills, due dates, etc. The spatial nav system should let the user step INTO a card's sub-parts, not just between cards.

Today (verified in the dev app):
- Clicking a tag pill inside a card sets focus on the tag moniker (e.g. `tag:01KN9K546V...`).
- Clicking the card's body sets focus on the task moniker.
- But there is no h/j/k/l path from a focused card to its internal tag/assignee pills; the user has no way to navigate into a card's interior.

### Harness / infrastructure already available (2026-04-20 19:45)

Landed by `01KPNWGFTF` + `01KPNWNEN1`:
- `kanban-app/ui/src/test/spatial-shim.ts`, `setup-spatial-shim.ts`
- `spatial-fixture-shell.tsx` — shared FixtureShell + FixtureKeybindingHandler
- `spatial-board-fixture.tsx` — 3 columns x 3 cards board
- `spatial-nav-board.test.tsx` — proven card-to-card nav pattern

The `spatial?: boolean` prop on `FocusScope` is already in place (from `01KPNWH82X`) and the Rust `navigate()` already implements container-first search with a `parent_scope` parameter. Both pieces the fix needs are already wired — this task is largely about threading `parent_scope` through from React.

### TDD — failing tests

Under `kanban-app/ui/src/test/spatial-nav-card-subparts.test.tsx`:

```tsx
describe("card interior navigation", () => {
  it("l from a focused card moves focus to the first tag pill on that card");
  it("h from a pill moves back to the card body");
  it("j from a focused pill does NOT reach pills on a different card");
  it("l through sibling pills on the same card");
});
```

### Approach

1. **Thread `parent_scope` through `spatial_register`**. Pills inside a card render inside the card's FocusScope. The card's moniker is their `parent_scope`. `focus-scope.tsx` currently passes `parent_scope: null` on every invoke — update it to read the nearest ancestor FocusScope moniker from `FocusScopeContext` and pass that.
2. **Verify container-first in Rust**. `SpatialState::navigate()` already supports the container-first search — unit-tested in `swissarmyhammer-spatial-nav`. No Rust change expected; confirm via a parity case added to `spatial-parity-cases.json`.
3. **Update JS shim** to match whatever Rust does (should be no-op if Rust is unchanged). Parity test will catch drift.
4. **Fixture**: extend `spatial-board-fixture.tsx` so one card has two tag pills (wrap them in FocusScopes), and assert the above scenarios.

### Acceptance

- [x] Design decision documented at the top of this task body (update description when agreed)
- [x] Failing test file lands first, all 4 cases red
- [x] Implementation makes each pass one-at-a-time
- [x] No regression in card-to-card board nav (01KPNWNEN1's tests stay green)
- [x] `parent_scope` is now threaded through `FocusScope` — spatial-parity-cases exercises the container-first path

### Implementation notes (2026-04-20 — assignee claude-code)

**Files changed:**
- `kanban-app/ui/src/components/focus-scope.tsx`: threaded `parent_scope` via `useContext(FocusScopeContext)` into `useSpatialClaim`; forwarded to `spatial_register` as `parentScope` (previously hard-coded `null`).
- `kanban-app/ui/src/components/focus-scope.test.tsx`: added unit test `nested FocusScope threads parent_scope moniker through spatial_register` — outer scope registers with `parentScope: null`, nested inner scope registers with the outer moniker.
- `kanban-app/ui/src/test/spatial-board-fixture.tsx`: extended fixture. `card-1-1` now renders as a `FocusScope(renderContainer=false)` wrapping a narrow body div + a horizontal `FixtureTagRow` with two pill FocusScopes. Other cards retain the original pill-free shape.
- `kanban-app/ui/src/test/spatial-nav-card-subparts.test.tsx`: 4 scenario tests + 1 discriminator test asserting pills register with the card moniker as `parent_scope`.
- `kanban-app/ui/src/test/spatial-parity-cases.json`: new case `card-interior container-first: pill stays within card siblings before falling through`.

**Test results:**
- `spatial-nav-card-subparts.test.tsx`: 5/5 pass
- `spatial-nav-board.test.tsx`: 4/4 pass
- `spatial-shim-parity.test.ts`: 12/12 pass (new parity case green)
- `cargo test -p swissarmyhammer-spatial-nav`: 50 unit + 1 parity pass
- `focus-scope.test.tsx`: 39/39 pass
- `spatial-nav-grid`, `spatial-nav-inspector`, `spatial-nav-leftnav`, `spatial-nav-canonical`, `data-table`, `column-view`: all green

**Not touched (per task boundary):** `left-nav.tsx`, `perspective-tab-bar.tsx`, `data-table.tsx`, `entity-focus-context.tsx`. Pre-existing failures in `spatial-nav-perspective.test.tsx` reproduce without my focus-scope.tsx edits and are in the parallel agent's territory.

## Review Findings (2026-04-20 16:20)

### Nits
- [x] `kanban-app/ui/src/test/spatial-nav-card-subparts.test.tsx:136-161` — The "j from a focused pill does NOT reach pills on a different card" test uses a conditional assertion that passes vacuously if focus stays on the same pill or moves to any non-pill target; only the narrow "pill on another card" outcome is forbidden. That's documented in the comment and is arguably the only strict invariant the geometry supports, but as a scenario test it is weak — the companion "pill spatial entries carry the enclosing card moniker as parent_scope" test (lines 179-214) is what actually exercises the wiring under test. Consider strengthening the `j` scenario to assert a concrete expected outcome (e.g. "focus stays on `pill1` because container-first finds no sibling below and no non-sibling pill is in the Down beam") rather than leaving the acceptable outcomes open-ended, or remove the scenario test in favor of the direct discriminator test.

### Nit fix (2026-04-20 16:28 — assignee claude-code)

Strengthened the scenario test. Renamed to `"j from a focused pill falls through to the card body directly below"` and replaced the conditional `if (focused?.startsWith("tag:"))` guard with a concrete `expectFocused(cardBelow)` poll plus an equality assertion against `FIXTURE_CARD_MONIKERS[0][1]` (`task:card-1-2`). Determined the expected target empirically (shim-captured focused moniker = `task:card-1-2`) and confirmed it against the geometry: container-first finds no sibling pill in the Down direction (pill2 is to the right, not below), so the full-layer beam test picks the nearest candidate below — `card-1-2`'s body, which spans the full column width. The test now fails if either the container-first search regresses (e.g. returns another card's pill) or the fallback picks the wrong body.

Updated the file header comment to match the new expected outcome. Scenario test now pins a single concrete outcome; the discriminator test at lines 179-214 still covers the `parent_scope` wiring directly. 5/5 tests pass in `spatial-nav-card-subparts.test.tsx`; 55/55 pass across `spatial-nav-board.test.tsx`, `focus-scope.test.tsx`, and `spatial-shim-parity.test.ts`.