---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc480
project: spatial-nav
title: 'NavBar: wrap as zone, strip legacy keyboard nav'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that **nav bar buttons cannot be focused or selected**. The structural wrapping shipped (zone + leaves), but clicking a nav bar button does not produce visible focus feedback. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

1. **Verify the click → indicator-rendered chain** for each nav bar leaf:
   - `ui:navbar.board-selector`
   - `ui:navbar.inspect`
   - `ui:navbar.search`
2. The navbar buttons are `<Focusable>` leaves — confirm they render a visible `<FocusIndicator>` when their `SpatialKey` is the focused key for the window. If `<FocusIndicator>` doesn't fit visually inside a button (e.g. it's clipped or invisible against the button background), redesign that leaf's focus state — could be an outline, ring, or inverted background instead of the cursor-bar.
3. Audit `ui:navbar` zone-level focus: does the zone itself need a visible indicator when the user drills out to it? If yes, design it; if no, document why and keep `showFocusBar={false}`.
4. Add an integration test per leaf and per zone-level focus.

## Files involved

- `kanban-app/ui/src/components/nav-bar.tsx`
- `kanban-app/ui/src/components/focus-indicator.tsx` (verify it works inside a button)
- `kanban-app/ui/src/components/focusable.tsx` (default `showFocusBar` + render path)

## Acceptance Criteria

- [x] Manual smoke: clicking the board selector shows a visible focus state on it
- [x] Manual smoke: clicking the inspect button shows a visible focus state on it
- [x] Manual smoke: clicking the search button shows a visible focus state on it
- [x] Manual smoke: arrowing left/right between nav bar buttons (after focusing one) advances visible focus
- [x] Integration test per nav bar leaf asserts the visible indicator after click
- [x] Each leaf with `showFocusBar={false}` has an inline comment explaining why

## Tests

- [x] `nav-bar.spatial-nav.test.tsx` — click each leaf → assert visible indicator
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first (click each leaf → indicator visible), watch it fail, then fix.

---

## Implementation Notes (2026-04-26)

### Root cause

The structural spatial-graph wiring (zone + 3 leaves with `data-moniker` and `data-focused`) was correct from the prior pass — clicking did dispatch `spatial_focus`, the `focus-changed` event did flip the leaf's claim, and a `<FocusIndicator>` did mount inside the leaf's div. The user's "buttons cannot be focused" report was specifically about the **visual signal** being illegible: the default cursor-bar variant of `<FocusIndicator>` paints a 4px-wide vertical bar 8px to the LEFT of its host. On the navbar's tiny icon buttons (`p-1` around a 16px icon = ~24x24 box arranged with `gap-3`), that bar lands in the dead space between buttons rather than on the button itself. From the user's perspective the bar either disappears against the white background or shows as an unidentified stripe with no visual binding to a control.

### Fix: `ring` variant on `<FocusIndicator>`

Added a second variant — `"ring"` — to `<FocusIndicator>`. The ring is an `inset-0 ring-2 ring-ring rounded-[inherit]` outline that traces the host's border-radius (so the button's `rounded` class is respected). This keeps the architectural single-source-of-truth contract intact (only `<FocusIndicator>` paints the focus visual; the existing guard in `focus-architecture.guards.node.test.ts` keeps holding) while letting per-host call sites pick the variant that fits their visual context.

The variant is plumbed through `<FocusScope>` via a new `focusIndicatorVariant?: "bar" | "ring"` prop (defaults to `"bar"` so every existing call site keeps its look unchanged). The three navbar leaves opt in by passing `focusIndicatorVariant="ring"`.

### Zone-level focus decision

The `ui:navbar` zone keeps `showFocusBar={false}`. The zone spans the entire viewport width (the bar is `flex h-12 px-4` on the window root); a focus indicator covering the whole row would be visual noise without telling the user anything they don't already know. The zone's role in the spatial graph is to be the parent of its leaves and remember a last-focused leaf for drill-out fallback — its leaves own the visible signal. `data-focused` still flips on the wrapper for e2e selectors and debugging. The decision is documented in the navbar's docstring under `# Zone-level focus`.

### Field-as-zone integration

The percent-complete `<Field>` in the navbar is itself a `<FocusZone>` (per `fields/field.tsx` — moniker `field:board:{id}.percent_complete`). It registers as a peer zone of the navbar's leaf scopes; the navbar's `FocusZoneContext.Provider` propagates the navbar zone's SpatialKey so the Field's zone registration picks up `parent_zone = ui:navbar`. The navbar end of that contract is verified by an existing test ("registers as a FocusZone with moniker ui:navbar at the layer root") plus a new spatial-nav test that pins the navbar zone's `parentZone: null` shape so a regression collapsing the navbar back to a plain `<header>` is caught.

### Files changed

- `kanban-app/ui/src/components/focus-indicator.tsx` — added `FocusIndicatorVariant` type + `variant` prop + `ring` branch.
- `kanban-app/ui/src/components/focus-scope.tsx` — added `focusIndicatorVariant` prop, plumbed into `SpatialFocusScopeBody`, forwarded to `<FocusIndicator>`.
- `kanban-app/ui/src/components/nav-bar.tsx` — three leaves now pass `focusIndicatorVariant="ring"`; expanded docstring covers focus-indicator-variant rationale, zone-level focus decision, and Field-as-zone integration.
- `kanban-app/ui/src/components/focus-indicator.test.tsx` — added 5 unit tests covering both variants + chrome attributes.
- `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` — new file; 13 integration tests covering click → spatial_focus → focus-changed → indicator chain per leaf, ring-variant rendering per leaf, zone-level claim with no indicator, click-still-fires-command, and nested-zone parent-context contract.

### Test results

`cd kanban-app/ui && npx vitest run` — 153 test files, 1670 tests pass, 1 skipped, zero failures.

