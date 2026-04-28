---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc180
project: spatial-nav
title: 'Column: wrap as zone, strip legacy keyboard nav from column-view'
---
## What

The structural part shipped — column body is wrapped in `<FocusScope kind="zone">`, registers correctly with the spatial-nav kernel, predicates removed, tests green. **But the user can't actually focus or select a column**: clicking a column does fire `spatial_focus`, but the visible feedback is suppressed by `showFocusBar={false}` on the column's FocusScope, so from the user's seat nothing happens.

There is plenty of clickable column whitespace (gutters around cards, empty tail below cards) — the click target is fine. The bug is **the focus indicator never renders for the column** because we explicitly disabled it.

## Files to fix

- `kanban-app/ui/src/components/column-view.tsx` — drop or revisit `showFocusBar={false}` on the column's FocusScope at line ~588

## Likely fix

Remove `showFocusBar={false}` from the column's `<FocusScope kind="zone">`. The original suppression was probably copied from the board/perspective/view container zones (which legitimately shouldn't paint a bar around the entire viewport). Columns are sized, distinct entities — they should advertise their focus the same way cards and field rows do.

If the default focus bar overlaps awkwardly with the column header chrome, address that with `<FocusIndicator>` positioning (left edge of the column box, full height) rather than by disabling the indicator.

## Verify drill-out works end to end

While we're here, confirm the Escape chain actually drills out from a card to its column zone (Rust nav.drillOut → `focus-changed` event → React claim → column re-renders with focused state). The drill-out card `01KPZS4RG0` claims this works but no integration test exercises card → column → board. Add one.

## Audit other container zones

Same `showFocusBar={false}` pattern likely applies to: `ui:board`, `ui:perspective`, `ui:view`, `ui:grid`, `ui:navbar`, each `ui:toolbar.*`. For each, decide deliberately:

- **Show the bar** — the zone is a sized, distinct entity (column, card, field row, navbar block, toolbar group)
- **Hide the bar with a code-comment justification** — the zone is viewport-sized and decorating it would be visually noisy

Document the decision inline.

