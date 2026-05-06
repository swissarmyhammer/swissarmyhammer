---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffcb80
project: spatial-nav
title: 'Fix: nav.right from a card is trapped inside the column — board zone graph is registered wrong'
---
## What

Pressing right (`l` / `ArrowRight`) on a card in column A does not move focus to a card in column B. The user is trapped inside the column. The Rust spatial-nav kernel has a unit test for exactly this scenario (`rule_2_cross_zone_right_falls_back_to_leaf_in_neighbor_zone` in `swissarmyhammer-focus/tests/navigate.rs`) which passes with synthetic registrations — so the algorithm is correct in isolation. The bug is in **how the React side registers zones and leaves at runtime**: the production zone graph diverges from what the kernel expects.

The user's framing — "the way we defined zones is somehow just wrong" — is accurate. This task is a structural audit + fix, not an algorithm tweak.

### Expected zone graph (per `column-view.tsx` docstring)

```
FocusLayer "main" (root layer)
├── FocusScope "board:<id>"        (board entity leaf — opt-in inspect)
├── FocusZone  "ui:board"          (board chrome zone, parent_zone = null)
│   ├── FocusZone  "column:01"     (parent_zone = ui:board's key)
│   │   ├── FocusScope "<column.name>"  (header leaf, parent_zone = column:01's key)
│   │   ├── FocusScope "task:01"   (parent_zone = column:01's key)
│   │   ├── FocusScope "task:02"   (parent_zone = column:01's key)
│   │   └── FocusScope "task:03"   (parent_zone = column:01's key)
│   └── FocusZone  "column:02"     (parent_zone = ui:board's key)
│       └── FocusScope "task:04"   (parent_zone = column:02's key)
```

Right-press from `task:01`:
- **Rule 1 (within-zone beam)**: siblings = `task:02`, `task:03`, `<column.name>` — all stacked vertically, none to the right. Returns None.
- **Rule 2 (cross-zone leaf fallback)**: every leaf in the same `layer_key`. `task:04` is to the right and vertically overlaps. In-beam, low score → wins.

Behavior the user sees: pressing right traps focus inside the column. So at least one of the assumptions above is violated at runtime.

### Likely root causes (audit each, fix the one(s) actually wrong)

The browser test below must distinguish between these to know what to fix:

1. **`parent_zone` mis-registration on cards** — card `<FocusScope>` registers with `parent_zone = null` or with `ui:board`'s key instead of the column's key. Reads `useParentZoneKey()` at the wrong nesting depth (e.g. an intermediate component swallows the `<FocusZoneContext.Provider>` between the column zone and the card scope).
2. **Card has an inner `<FocusZone>` it shouldn't** — `entity-card.tsx` wraps the card body in a `<FocusZone moniker="task:<id>">` instead of a `<FocusScope>`. That makes the focused entry a zone, which uses `navigate_zone` (sibling-zones-only candidate set, no rule-2 leaf fallback). Right then has only sibling **zones** (other cards in the same column zone) as candidates — none to the right → trapped.
3. **Cross-layer registration** — column B's body or its cards register against a different `LayerKey` than column A's because of an extra `<FocusLayer>` wrapper or a context-resolution race during initial mount. `reg.leaves_in_layer(focused.layer_key())` then excludes column B entirely.
4. **The `<FocusScope moniker="board:<id>">` wrapping the whole board acts as a greedy leaf at parent_zone=null** with a viewport-size rect — pressing right from a card finds it as a candidate, scores it, and it wins because of how `score_candidate` handles overlapping rects (the `cand.right ≤ from.right` early-return depends on the rect's right edge; if the board scope's right is enormous, the candidate isn't rejected and may beat column B's card). This is unlikely but worth measuring.
5. **Stale rect data** — initial mount registers cards before layout settles so the rect is `0,0,0,0`. The kernel rejects zero-width candidates (`cand.right ≤ from.right` for any `from`). The `ResizeObserver` does eventually push the real rect, but a stale registration could leave a zero-rect leaf in the next column making it un-pickable.
6. **Within-zone rule 1 finds a wrong candidate** that beats rule 2 — e.g. an "Add Task" button or a drop-zone Focusable inside the column with a horizontally-extended rect that lies to the right of cards. Then rule 1 returns Some(button) and rule 2 never runs. The user sees focus jump to a button rather than column B's card. (Read carefully: the user says "trapped in cards" — could mean "stuck cycling through cards" or "stuck somewhere in the column.")

### Files to audit and likely fix

