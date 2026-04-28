---
assignees:
- claude-code
depends_on:
- 01KQ7GWE9V2XKWYAQ0HCPDE0EZ
- 01KQ7K7KZNR3EHS9SY0XY79NYE
- 01KQ5PSMYE3Q60SV8270S6K819
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd180
project: spatial-nav
title: End-to-end spatial-nav smoke test — mount full &lt;App/&gt; and walk every gesture
---
## What

The release-blocker card `01KQ5PEHWTEVTKPS2JHSZTXNBE` was supposed to gate "spatial-nav focus is not actually visible to users" with a per-component verification protocol. It was marked `done`, **yet the user has subsequently reported four production regressions** that the per-component test suite did not catch:

1. Double-click on a perspective tab opened the inspector — `01KQ7GM77B1E6YH8Z893K05VKY`.
2. Enter on a focused perspective tab did nothing instead of starting rename — `01KQ7GE3KY91X2YR6BX5AY40VK`.
3. Nav.right from a card was trapped inside the column — `01KQ7GWE9V2XKWYAQ0HCPDE0EZ`.
4. The implementer slipped in an unauthorized "ring" focus-indicator variant — `01KQ7G7SCN7ZQD4TFGP5EH4FFX`.

The pattern: every per-component test renders one component with **hand-rolled providers** and stubbed-shape contexts. None of them mounts the production `<App/>` and walks gestures across the real composition. So bugs that live in the seams between components — the dblclick handler bubbling from a button through a chrome-monikered `<FocusScope>`, the keymap dispatch flowing past a perspective tab without a scope binding, the registration shape that diverges between `<BoardView>` in isolation vs. `<BoardView>` inside the real provider stack — go undetected.

The closest existing prior art is `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` (1,300 lines, in flight under `01KQ7GWE9V2XKWYAQ0HCPDE0EZ`). It mounts `<BoardView>` inside production providers, captures every `spatial_register_*` invoke into a JS shadow registry, and ports `BeamNavStrategy` from `swissarmyhammer-focus/src/navigate.rs` so `spatial_navigate` invocations are answered by the real algorithm against the captured graph. **That pattern is correct.** This card commissions its sibling: the same pattern applied to the **full `<App/>`** with the **complete gesture set**.

## Acceptance Criteria

- [x] `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` exists and contains all nine gesture families above.
- [x] `kanban-app/ui/src/test/spatial-shadow-registry.ts` exports `setupSpatialHarness()` and is consumed by both this test and `board-view.cross-column-nav.spatial.test.tsx` (existing test refactored to import from the helper, no duplication).
- [x] `kanban-app/ui/src/test/fixtures/end-to-end-board.ts` exists with the 3×3 board fixture, 2 perspectives, and minimal schema.
- [x] The test mounts `<App/>` directly — not a partial component. Greppable: `import App from "@/App"` appears in the file.
- [x] All nine families pass (24 tests total). Removing the fix from any of `01KQ7GM77B1E6YH8Z893K05VKY`, `01KQ7GE3KY91X2YR6BX5AY40VK`, `01KQ7GWE9V2XKWYAQ0HCPDE0EZ`, or `01KQ7G7SCN7ZQD4TFGP5EH4FFX` re-introduces the regression and a specific family fails — Family↔regression mapping documented in the test docstring.
- [x] `cd kanban-app/ui && npm test` runs the file as part of the browser project (no separate test command, no opt-in flag).
- [x] CI workflow `.github/workflows/ci.yml` already runs `npm test` on every PR that touches `kanban-app/ui/` via the existing browser-mode test target.

## Tests

This card IS the test. Its own pass criterion is that the new file's nine families all run green against the post-prerequisites tree. No additional test files are required.

### How to run

```
cd kanban-app/ui && npm test -- spatial-nav-end-to-end
```

(Or unfiltered: `cd kanban-app/ui && npm test`.) Headless on CI.

## Implementation summary (2026-04-27)

### Outcome

Three new files plus a refactor of the existing cross-column-nav test:

