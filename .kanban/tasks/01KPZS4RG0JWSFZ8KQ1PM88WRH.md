---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: doing
position_ordinal: '8980'
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

- [ ] Pressing Space with focus on any leaf (column header, card title, status pill, field row, field label, field pill, navbar button, perspective tab, grid cell) opens the inspector for that entity
- [ ] Pressing Space with focus on a Zone (column, card, panel, navbar) opens the inspector for the zone's entity if applicable, or no-ops cleanly with a logged reason
- [ ] `focusedMonikerRef.current` reflects the spatially-focused moniker after every click and after every keyboard nav
- [ ] Existing `app-shell.test.tsx:698` "Space dispatches a command with keys.cua=Space" test still passes
- [ ] Drill-in (Enter) and drill-out (Escape) still work as specified in this card's prior body

## Tests

- [ ] `app-shell.spatial-nav.test.tsx` — focus a leaf via simulated click, press Space, assert `dispatchInspect` was called with the leaf's moniker as target
- [ ] `app-shell.spatial-nav.test.tsx` — focus a leaf via simulated keyboard nav, press Space, assert same
- [ ] `spatial-focus-context.test.tsx` — `focus-changed` event with a `next_key` that resolves to a known moniker calls `setFocus(moniker)` on entity-focus; `next_key: null` calls `setFocus(null)`
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Wait for `01KQ5PP55S` (architecture fix) to land before starting.
- Use `/tdd` — write the failing app-shell test first (Space on a focused leaf calls `dispatchInspect`), watch it fail, then implement the bridge.

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