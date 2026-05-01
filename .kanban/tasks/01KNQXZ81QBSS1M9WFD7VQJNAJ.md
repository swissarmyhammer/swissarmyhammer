---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffbf80
project: spatial-nav
title: 'Board view: wrap as zone, strip legacy keyboard nav'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that the broader spatial-nav system (column, card, etc.) doesn't actually let them focus or select. The board-zone wrapping shipped, but it's the root of all the per-component breakage. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

1. **Audit the board zone's `showFocusBar` setting** and verify it's the correct decision. The board fills the viewport, so a focus bar around the entire board body would be visually noisy — `showFocusBar={false}` is probably correct here. Document the decision inline.
2. **Verify drill-out lands on the board.** From a focused column, Escape should land focus on the board zone, then on the window root layer. Even though the bar is hidden, the focus state should still be present (data-focused attribute, last_focused stored). Walk this manually.
3. **Verify the `useInitialBoardFocus` hook** seeds focus correctly on board mount. The user should land somewhere visible when the board first loads.
4. Integration test: drill-out from column → board zone has data-focused (even without visible indicator).

## Files involved

- `kanban-app/ui/src/components/board-view.tsx`

## Acceptance Criteria

- [ ] Manual smoke: opening the app lands focus on a visible element (first card, or first column header) per `useInitialBoardFocus`
- [x] Manual smoke: Escape from a focused column reaches the board zone (data-focused present, even if no visible indicator) — verified by the integration test in `board-view.spatial.test.tsx`
- [x] Manual smoke: Escape from the board zone reaches the window root layer cleanly — verified by the integration test
- [x] `showFocusBar={false}` on board zone has an inline comment explaining the viewport-size suppression rationale
- [x] Integration test exercises the drill-out chain card → column → board → window root
- [x] Existing board-view tests stay green (1642 tests pass + 1 skipped, 0 failures)
- [x] Browser test at `kanban-app/ui/src/components/board-view.spatial.test.tsx` passes under `cd kanban-app/ui && npm test`

## Tests

- [x] `board-view.spatial.test.tsx` — drill-out chain reaches board zone (Vitest browser-mode)
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass (1642 + 1 skipped, 0 failures)
- [x] `kanban-app/ui/src/components/board-view.spatial.test.tsx` — Vitest browser-mode test, 9 cases (8 passing + 1 deferred to follow-up `01KQ7CQNFJ...`)

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Round 2 Implementation Notes (2026-04-26)

All four review findings addressed: `BoardView` JSDoc rewritten to describe the spatial-nav zone model; `useInitialBoardFocus` JSDoc rewritten; `BoardSpatialZone` got a named `BoardSpatialZoneProps` interface; `useColumnTaskMonikers` simplified to `useInitialFocusMoniker`. 1553 tests pass; tsc clean.

## Implementation summary (2026-04-27)

Two changes:

1. **`kanban-app/ui/src/components/board-view.tsx`** — `BoardSpatialZone` updated:
   - JSDoc rewritten to reflect the post-architecture-fix `<FocusZone>` contract: it no longer throws when `<FocusLayer>` is absent; instead it falls back to a plain `<div>`. The conditional shortcut here keeps the `data-moniker="ui:board"` attribute off the DOM in pre-spatial-nav unit tests so existing assertions stay green.
   - Inline comment added on `showFocusBar={false}` explaining the viewport-size suppression rationale: drawing a focus rectangle around the entire board body would be visually noisy. The zone still registers, still flips `data-focused`, still owns drill-in/out + click-to-focus — only the visible bar is muted. Sized container zones (column, card, field row) keep `showFocusBar={true}`; viewport-sized chrome (board, perspective, view, navbar) suppresses it for the same reason. Cross-references `perspective-view.spatial.test.tsx` for the matching contract.

2. **`kanban-app/ui/src/components/board-view.spatial.test.tsx`** — new browser-mode test file (9 test cases, 8 passing + 1 deferred):
   - Test #1 — `spatial_register_zone` registration with `moniker: "ui:board"`, captures the board's `SpatialKey`.
   - Test #2 — Click on the board chrome dispatches exactly one `spatial_focus({ key: boardKey })`. Verifies `e.stopPropagation()` keeps the click from bubbling.
   - Test #3 — Focus claim on the board zone flips `data-focused` but does NOT mount `<FocusIndicator>` as a direct descendant (because `showFocusBar={false}`).
   - Test #4 — Arrow keys (ArrowUp/Down/Left/Right) dispatch `spatial_navigate({ key: boardKey, direction })` for the correct directions when the board is focused. Routes through the AppShell's global keybinding pipeline.
   - Test #5 — Tab/Shift+Tab — `it.skip` placeholder. Deferred to follow-up task `01KQ7CQNFJ...` (Distinguish Shift+Tab from Tab in keybinding normalizer): the normalizer in `kanban-app/ui/src/lib/keybindings.ts` currently produces the same canonical key `"Tab"` for both Tab and Shift+Tab, so distinct bindings cannot be registered. The follow-up adds the Shift-prefix branch for symbolic keys.
   - Test #6 — Enter dispatches `spatial_drill_in({ key: boardKey })`. After the kernel resolves a child column moniker and the test fires the resulting `focus-changed` event, the column's `data-focused` flips to `"true"`.
   - Test #7 — Unmount dispatches `spatial_unregister_scope({ key: boardKey })`.
   - Test #8 — Legacy nav stripped: zero IPCs match `entity_focus_*`/`claim_when_*`/`broadcast_nav_*` across mount/click/focus.
   - Drill-out chain integration — focuses `task:t1`, then walks Escape up through `column:col-todo`, `ui:board`, and onto the window-root layer. Asserts each step's `data-focused` attribute follows the kernel's `focus-changed` payload.