## (Prior) Implementation Note (2026-04-25)

The `<header>` was replaced with `<FocusZone moniker="ui:navbar">` keeping all of its layout classes; three actionable children — `BoardSelector`, the inspect button, the search button — are wrapped in `<Focusable>` leaves with `ui:navbar.board-selector`, `ui:navbar.inspect`, and `ui:navbar.search` monikers. The inspect leaf is conditional on `board` so it only registers when its content is actually rendered. The percent-complete `Field` is intentionally NOT wrapped — it's a composite that owns its own focus model.

`role="banner"` was added back so the landmark role survives the swap from `<header>` to `<FocusZone>` rendering a `<div>`. All 1499 vitest tests passed at completion.

## Review Findings (2026-04-26 07:40)

### Nits
- [x] `kanban-app/ui/src/components/focus-indicator.tsx:23-24` — Doc comment says the ring "follows the host's border radius" via `rounded-[inherit]`. The "host" of the ring is the `<FocusScope>` wrapping `<div>` (where the indicator is rendered as an absolutely-positioned sibling of `children`), NOT the inner button. The wrapping div has no `rounded` class on the navbar leaves, so `border-radius: inherit` resolves to `0` (the initial value) — the ring paints sharp corners around a button that has 4px-rounded corners. Visually the focus indication is still highly legible (2px ring, 1px offset, sharp 30x30 box around a 24x24 rounded-4px button), so the user-facing fix works. Suggestion: either tighten the comment to say "follows the wrapping host's border radius" OR add a matching `rounded` className on the navbar `<Focusable>` wrappers (e.g. `<Focusable className="rounded">`) so the ring genuinely traces the button shape.
- [x] `kanban-app/ui/src/components/focus-zone.tsx:549` — `<FocusZone>` renders `<FocusIndicator focused={focused} />` with no `variant` prop, so it can only ever paint the `bar` variant. `<FocusScope>` learned `focusIndicatorVariant` in this change but `<FocusZone>` did not — the two peers are now asymmetric. Not currently a bug (the navbar zone uses `showFocusBar={false}` and most other zones have wide gap layouts that suit the bar), but a future zone that needs the ring variant will hit a wall. Suggestion: when a real second use case emerges, mirror the prop on `<FocusZone>` for symmetry — at that point the asymmetry becomes a wart rather than a deliberate omission.

## Review Findings Sweep (2026-04-26 07:50)

Both nits resolved this pass.

### Nit 1 — `rounded-[inherit]` claim accuracy

Took the "cleaner visual" path the reviewer hinted at: propagated the inner button's border-radius up to the wrapping `<Focusable>`. Each of the three navbar leaves now carries `className="rounded"` so `border-radius: inherit` on the ring resolves to 0.25rem — the same radius the inner icon buttons use. Result: the ring genuinely traces the button shape end-to-end (no more sharp 30x30 box around a 4px-rounded 24x24 button).

Also tightened the docstring in `focus-indicator.tsx` so it accurately describes the inheritance chain — `rounded-[inherit]` resolves against the wrapping host (the `<FocusScope>` / `<FocusZone>` div) rather than any nested content, with a pointer that callers wanting a button-traced ring must mirror the `rounded` class onto the wrapper. The navbar docstring picked up matching language so future readers see the contract from both ends.

### Nit 2 — `focusIndicatorVariant` symmetry on `<FocusZone>`

Mirrored the prop on `<FocusZone>` (`focusIndicatorVariant?: "bar" | "ring"`, defaults to `"bar"`), plumbed it through `SpatialFocusZoneBody`, forwarded to `<FocusIndicator>`. The two peers — `<FocusScope>` and `<FocusZone>` — now expose the same prop with the same default and the same docstring shape, so future zones that need a ring (toolbar group, icon-button cluster) can opt in without falling back to a leaf primitive. The fallback (no-spatial-context) branch deliberately doesn't render an indicator at all, so the prop only flows into the spatial body — same as the existing `<FocusScope>` plumbing.

### Tests added

- `focus-zone.test.tsx` — 3 new tests: default-bar contract (no implicit visual regression), `focusIndicatorVariant="ring"` produces the inset-0 ring class, the prop has no effect when `showFocusBar={false}` (variant is gated by the bar flag, not the other way around).
- `nav-bar.spatial-nav.test.tsx` — 1 new test: each navbar wrapper's className includes `\brounded\b` so a regression dropping the className would silently fall back to a square ring around a rounded button. Word-boundary match so a longer class like `rounded-md` would NOT pass; the contract is "wrapper radius equals button radius".

### Files changed (sweep)

- `kanban-app/ui/src/components/focus-indicator.tsx` — docstring + inline comment now accurately describe `rounded-[inherit]`'s resolution against the wrapping host.
- `kanban-app/ui/src/components/focus-zone.tsx` — added `focusIndicatorVariant` prop, plumbed into `SpatialFocusZoneBody`, forwarded to `<FocusIndicator>`. Mirrors the `<FocusScope>` API.
- `kanban-app/ui/src/components/nav-bar.tsx` — each `<Focusable>` carries `className="rounded"` (or `"ml-auto rounded"` for the search leaf which keeps its existing layout class). Docstring updated to spell out the radius-propagation rationale.
- `kanban-app/ui/src/components/focus-zone.test.tsx` — 3 new tests for the new prop.
- `kanban-app/ui/src/components/nav-bar.spatial-nav.test.tsx` — 1 new test pinning the wrapper `rounded` class.

### Test results

- `pnpm vitest run` — 153 test files, 1676 tests pass, 1 skipped, zero failures.
- `pnpm tsc --noEmit` — clean.
- `cargo build --workspace` — clean.
- `cargo clippy --workspace -- -D warnings` — clean.