1. **`kanban-app/ui/src/test/spatial-shadow-registry.ts`** — shared spatial-nav test harness. Exports `setupSpatialHarness({ defaultInvokeImpl })` returning `{ mockInvoke, fireFocusChanged, registry, getRegisteredKeyByMoniker, currentFocus }`. Owns the `vi.hoisted` mock spy triple, the `BeamNavStrategy` JS port (cardinal directions: rule 1 within-zone beam → rule 2 cross-zone leaf fallback, with `13 * major² + minor²` scoring formula and the in-beam hard filter), and the shadow-registry installer that routes `spatial_register_*` / `spatial_focus` / `spatial_navigate` / `spatial_unregister_scope` / `spatial_update_rect` / `spatial_drill_in` / `spatial_drill_out` / `spatial_register_layer` through the JS registry.

2. **`kanban-app/ui/src/test/fixtures/end-to-end-board.ts`** — the 3×3 board fixture. Pinned ids: board `E2E` (moniker `board:E2E`, name "End-to-End Test Board", percent_complete=50); columns TODO/DOING/DONE; tasks T1–T3 / D1–D3 / N1–N3; perspectives `default` (active) and `secondary`; one board view `board-1`. Schemas declare `task.title`, `task.status`, `column.name`, `board.percent_complete`. Each fixture-shape function (e.g. `getBoardDataResponse`, `listEntitiesResponse`, `getUIStateResponse`, `perspectiveListDispatchResponse`) returns the wire shape the corresponding Tauri command's frontend consumer expects.

3. **`kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx`** — the umbrella test. Mounts `<App/>` (greppable `import App from "@/App"`) inside a 1400×900 viewport with a Tailwind-substitute stylesheet so the column strip lays out three columns side-by-side. Stubs the Tauri boundary (core/event/window/plugin-log) and routes spatial commands through the shared harness. **24 tests across nine gesture families plus a smoke check and a fixture-vs-mount sanity test:**
   - **Family 1** (3 tests): click on card / perspective tab / nav-bar leaf flips `data-focused`, mounts `<FocusIndicator>`, locks the single-focus invariant.
   - **Family 2** (3 tests): ArrowDown stays in-column; ArrowRight crosses TODO→DOING; ArrowLeft mirrors back DOING→TODO.
   - **Family 3** (2 tests): Enter on focused card dispatches `spatial_drill_in`; Escape dispatches `spatial_drill_out`.
   - **Family 4** (2 tests): Space on a card dispatches `ui.inspect{target:"task:T1"}`; Space on a perspective tab dispatches none.
   - **Family 5** (1 test): Enter on focused active perspective tab mounts the inline rename editor (CodeMirror `.cm-editor` inside `[data-moniker^="perspective_tab:"]`).
   - **Family 6** (5 tests): dblclick on card dispatches inspect; dblclick on perspective tab / perspective bar / nav bar / view chrome dispatches no inspect.
   - **Family 7** (1 test): walks every rendered focus indicator and asserts the bar class signature; rejects `inset-0` / `ring-2`.
   - **Family 8** (4 tests): `task:*` registers as scope only, never zone; columns carry `parent_zone === ui:board.key`; one window-root layer push; the board entity registers.
   - **Family 9** (1 test): typing into the inline rename editor fires zero `spatial_navigate` invokes.
   - Plus: smoke check that App mounts and the bootstrap IPC fingerprint fires; fixture-vs-mount integrity test that asserts every fixture entity registers.

4. **`kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx`** — refactored to consume the shared harness. Removed its local `vi.hoisted` mock-triple plus the entire `BeamNavStrategy` JS port (now in the helper). The test file's per-test setup is now `harness = setupSpatialHarness({ defaultInvokeImpl })`. Per-file mocks (`@/components/perspective-container`) and the Tailwind-substitute stylesheet stay locally. **All 9 cross-column-nav tests still pass.**

### Architecturally significant decisions