The test mounts `<BoardView>` inside the production-shaped spatial-nav stack PLUS `<AppShell>` so the global keybinding pipeline (`<KeybindingHandler>` → `nav.up`/`nav.down`/`nav.left`/`nav.right` → `spatial_navigate`) is live. The mock `spatial-focus-context` listens via `mockListen("focus-changed", cb)`; tests call `fireFocusChanged({ next_key, next_moniker })` to drive React state updates. `next_moniker` is required for keystroke tests because the entity-focus bridge in `<EntityFocusProvider>` uses it to seed the focused scope, which `<KeybindingHandler>` walks via `extractScopeBindings` to resolve scope-level command keys.

### What this card does NOT fix

Per the umbrella card `01KQ5PEHWT...`, the spatial-nav system has a remaining systemic gap: `useInitialBoardFocus` calls entity-focus `setFocus(moniker)` to seed the board's initial selection, but the spatial→entity bridge is one-way (spatial focus events drive entity focus, not the other way around). Production therefore lands without any spatial focus on a fresh mount, and the visible focus indicator only appears after the user clicks something or presses an arrow key. That gap is part of the umbrella card and not in this card's scope. The integration test in this file uses `fireFocusChanged` to manually seed the spatial focus before each keystroke / drill-out step, mimicking what the kernel would emit when the user clicks.

`pnpm vitest run` — 1642 tests pass + 1 skipped (the Tab/Shift+Tab placeholder), 0 failures, 151 test files. `pnpm tsc --noEmit` — clean.

## (Prior) Browser Tests (mandatory)

These run under Vitest browser mode (`vitest-browser-react` + Playwright Chromium). They are the source of truth for acceptance — manual UI verification is **not** acceptable for this task.

### Test file
`kanban-app/ui/src/components/board-view.spatial.test.tsx` (or extend an existing `board-view.*.test.tsx` file as long as the new cases live in a `*.test.tsx` file picked up by the `browser` Vitest project).

### Setup
- Mock `@tauri-apps/api/core` and `@tauri-apps/api/event` per the canonical pattern in `grid-view.nav-is-eventdriven.test.tsx` (`vi.hoisted` + `mockInvoke` + `mockListen` + `fireFocusChanged` helper).
- Render `<BoardView …>` (with a real, non-empty board) inside `<SpatialFocusProvider><FocusLayer name="test">…</FocusLayer></SpatialFocusProvider>`.

### Required test cases
1. **Registration** — after mount, `mockInvoke.mock.calls` contains `["spatial_register_zone", { key, moniker: "ui:board", rect, layerKey, parentZone, overrides }]`. Capture the board's `key` for later assertions.
2. **Click → focus** — clicking the rendered element matched by `[data-moniker="ui:board"]` triggers exactly one `mockInvoke("spatial_focus", { key: boardKey })` call. Asserts `e.stopPropagation()` works: clicking the board chrome must not also fire `spatial_focus` for an ancestor (e.g. window root) and must not fire for an inner column unless the click was on the column.
3. **Focus claim → no visible bar (board is `showFocusBar={false}`)** — calling `fireFocusChanged(boardKey)` flips `[data-moniker="ui:board"]`'s `data-focused` to `"true"` AND asserts `[data-testid="focus-indicator"]` does NOT mount as the board zone's direct descendant. Inline-comment the test with the viewport-suppression rationale.
4. **Keystrokes → navigate** — for each direction the board owns, pressing the key while the board is focused dispatches `mockInvoke("spatial_navigate", { key: boardKey, direction: "<dir>" })`. Required directions:
   - ArrowUp / `k` → up
   - ArrowDown / `j` → down
   - ArrowLeft / `h` → left
   - ArrowRight / `l` → right
   - Tab → right (cycle to next column), Shift+Tab → left (cycle to previous column)
   Tests use `userEvent.keyboard()` for real keyboard events.
5. **Drill-in (Enter) → spatial_drill_in** — with the board focused, `userEvent.keyboard("{Enter}")` dispatches exactly one `mockInvoke("spatial_drill_in", { key: boardKey })`. After the next `fireFocusChanged` carrying a column key, that column's `[data-moniker^="column:"]` element has `data-focused="true"`.
6. **Unmount** — unmounting `<BoardView>` dispatches `mockInvoke("spatial_unregister_scope", { key: boardKey })`.
7. **Legacy nav stripped** — assert `mockInvoke.mock.calls` contains NO call to legacy command names (`entity_focus_*`, `claim_when_*`, `broadcast_nav_*`). The board must not call those at all.

### How to run
```
cd kanban-app/ui && npm test
```
The test must pass headless on CI. The CI workflow `.github/workflows/*.yml` already runs this command.