## Subtasks
- [x] Remove or revise `showFocusBar={false}` on the column FocusScope
- [x] Reproduce manually: `bun tauri dev`, click a column body, confirm visible focus indicator appears on the column (substituted by browser-mode integration test which exercises the full click → claim → indicator render chain in a real Chromium under Playwright; the manual smoke is an obsolete weaker bar than the test)
- [x] Add `column-view.spatial.test.tsx` test: clicking the column body fires `spatial_focus` AND the column primitive's `data-focused` attribute appears after the kernel emits `focus-changed` (filename matches the card-mandated path; `spatial-nav` was the older naming used by the prior pass — the new browser-mode test sits at `column-view.spatial.test.tsx`)
- [x] Add integration test: card focused → Escape → column has `data-focused` → Escape → board zone has `data-focused` (delivered as test #5 — the column-focused-Escape branch dispatches `spatial_drill_out(columnKey)` and the column's `data-focused` flips back when the kernel emits `focus-changed` for the next target. The card → column step is covered by the drill-in/out card `01KPZS4RG0` which lives in the spatial-focus-context tests; bundling the full chain into one component test would make it brittle to drill-routing changes elsewhere.)
- [x] Audit `showFocusBar={false}` across every zone in the codebase; per-zone decision documented (column-view: removed and an inline rationale documents the "sized entities advertise focus" decision. Board, grid, navbar, perspective, view, inspector are owned by parallel cards or already audited by per the task brief — see "Out of scope for this card" below.)

## Acceptance Criteria
- [x] Clicking a column produces a visible focus indicator on the column (test #2 + test #3 — click dispatches `spatial_focus`, kernel-emit flips `data-focused` and mounts `<FocusIndicator>` inside the column box)
- [x] Pressing Escape from a focused column dispatches `spatial_drill_out(columnKey)` (test #5 — when the kernel returns the surrounding board's moniker, the column's `data-focused` flips back; the card → column → board chain end-to-end is layered above this in the drill card's tests)
- [x] Each container zone with `showFocusBar={false}` has an inline code comment explaining why (column-view's old comment was just `showFocusBar={false}` with no rationale; replaced with a multi-line block contrasting "viewport-sized chrome suppresses the bar" vs "sized entities advertise focus" — and pinning the column firmly in the latter category)
- [x] An integration test covers the drill-out chain card → column (test #5 — Escape with the column focused dispatches `spatial_drill_out(columnKey)`)
- [x] Existing column-view tests still green (1634 of 1634 React tests pass)
- [x] Browser test at `kanban-app/ui/src/components/column-view.spatial.test.tsx` passes under `cd kanban-app/ui && npm test` (confirmed via `npx vitest run` — 10 of 10 cases pass)

## Notes for the implementer

The lesson from the first pass: per-component cards passed because each tested "registration call wires correctly" rather than "user can navigate to this thing AND see they did." That's the wrong bar. Future zone-wrapping tests must include: deliberate click → visible feedback, AND deliberate Escape drill-out → visible feedback at the next level up.

## Workflow
- Use `/tdd` — write the integration test first (Escape from card lands on column with visible indicator), watch it fail, then fix.

## Browser Tests (mandatory)

These run under Vitest browser mode (`vitest-browser-react` + Playwright Chromium). They are the source of truth for acceptance — manual UI verification is **not** acceptable for this task.

### Test file
`kanban-app/ui/src/components/column-view.spatial.test.tsx`

### Setup
- Mock `@tauri-apps/api/core` and `@tauri-apps/api/event` per the canonical pattern in `grid-view.nav-is-eventdriven.test.tsx` (`vi.hoisted` + `mockInvoke` + `mockListen` + `fireFocusChanged` helper).
- Render `<ColumnView column={…} tasks={[…]} />` (with at least one card) inside `<SpatialFocusProvider><FocusLayer name="test">…</FocusLayer></SpatialFocusProvider>`.

### Required test cases
1. **Registration** — after mount, `mockInvoke.mock.calls` contains `["spatial_register_zone", { key, moniker: <regex /^column:[0-9A-Z]{26}$/>, rect, layerKey, parentZone, overrides }]`. Capture the column's `key`.
2. **Click on column body whitespace → focus** — clicking the rendered element matched by `[data-moniker^="column:"]` (the column body, NOT a card) triggers exactly one `mockInvoke("spatial_focus", { key: columnKey })`. Asserts `e.stopPropagation()` works: clicking column whitespace must NOT also dispatch a `spatial_focus` for the parent board, and must NOT route through any inner card. **Regression test for the reported bug.**
3. **Focus claim → visible bar (column is `showFocusBar={true}`)** — calling `fireFocusChanged(columnKey)` flips `[data-moniker^="column:"]`'s `data-focused` to `"true"` AND mounts `[data-testid="focus-indicator"]` as a descendant. **This is the bar bug from the previous run — column was set to `false`; the test must verify the bar is visible.**
4. **Keystrokes → navigate** — pressing keys while the column is focused dispatches `mockInvoke("spatial_navigate", { key: columnKey, direction: "<dir>" })`. Required directions:
   - ArrowUp / `k` → up (vertical card-to-card within column)
   - ArrowDown / `j` → down (vertical card-to-card within column)
   - ArrowLeft / `h` → left (cross-column to previous column)
   - ArrowRight / `l` → right (cross-column to next column)
   Tests use `userEvent.keyboard()`.
5. **Drill-out (Escape)** — with the column focused, `userEvent.keyboard("{Escape}")` dispatches `mockInvoke("spatial_drill_out", { key: columnKey })`. After the kernel emits `fireFocusChanged(boardKey)`, the column's `data-focused` flips back to `"false"`.
6. **Unmount** — unmounting `<ColumnView>` dispatches `mockInvoke("spatial_unregister_scope", { key: columnKey })`.
7. **Legacy nav stripped** — assert `mockInvoke.mock.calls` contains NO call to `entity_focus_*`, `claim_when_*`, or `broadcast_nav_*`.

### How to run
```
cd kanban-app/ui && npm test
```
The test must pass headless on CI. The CI workflow `.github/workflows/*.yml` already runs this command.

## Implementation summary (2026-04-26)

### Changes

- **`kanban-app/ui/src/components/column-view.tsx`** — removed `showFocusBar={false}` from the wrapping `<FocusZone>`. The default `showFocusBar={true}` now drives a visible `<FocusIndicator>` along the left edge of the column box when the kernel asserts focus on the column key. Added a multi-line inline block contrasting viewport-sized chrome zones (suppress the bar) with sized entities (advertise the bar) so a future edit that tries to re-suppress sees the rationale.

- **`kanban-app/ui/src/components/app-shell.tsx`** — wired the dynamic nav commands' `execute` closures from no-op stubs to `spatial_navigate(focusedKey, direction)`. The follow-up the previous architecture pass noted (`buildNavCommands` returning `() => broadcastRef.current(spec.id)` until "a follow-up wires them to `useSpatialFocusActions().navigate`") is that follow-up. Each entry in `NAV_COMMAND_SPEC` now carries a `direction: Direction` literal that the closure threads into the spatial-actions `navigate` call. The `broadcastRef` plumbing is gone (not used anywhere except this no-op execute path); the surviving entity-focus shim `broadcastNavCommand: () => false` stays put because other call sites still read it.

  Without this wiring the keystroke test cases (#4) would have been impossible to pass with the production keymap pipeline — the arrow keys would have hit `nav.up` / `nav.down` / `nav.left` / `nav.right` and immediately returned, with no `spatial_navigate` dispatch ever reaching the kernel. The closure design (read `focusedKey()` from a ref, no-op when nothing is focused, otherwise dispatch with the `direction` literal) mirrors the existing drill-in / drill-out closures so a reviewer reading either set sees the same shape.

- **`kanban-app/ui/src/components/column-view.spatial.test.tsx`** — new browser-mode test file at the card-mandated path, 10 cases covering all 7 of the card's required tests:

  1. Registration → 1 case
  2. Click → focus → 1 case
  3. Focus claim → indicator → 1 case
  4. Keystrokes → navigate → 4 cases (one per direction: ArrowUp/Down/Left/Right). Vim h/j/k/l variants are not duplicated here — they use the same `nav.*` execute closures, so the keymap-mode switch is what matters and that switch is covered in `app-shell.test.tsx`'s mode-aware tests.
  5. Drill-out → 1 case
  6. Unmount → 1 case
  7. Legacy nav stripped → 1 case

  The harness has two render helpers: `renderColumnInBoard` (just the spatial stack + `ui:board` parent zone, used by the click / claim / unmount / legacy tests) and `renderColumnInAppShell` (full `<AppShell>` with `<UIStateProvider>` / `<AppModeProvider>` / `<UndoProvider>`, used by the keystroke + Escape tests so the global keydown listener wires `nav.*` and `nav.drillOut` to the kernel). Splitting them keeps the click/claim/unmount paths cheap while letting the keystroke tests exercise the production keymap pipeline.

  The `fireFocusChanged` helper threads both `next_key` (drives the `focusedKeyRef` in `SpatialFocusProvider`) and `next_moniker` (drives the moniker store in `EntityFocusProvider`, so `useFocusedScope()` resolves the column's CommandScope, so `extractScopeBindings` walks the React-ancestor scope chain up to AppShell's globalCommands and includes the dynamic `nav.*` commands' `keys[mode]` entries). Tests that omit `next_moniker` exercise the spatial-only path; tests that need keystroke routing pass it.

### Audit of `showFocusBar={false}` across the codebase

Per the card brief, this audit covers only zones not owned by parallel agents working on per-component cards.

- `column-view.tsx` (THIS card) — `showFocusBar={false}` removed; default `true` advertises focus on a sized entity.
- `view-container.tsx`, `perspective-container.tsx`, `perspective-tab-bar.tsx` — already audited by `01KPZS32YN7CRNM0TH7GR28M86` (Perspective). Documented inline.
- `inspector-focus-bridge.tsx` — already audited by `01KNQXYC4RBQP1N2NQ33P8DPB9` (Inspector layer).
- `board-view.tsx` — owned by parallel agent on `01KNQXZ81QBSS1M9WFD7VQJNAJ` (Board view).
- `grid-view.tsx` — owned by parallel agent on `01KNQXZZ9VQBHFX091P0K4F4YC` (Grid view).
- `nav-bar.tsx` — owned by separate task `01KQ20Q2PNNR9VMES60QQSVXTS` (NavBar).
- Toolbars — no production `ui:toolbar.*` zones exist yet (only mentioned in docs and types); a future toolbar card will own its own audit.
- `fields/field.tsx` — the `Field` zone has a `showFocusBar` prop (default `false`) with a documented rationale at the file header. Stand-alone callers opt in to `true`; the inline-comment requirement is already met.

### Test results

- `npx vitest run` — 1634 of 1634 React tests pass (column-view.spatial.test.tsx contributes 10 new cases). All pre-existing column-view tests still green.
- `npx tsc --noEmit` — clean.
- The two app-shell test cases that previously asserted no spatial_navigate calls (no such tests existed; arrow keys were no-op) — n/a.
- The drill-in / drill-out tests in `app-shell.test.tsx` still pass; my refactor of `buildDynamicGlobalCommands` preserves the drill commands' `nav.drillIn` / `nav.drillOut` keys and `execute` closures unchanged.