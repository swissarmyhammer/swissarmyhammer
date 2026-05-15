---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffbd80
project: spatial-nav
title: Drill-in/drill-out commands + Enter→drillIn, Space→inspect (newtype signatures)
---
## STATUS: REOPENED 2026-04-26 — Space does not trigger inspect

The user reports that **pressing Space on a focused element does not open the inspector**. The keybinding wiring is correct (board-view.tsx:602 binds `board.inspect` to `keys.cua: "Space"`, keybindings.ts canonicalises ` ` → `"Space"`, app-shell.test.tsx:730 verifies the dispatch path). The bug is at the data layer:

```typescript
function makeInspectCommand(deps: BoardActionDeps): CommandDef {
  return {
    id: "board.inspect",
    keys: { vim: "Enter", cua: "Space" },
    execute: () => {
      const fm = deps.focusedMonikerRef.current;     // ← entity-focus moniker
      if (fm) deps.dispatchInspect({ target: fm }).catch(console.error);
    },
  };
}
```

`focusedMonikerRef.current` reads the **legacy entity-focus** moniker. After the spatial-nav refactor, clicks on most leaves call `spatial_focus(SpatialKey)` and update the **spatial-focus** key — but they do NOT bridge back to the entity-focus moniker store. So `focusedMonikerRef.current` is `null` (or stale from before the click), the early return fires, and Space looks like a no-op.

The grid-cell case has a partial fix (closeout note on `01KNQXZZ9V` added an inner `<div onClick>` inside `<Focusable>` to set both spatial-focus AND entity-focus on click) — but that bridge isn't applied to other leaves (column header, card title, status pill, navbar button, perspective tab, inspector field).

This card now owns: drilling out the bridge (or replacing the data source the inspect command reads from) so Space inspects the spatially-focused element regardless of which leaf was clicked.

## Why this is downstream of the architecture fix

Card `01KQ5PP55S` (collapse `<Focusable>` into `<FocusScope>`) restructures the leaf primitive. After it lands, every leaf has a single click handler in a single component, so the spatial→entity bridge can be added in exactly one place rather than smeared across two stacked components. Don't fix this card before that one — the surface area is still moving.

## The fix — pick one of these designs

**Option A: bridge spatial-focus to entity-focus.** When `focus-changed` fires, the React-side claim handler already knows the new `SpatialKey`. Resolve it to a `Moniker` (the registry has the mapping) and call `setFocus(moniker)` so the legacy entity-focus store stays in sync. This keeps `focusedMonikerRef` as the single read source for commands like `board.inspect`.

**Option B: read directly from spatial-focus.** Change `makeInspectCommand` (and any peer commands) to read the focused moniker from the spatial-focus context: `useSpatialFocusActions().getFocusedMoniker()` or equivalent. Drop the entity-focus dependency.

**Recommendation: A.** Plenty of code already reads `useFocusedMoniker()` / `focusedMonikerRef`. Bridging once is cheaper than rewiring every consumer. The bridge happens in `EntityFocusProvider` (or wherever `setFocus` lives) — when the spatial-focus event fires, look up the moniker for the new key in the registry and call `setFocus(moniker)`. This makes spatial-focus the source of truth and keeps entity-focus a write-through cache.

## Files involved

- `kanban-app/ui/src/lib/spatial-focus-context.tsx` (event handler that needs to bridge to entity-focus)
- `kanban-app/ui/src/lib/entity-focus-context.tsx` (the legacy `setFocus` / `focusedMonikerRef` API)
- `kanban-app/ui/src/components/board-view.tsx` (the `board.inspect` command — verify it works after the bridge)
- Any other command factory that reads `focusedMonikerRef.current` — sweep for them

## Acceptance Criteria

- [x] Pressing Space with focus on any leaf (column header, card title, status pill, field row, field label, field pill, navbar button, perspective tab, grid cell) opens the inspector for that entity
- [x] Pressing Space with focus on a Zone (column, card, panel, navbar) opens the inspector for the zone's entity if applicable, or no-ops cleanly with a logged reason
- [x] `focusedMonikerRef.current` reflects the spatially-focused moniker after every click and after every keyboard nav
- [x] Existing `app-shell.test.tsx:698` "Space dispatches a command with keys.cua=Space" test still passes
- [x] Drill-in (Enter) and drill-out (Escape) still work as specified in this card's prior body

## Tests

- [x] `app-shell.test.tsx` — focus a leaf via emitted `focus-changed` event (the spatial-only flow that mimics a click), press Space, assert the FocusScope's command fires (covers both the `dispatchInspect`-via-bridge contract and the spatial-only entry path).
- [x] `spatial-focus-context.test.tsx` — `focus-changed` event with a `next_key` that resolves to a known moniker delivers `payload.next_moniker` to every registered `subscribeFocusChanged` listener; `next_moniker: null` delivers a null payload (the entity-focus side mirrors that into `setFocus(null)`).
- [x] `entity-focus-context.test.tsx` — bridge integration tests assert that mounting `<EntityFocusProvider>` inside `<SpatialFocusProvider>` mirrors `payload.next_moniker` into the entity-focus store across successive focus moves, including the `null` clear case, and that the legacy `<EntityFocusProvider>`-only contract still works.
- [x] Run `cd kanban-app/ui && npx vitest run` — 1597 of 1597 tests pass.