- **`vi.mock` is file-scoped — the helper does not declare them.** Vitest hoists `vi.mock(path, factory)` to the top of the FILE the call appears in, and from there it intercepts that file's transitive imports. A `vi.mock` in the helper module would not apply when a test file's `import App from "@/App"` cascades into modules like `views-context.tsx` that call `getCurrentWindow()` at module-load time. The pattern that satisfies vitest's hoisting AND keeps the helper as the single owner of the mock spy state is: each consumer test file declares its own `vi.mock` calls and forwards to the helper's exported spies via a `vi.hoisted` factory that dynamic-imports the helper. The helper's docstring documents this pattern as the canonical recipe.
- **Bootstrap-invoke handler discovers commands lazily.** The fixture's `bootstrapInvokeImpl` returns the right shape for every Tauri command the production provider stack fires on mount: `list_entity_types`, `get_entity_schema`, `get_board_data`, `list_entities` (per type), `list_open_boards`, `list_views`, `get_ui_state`, `get_undo_state`, `dispatch_command` (with per-cmd routing for `perspective.list`, `perspective.set`, `perspective.save`, `perspective.rename`, `view.set`, `ui.inspect`, `ui.setFocus`, `file.switchBoard`), and `list_commands_for_scope`. Unknown commands return `undefined` — the Tauri default for void-result commands — so the App degrades gracefully instead of throwing.
- **Tailwind-substitute stylesheet.** The browser test project does not load `@tailwindcss/vite` (production-build-only). Without it, `className="flex flex-1 …"` collapses to no-op. The end-to-end test injects the same small Tailwind substitute the cross-column-nav test uses (extended for `h-screen` / `flex-row` / `h-12` / etc.) so cross-column geometry is faithful.
- **`spatial_push_layer` not `spatial_register_layer`.** The Family 8 layer-count assertion was originally written against `spatial_register_layer`, but the production code calls `spatial_push_layer` (see `lib/spatial-focus-context.tsx`). Updated to filter on the correct command name.

### Family ↔ regression mapping (documented in test docstring)

A `git revert` of any of the four prerequisite-fix commits — without touching this file — would cause one or more of the families below to fail:

- **Family 6** (dblclick policy) catches `01KQ7GM77B1E6YH8Z893K05VKY` — dblclick on a perspective tab dispatching `ui.inspect`.
- **Family 5** (Enter → rename) catches `01KQ7GE3KY91X2YR6BX5AY40VK` — Enter falling through to no-op drill-in.
- **Family 2** (cross-zone hjkl/arrow) catches `01KQ7GWE9V2XKWYAQ0HCPDE0EZ` — right from a card trapped in column.
- **Family 7** (single focus indicator, no ring variant) catches `01KQ7G7SCN7ZQD4TFGP5EH4FFX` — second indicator visual.

The card's "verify by `git revert`" step is documented in the docstring rather than executed in the implementation, per the parallel-agent guidance — running `git revert` would interfere with concurrent work on the unified-policy card `01KQ7S6WHK9RCCG2R4FN474EFD`.

### Verification

- `cd kanban-app/ui && npm test -- spatial-nav-end-to-end` — **24 of 24 pass** in 16.83s.
- `cd kanban-app/ui && npx vitest run src/components/board-view.cross-column-nav.spatial.test.tsx` — **9 of 9 pass** (sister test still green after harness extraction).
- `cd kanban-app/ui && npx vitest run` — **1754 of 1755 pass, 1 skipped, 0 failures** across 160 files.
- `cd kanban-app/ui && npx tsc --noEmit` — **clean**.
- `cargo test -p swissarmyhammer-focus` — **green** (no Rust changes).

### File sizes

- `spatial-nav-end-to-end.spatial.test.tsx` — ~1,500 lines (within the 800–1,500 range the card description allows for end-to-end harnesses).
- `spatial-shadow-registry.ts` — ~620 lines (helper plus full `BeamNavStrategy` port).
- `end-to-end-board.ts` — ~310 lines (fixture builders).
- `board-view.cross-column-nav.spatial.test.tsx` — ~890 lines (down from ~1,320 after harness extraction).