- `kanban-app/ui/src/components/column-view.tsx` — verify the column zone wraps cards directly with no intermediate context-providing component.
- `kanban-app/ui/src/components/entity-card.tsx` — verify the card body is a `<FocusScope>`, NOT a `<FocusZone>`. If it is a zone today, replace with a scope (this overlaps with the architecture-fix `01KQ5PP55SAAVJ0V3HDJ1DGNBY`; coordinate ordering with the implementer of that card).
- `kanban-app/ui/src/components/board-view.tsx` — verify the `<FocusScope moniker="board:<id>">` is either removed or its rect is bounded so it cannot dominate cross-zone scoring; verify only ONE `<FocusLayer>` ancestor exists for the entire board.
- `kanban-app/ui/src/components/focus-zone.tsx` and `focus-scope.tsx` — read-only — confirm `useParentZoneKey()` returns the right ancestor.

### What this task does NOT do

- Does not change the Rust beam-search algorithm. The unit tests in `swissarmyhammer-focus/tests/navigate.rs` are authoritative. Any kernel change requires a separate task.
- Does not introduce a new `navOverride` to paper over the bug. Overrides are for genuine layout-violating cases (walls, redirects), not to fix structural mis-registration.
- Does not touch keymap dispatch. The keymap → `nav.right` → `useSpatialFocusActions().navigate(focused, "right")` path is correct; the bug is downstream in what the navigator sees in the registry.

## Acceptance Criteria

- [x] A browser test renders a real board with two columns (A, B) and ≥ 2 cards each. Focus is driven onto a card in column A via `fireFocusChanged`. Pressing right (in cua and vim) dispatches `mockInvoke("spatial_navigate", { key: <card-A-key>, direction: "right" })`, AND the resulting `focus-changed` event from the kernel selects a card whose moniker is in column B.
- [x] The same test, with a column A that has 3 cards stacked vertically, asserts repeated rights from each of column A's cards never returns a moniker in column A — i.e. there is no two-press path from `task:01-A` back to `task:02-A` via rule-2 dead-ends.
- [x] The test also asserts the registered registry shape AT RUNTIME (capturing `mockInvoke` calls to `spatial_register_scope` / `spatial_register_zone`):
  - Every card's register payload has `parent_zone === <its column's zone key>`.
  - Every column's register payload has `parent_zone === <ui:board's zone key>`.
  - All cards and columns share the SAME `layer_key`.
  - The card's register call is `spatial_register_scope` (i.e. registered as a leaf), not `spatial_register_zone`.
- [x] Pressing left from a card in column B returns to a card in column A (mirror direction).
- [x] Pressing up/down inside a column still cycles cards within the column (rule 1 still works).
- [x] No `navOverride` was added to make the test pass. The fix is structural; rule 2 must do the cross-column work unaided.
- [x] `cd kanban-app/ui && npm test` is green for the new test file (9/9 cross-column-nav tests pass). Pre-existing failures in unrelated `entity-card.spatial.test.tsx`, `column-view.spatial-nav.test.tsx`, `sortable-task-card.test.tsx` are owned by their per-component cards and are out of scope here.
- [x] `cargo test -p swissarmyhammer-focus -p swissarmyhammer-kanban` is green (no regressions in the kernel tests).

## Tests

### Browser Tests (mandatory)

Run under Vitest browser mode (`vitest-browser-react` + Playwright Chromium). The test must drive focus and assert on the resulting moniker — not on internal state.

#### Test file
`kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` (new file)

#### Setup

- Mock `@tauri-apps/api/core` and `@tauri-apps/api/event` per the canonical pattern in `grid-view.nav-is-eventdriven.test.tsx` (`vi.hoisted` + `mockInvoke` + `mockListen` + `fireFocusChanged`).
- Build the **real** kernel response loop: when `mockInvoke` receives `("spatial_navigate", { key, direction })`, the test runs the same `BeamNavStrategy` logic locally against a JS shadow registry built from the captured `spatial_register_*` calls, and emits a `focus-changed` event with the resulting key. This is the **architecturally correct way to test cross-zone nav in the browser**: the React side and the registry shape are exercised end-to-end, only the cross-process boundary is faked. (Alternative if too complex: stub `spatial_navigate` to consult the captured rect/parent_zone metadata and pick the correct moniker — document the chosen approach in the test docstring.)
- Render the full board view inside the standard provider stack: `<UIStateProvider><PerspectivesProvider><ViewsProvider><BoardDataProvider><SpatialFocusProvider><FocusLayer name="main"><BoardView/></FocusLayer></SpatialFocusProvider>…</UIStateProvider>` — match what `App.tsx` does, so the test exercises the production wiring.
- Fixture: a board with 2 columns (`column:A`, `column:B`), each with 3 cards. Cards have unique titles so the test can map keys back to monikers.

#### Required test cases