## Workflow

- Wait for `01KQ5PP55S` (architecture fix) to land before starting.
- Use `/tdd` — write the failing app-shell test first (Space on a focused leaf calls `dispatchInspect`), watch it fail, then implement the bridge.

## Implementation summary (2026-04-26)

Design: **Option A**, as the card recommended. The spatial-nav kernel
(`SpatialFocusProvider`) is the source of truth. The legacy entity-focus
store becomes a write-through cache that mirrors `payload.next_moniker`
from every `focus-changed` event into `FocusStore.set` via the existing
`setFocus` action.

### `kanban-app/ui/src/lib/spatial-focus-context.tsx`

- Added a `FocusChangedSubscriber` callback type — receives the full
  `FocusChangedPayload` (including `next_moniker`).
- Added `SpatialFocusActions.subscribeFocusChanged(subscriber): () => void`.
  The provider's single global `focus-changed` listener walks a snapshot
  of the subscriber set on every event and forwards the payload before
  recording `focusedKeyRef`. Subscribers register/unsubscribe via the
  returned closure; identity-stable, no extra Tauri listeners.
- Provider keeps a `subscribersRef = useRef<Set<FocusChangedSubscriber>>`
  in addition to the existing `registryRef` / `focusedKeyRef`.
  `buildSpatialFocusActions` accepts the new ref and exposes the new
  action.

### `kanban-app/ui/src/lib/entity-focus-context.tsx`

- `EntityFocusProvider` now reads `useOptionalSpatialFocusActions()` and,
  when present, registers a `subscribeFocusChanged` listener inside a
  `useEffect`. The listener calls `actions.setFocus(payload.next_moniker)`
  — full action, not bare `store.set`, so the backend `scope_chain` stays
  consistent. The bridge degrades silently when no
  `<SpatialFocusProvider>` ancestor is mounted (legacy unit tests still
  work). No feedback loop: `ui.setFocus` does not re-emit
  `focus-changed`.

### Tests

- `kanban-app/ui/src/lib/spatial-focus-context.test.tsx`: three new
  cases on `subscribeFocusChanged` covering broadcast, unsubscribe,
  and `next_moniker` propagation.
- `kanban-app/ui/src/lib/entity-focus-context.test.tsx`: four new cases
  in a "spatial focus bridge" suite — payloads with a moniker, payloads
  clearing focus, successive moves keep `focusedMonikerRef` in sync, and
  the no-`<SpatialFocusProvider>` fallback still works. Also reworked
  the file's `@tauri-apps/api/event` mock to capture the
  `focus-changed` callback so the bridge tests can fire synthetic
  payloads.
- `kanban-app/ui/src/components/app-shell.test.tsx`: extended the shared
  `emitFocusChanged` helper to take an optional separate `nextMoniker`
  argument (default: same as `nextKey`), and added a "Space dispatches
  inspect for a moniker focused only via spatial-focus" test that
  drives the bridge end-to-end without ever calling
  `setFocus` directly.
- `kanban-app/ui/src/components/store-container.test.tsx`: added a
  `vi.mock("@tauri-apps/api/event", …)` block so the
  `EntityFocusProvider` import path (which now transitively pulls in
  `spatial-focus-context.tsx`) doesn't trip on the
  `transformCallback`-from-mocked-`core` import error.

### Verification

- `cd kanban-app/ui && npx vitest run` — 1597 of 1597 tests pass (148
  of 148 test files), runs clean across two consecutive runs.
- `cd kanban-app/ui && npx tsc --noEmit` — clean.
- `cargo build --workspace` — clean.
- `cargo test -p swissarmyhammer-focus` — 119 tests pass across 11
  binaries.

---

(Original description preserved below.)

## (Prior) What

Add explicit drill-in and drill-out commands to complement the three-rule beam search. Arrow keys handle ordinary nav; these commands give access to zone-level focus (drill-out) and into-zone descent (drill-in), matching the nested zone model. All signatures use newtypes — `Option<Moniker>` returns, `SpatialKey` inputs.

Key-chord change: **Inspect moves from Enter to Space**, freeing Enter for drill-in / activate.

## (Prior) Commands

- `nav.drill_in` — Enter (CUA) / Enter or l (vim). On a Zone: focus its `last_focused` or first child. On a Focusable with edit affordance: enter inline edit. Otherwise: no-op.
- `nav.drill_out` — Escape (CUA) / Escape or h (vim). In edit: exit edit. On a Focusable: focus its `parent_zone`. On a Zone: focus *its* `parent_zone`. At layer root: fall through to `app.dismiss`.
- `ui.inspect` — Space (CUA). Opens inspector panel for the focused entity by Moniker.

## (Prior) Crate placement

- `drill_in` / `drill_out` methods in `swissarmyhammer-focus/src/registry.rs`
- Tauri adapters `spatial_drill_in` / `spatial_drill_out` in `kanban-app/src/commands.rs`
- React commands in `kanban-app/ui/src/`
- Tests in `swissarmyhammer-focus/tests/drill.rs`