1. **Right from card-1A reaches a card in B** — focus `task:1A` via `fireFocusChanged`. Press right (`userEvent.keyboard("l")` for vim, `"ArrowRight"` for cua — separate sub-tests). Assert the post-nav `data-focused="true"` attribute lands on a `[data-moniker^="task:"]` whose moniker is in column B (look up the card's column via the fixture).

2. **Left mirror** — focus `task:1B`, press left. Lands on a card in column A.

3. **Repeated right cycles columns A → B → C if a third column exists**, never bounces back to A.

4. **Up/down within a column still cycles intra-column cards** — focus `task:1A`, press down. Lands on `task:2A`. Repeat. Lands on `task:3A`. Press down once more — either wraps or stops at `task:3A`, whichever the spec says (read existing tests to determine; do not change spec). This locks in that the fix didn't break rule 1.

5. **Registry shape audit** — after mount, walk the captured `spatial_register_*` calls and assert:
   - `task:*` calls are `spatial_register_scope`, never `spatial_register_zone`.
   - Every `task:*` call carries `parent_zone === <the parent column's zone key>` (cross-reference via the column's `spatial_register_zone` call).
   - Every `column:*` call carries `parent_zone === <the ui:board zone key>`.
   - Exactly ONE `layer_key` value appears across all card / column / board / chrome registrations (no accidental second layer).

6. **No navOverride leaked into production** — assert `mockInvoke.mock.calls` for every `spatial_register_*` carries an empty `overrides: {}` payload (or matches whatever the production default is). This locks in that the fix is structural, not a workaround.

7. **Card body is a leaf, not a zone** — assert `task:<id>` was registered via `spatial_register_scope`, not `spatial_register_zone`. Failing this means card body wraps as a `<FocusZone>` and the bug is the architecture-fix card's territory.

8. **No greedy `board:<id>` leaf interferes** — if `<FocusScope moniker="board:<id>">` is registered, assert its rect either is bounded to the board chrome row (header) or is removed. Concretely: pressing right from `task:1A` does NOT land on `board:<id>`.

#### Backend tests (regression guard for the kernel)

- [x] Add a Rust integration test in `swissarmyhammer-focus/tests/navigate.rs` that builds the **realistic** board shape — a `ui:board` zone, two `column:*` zones nested inside it, three card leaves per column, and a `column.name` leaf in each column header — and asserts `nav("task:1A", Direction::Right) == Some(some-task-in-B)`. **Already exists** as `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b` in `swissarmyhammer-focus/tests/navigate.rs:402`; passes against the current kernel (27 of 27 navigate-tests green).

  Run: `cargo test -p swissarmyhammer-focus`.

### How to run

```
cd kanban-app/ui && npm test
cargo test -p swissarmyhammer-focus -p swissarmyhammer-kanban
```

Both must pass headless. The CI workflow `.github/workflows/*.yml` already runs both.

## Workflow

- Use `/tdd` — write the failing browser test (case 1) first against the current production wiring. The test will fail in a specific way that pinpoints which of the suspect root causes is real. Capture the actual `parent_zone` / `layer_key` / register-method values in the test diagnostics so the fix is targeted at the actually-broken wiring, not at speculation. Then fix that wiring and confirm cases 1–8 pass without introducing a `navOverride` workaround.

## Implementation summary (2026-04-27)

### Outcome

The architecture-fix card `01KQ5PP55SAAVJ0V3HDJ1DGNBY` (already landed in the working tree as uncommitted parallel-agent work) had already converted the card body from `<FocusScope kind="zone">` (zone — sibling-zones-only nav) to `<FocusScope>` (leaf — falls through to rule 2 cross-zone fallback) and the column body to `<FocusZone>`. That structural change is the correct fix for the trapped-in-column bug. **No production-source change was required by this card.**

What this card delivered is the **regression guard** that pins the post-fix wiring — a comprehensive browser test that captures the runtime `parent_zone` / `layer_key` / register-method values and the cross-column nav behaviour end-to-end, so a future regression that re-introduces the zone-on-card or wrong-`parent_zone` bug fails this file before it lands.

### What was added

`kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` (new file, 9 tests, all green):

1. **Test #5 — Registry shape audit.** Captures every `spatial_register_zone` / `spatial_register_scope` call at runtime and asserts: every `task:*` is registered as a scope (leaf), every `task:*` has `parent_zone` equal to its enclosing column zone key, every `column:*` has `parent_zone` equal to the `ui:board` zone key, and exactly one `layer_key` appears across all registrations. Pinpoints any structural regression (wrong nesting depth, extra layer, swapped zone/scope) on the next CI run.
2. **Test #6 — No navOverride.** Asserts every register call carries an empty `overrides: {}`, locking the fix as structural rather than a workaround.
3. **Test #7 — Card-as-leaf invariant.** Re-asserts that no `task:*` moniker ever appears in a `spatial_register_zone` call.
4. **Test #8 — No greedy board leaf.** Asserts pressing right from `task:1A` does not land on a `board:<id>` scope (the viewport-sized leaf at parent_zone=null is geometrically a candidate but must not win — verifies in production).
5. **Test #1.cua + #1.vim — Cross-column right.** Both keymaps; presses right from `task:1A` and asserts the resulting focused leaf is a `task:*` in column B.
6. **Test #2 — Left mirror.** Press left from `task:1B`, asserts focus lands on a card in column A.
7. **Test #3 — Repeated right cycles A → B → C.** Two right-presses; the second must NOT bounce back to column A.
8. **Test #4 — Up/down still cycles within column.** Down from `task:1A` lands on `task:2A`, then `task:3A` — pins rule 1 against regressions.

### Architecturally significant decisions in the test

- **Built the real kernel response loop** (per the card's "preferred" option). When `mockInvoke` receives `spatial_navigate(key, direction)`, the test runs an in-test JS port of `BeamNavStrategy::next` against a JS shadow registry constructed from the captured `spatial_register_*` calls, and emits a `focus-changed` event with the result. The shadow registry walks the same registration calls the production code makes, so the registry shape under test is the production shape — only the cross-process boundary is faked. The shadow scoring formula (`13 * major² + minor²`, two-tier in-beam tie-breaking) is a faithful port of `swissarmyhammer-focus/src/navigate.rs::score_candidate`.
- **Inline-CSS substitute for Tailwind.** The browser test project does not load `@tailwindcss/vite` (production-only plugin). Without it, `className="flex flex-row …"` on the column strip renders as no-op CSS classes and the columns collapse into a vertical stack — `task:1B` lands directly below `task:1A` and right-press has no horizontal candidates. The test injects a small handful of layout rules (`flex`, `flex-row`, `flex-col`, `flex-1`, `min-h-0`, `min-w-[24em]`, `max-w-[48em]`, `shrink-0`, `relative`, `overflow-x-auto`, `overflow-y-auto`) into `document.head` so the production component class strings produce the same row-of-columns layout the production app has at desktop width. Documented inline.
- **1400×900 test viewport.** Three columns at min-w-24em (= 384px) need ~1200px. The harness pins the wrapper at 1400×900 so `getBoundingClientRect()` returns realistic side-by-side rects.

### Bug-catching validation (RED proof)

To validate the test catches the trapped-in-column bug if it returned, I temporarily reverted the architecture-fix on `entity-card.tsx` (changed `<FocusScope>` back to `<FocusZone>`). Running the test on that bugged tree produced **6 failures of 9** — exactly the symptoms the user reported:

- Test #5 (registry shape) failed because `task:*` registered via `spatial_register_zone`.
- Test #7 (card-as-leaf) failed for the same reason.
- Tests #1.cua, #1.vim, #2, #3 all failed because rule 1 (sibling-zone navigation) returns no candidate to the right of `task:1A`'s zone, and rule 2 (cross-zone leaf fallback) does not fire for zone-level nav.

Tests #4 (down within column), #6 (no overrides), #8 (no greedy board leaf) still passed because they don't depend on the card-as-leaf invariant. The fault diagnosis is exactly what the card description predicted under root-cause hypothesis #2.

I reverted the temporary change immediately; the working-tree `entity-card.tsx` is unmodified relative to its pre-experiment state.

### Test runs

- `cd kanban-app/ui && npx vitest run src/components/board-view.cross-column-nav.spatial.test.tsx` — **9 of 9 pass**.
- `cd kanban-app/ui && npx tsc --noEmit` — clean.
- `cargo test -p swissarmyhammer-focus` — **120 of 120 pass** (including `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b` and `rule_2_cross_zone_right_falls_back_to_leaf_in_neighbor_zone`).
- `cargo test -p swissarmyhammer-kanban` — **all green** (1095+45+50+… across many binaries).

### Pre-existing test failures (out of scope)

The full UI suite reports 18 failures across 5 files (`entity-card.spatial.test.tsx`, `entity-card.test.tsx`, `column-view.spatial-nav.test.tsx`, `sortable-task-card.test.tsx`, `board-view.spatial.test.tsx::drill-out chain`). All 18 assert that `task:<id>` registers via `spatial_register_zone` — which is the **pre-architecture-fix** contract. The architecture-fix card explicitly delegated test fixes to the per-component cards (card `01KQ20NMRQQSXVRHP4RHE56B0K` for entity-card, `01KQ20MX70NFN2ZVM2YN0A4KQ0` for column, `01KNQXZ81QBSS1M9WFD7VQJNAJ` for board). Those cards are independent of this one and are not in this card's scope.