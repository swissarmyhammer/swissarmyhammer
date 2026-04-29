---
assignees:
- claude-code
position_column: review
position_ordinal: '8180'
project: spatial-nav
title: 'Inspector layer simplification: drop panel zone + InspectorFocusBridge; field zones at layer root; pin barrier and cross-panel nav with TDD'
---
## What

Simplify the inspector spatial structure to a single shared `<FocusLayer name="inspector">` containing field zones directly at the layer root. Drop the panel zone (`<FocusZone moniker="panel:type:id">`) and the entire `<InspectorFocusBridge>` component. The Layer is the only barrier needed; nothing between Layer and Field zones earns its keep.

User direction:
> "I do not think we need a 'panel zone'."
> "I'm real skeptical about the InspectorFocusBridge purpose."
> "One layer for the whole panel stack allowing navigation between inspectors — which you should test."

## Why

The panel zone wraps each `<InspectorPanel>` body in a `<FocusZone>`. Its only structural job is to be the field zones' `parent_zone` so kernel iter 0 can find peers. But if every field zone in the inspector layer registers with `parent_zone = None`, iter 0 finds the same peers (any field zone in the same layer). Drill-out at layer root echoes the focused moniker (per `01KQAW97R9XTCNR1PJAWYSKBC7`'s contract), the React `nav.drillOut` chain detects equality and dispatches `app.dismiss` — same behavior, fewer entities.

InspectorFocusBridge wraps `<EntityInspector>` in a `<FocusScope moniker={entityMoniker}>` (a leaf in the kernel's vocabulary — and an entity is conceptually a zone, not a scope) plus a CommandScopeProvider for three edit-mode commands that duplicate per-field semantics already provided by `field.edit` at the field-zone scope. Once the panel zone is gone and the inspector nav commands are gone (`01KQCKVN140DGBCK8NF8RZM4R5`), what remains is dead scaffolding.

## Implementation summary

**Step 1 — Kernel-state shape-snapshot test (RED then GREEN)**
- Created `kanban-app/ui/src/test-helpers/kernel-simulator.ts` (~270 lines, ~160 LOC of code; the rest is doc comments). Records every spatial-nav IPC and routes spatial_navigate through the existing `navigateInShadow` cascade port from `src/test/spatial-shadow-registry.ts` so both modules stay in lock-step when kernel rules change. Wasm bindings to the real Rust kernel aren't available in browser tests, so cascade is simulated in TS — the tradeoff is documented in the file header.
- Created `kanban-app/ui/src/components/inspector-focus-bridge.layer-barrier.browser.test.tsx` (5 tests).

**Step 2 — Single-panel boundary nav test**
- Created `kanban-app/ui/src/components/inspector.boundary-nav.browser.test.tsx` (3 tests).

**Step 3 — Cross-panel nav test**
- Created `kanban-app/ui/src/components/inspector.cross-panel-nav.browser.test.tsx` (4 tests).

**Step 4 — Close-panel restores focus test**
- Created `kanban-app/ui/src/components/inspector.close-restores-focus.browser.test.tsx` (2 tests).

**Step 5 — Production refactor**
- Deleted `kanban-app/ui/src/components/inspector-focus-bridge.tsx`.
- `inspectors-container.tsx`: removed the per-panel `<FocusZone moniker="panel:*">` wrap; `<EntityInspector>` renders directly inside `<SlidePanel>`. Removed `ClaimPanelFocusOnMount` — the first-field focus claim is owned by `useFirstFieldFocus` already inside `EntityInspector` (which captures prev focus + restores on unmount).
- `entity-inspector.tsx`: removed the `navRef` prop and `InspectorFocusBridge` references.
- `fields/field.tsx`: extended scope-level edit-mode commands to mirror grid's split. `field.edit` now binds `vim:i` + `cua:Enter`; new `field.editEnter` binds `vim:Enter`. Both share the same drill-in-then-edit closure. This covers `inspector.edit` (vim:i, cua:Enter) and `inspector.editEnter` (vim:Enter) semantics; `inspector.exitEdit` had no keys and was driven by editor onCancel callbacks (the chain remains intact via FieldRow's `useFieldEditing`).

**Step 6 — Cleaned up superseded tests**
- Deleted: `entity-inspector.field-up-down.diagnostic.browser.test.tsx` (asserted panel-as-parentZone shape).
- Deleted: `inspector-focus-bridge.test.tsx` and `inspector-focus-bridge.unified-nav.browser.test.tsx` (tested deleted bridge).
- Deleted: `inspector-dismiss.browser.test.tsx` (tested panel-zone-as-layer-root drill-out; superseded by boundary-nav + close-restore tests).
- Deleted: `inspectors-container.enter-drill-in.browser.test.tsx` (tested Enter-on-panel-zone drill-in; concept gone).
- Deleted: `inspectors-container.spatial-nav.test.tsx` (tested panel-zone focus indicator + cross-panel fallback; superseded).
- Updated: `inspectors-container.guards.node.test.ts` — flipped from "panel zone must register" to "no panel zone, no FocusZone, no InspectorFocusBridge import".
- Updated: `inspectors-container.test.tsx` — removed panel-zone registration / unregistration assertions; kept layer push/pop semantics.
- Updated: `entity-inspector.test.tsx` — removed `renderViaInspectorBridge` helper and the entity-scope-wrapper test; deleted import.
- Updated: `keybindings.test.ts` — switched `inspector.edit/editEnter/exitEdit` references to `field.edit/field.editEnter`.
- Updated: `focus-on-click.regression.spatial.test.tsx` — removed the panel-background test (superseded by field-zone click contract).

## Test results

- All 14 new tests pass.
- Browser suite: 176 files / 1831 tests passing, 1 skipped, 0 failures.
- Node-only unit suite: 9 files / 89 tests passing, 0 failures.
- TypeScript clean (`npx tsc --noEmit`).
- Rust `swissarmyhammer-focus` and `swissarmyhammer-kanban` crates: all tests passing.
- (Pre-existing failure in `shelltool-cli` is unrelated.)

## Acceptance Criteria

- [x] `<InspectorFocusBridge>` component is deleted from the codebase. No imports of it remain.
- [x] `<FocusZone moniker="panel:*">` is no longer registered for inspectors. The kernel-state snapshot test confirms.
- [x] `<FocusScope moniker={entityMoniker}>` is no longer registered for inspectors.
- [x] All field zones inside an inspector register with `layerKey === inspectorLayerKey` AND `parentZone === null`.
- [x] `spatial_push_layer` for `name === "inspector"` fires before any field zone registers.
- [x] ArrowDown at the last field stays put (echoed moniker).
- [x] ArrowUp at the first field stays put.
- [x] No non-inspector moniker (board, column, card) appears in `useFocusedScope()` while the inspector is open.
- [x] Cross-panel nav works: ArrowLeft/Right between two open panels moves focus by rect.
- [x] Closing the topmost panel: layer pops when only panel closes; stays alive when one of two closes (regression guard preserved).
- [x] Existing tests in `01KQAXS8QKWCKFK8ENEMN7WHR1` and `01KQCKVN140DGBCK8NF8RZM4R5` pass after the refactor (the diagnostic test from `01KQAXS8QKWCKFK8ENEMN7WHR1` was superseded by the new layer-barrier test, which pins the new contract).

## Cross-references

`01KQAW97R9XTCNR1PJAWYSKBC7`, `01KQAXS8QKWCKFK8ENEMN7WHR1`, `01KQCKVN140DGBCK8NF8RZM4R5`, `01KQ9X3A9NMRYK50GWP4S4ZMJ4`.

## Review Findings (2026-04-29 10:55)

### Warnings
- [x] `kanban-app/ui/src/test-helpers/kernel-simulator.ts:253` — Simulator's `spatial_navigate` returns `undefined` (no `focus-changed` emit) when `navigateInShadow` returns null (e.g., focused entry is at layer root with no peer in direction). The real Rust kernel echoes the focused moniker AND emits a `focus-changed` event with that moniker per the no-silent-dropout contract (`navigate.rs` `cardinal_cascade` returns `focused_moniker.clone()` and the SpatialState emit-after-write fires). User-observable end-state is identical (focus stays put), but the IPC trace diverges from production. Future tests that count `focus-changed` events or assert on a moniker echo during a no-motion case will behave differently against the simulator vs. real kernel. Suggested fix: when `navigateInShadow` returns null, emit a synthetic `focus-changed` event with `prev_key=fromKey, next_key=fromKey, next_moniker=<focused entry's moniker>` so the simulator matches the kernel's emit-on-stay-put behavior.
  - **Resolution (2026-04-28):** When `navigateInShadow` returns null, the simulator now looks up the focused entry's moniker in its `registrations` map and emits a synthetic `focus-changed` event with `prev_key === next_key === fromKey` and `next_moniker = focused entry's moniker`. The module-level docstring documents the new emit-on-stay-put behavior under a "No-silent-dropout emit contract" section. All existing tests continue to pass (the change is additive — tests that didn't assert on the emit count are unaffected).

### Nits
- [x] `kanban-app/ui/src/components/app-shell.tsx:329` — Stale comment references `inspector.edit` as a scope-level shadow command, but `inspector.edit` was deleted in this card. The current comparable example is `field.edit` (vim:i / cua:Enter) on focused field zones. Suggested fix: change `` `inspector.edit`, card-name rename`` to `` `field.edit`, card-name rename``.
  - **Resolution (2026-04-28):** Comment updated to reference `field.edit` instead of `inspector.edit`.
- [x] `kanban-app/ui/src/lib/entity-focus-context.tsx:127` — Stale comment lists `inspector-focus-bridge` among the call sites that consume the navigate-callback compatibility shim. The bridge file was deleted in this card. Suggested fix: drop `inspector-focus-bridge` from the example list (`board-view, grid-view, app-shell` remain).
  - **Resolution (2026-04-28):** `inspector-focus-bridge` removed from the call-sites example list; the docstring now reads `(board-view, grid-view, app-shell)`.
- [x] `kanban-app/ui/src/components/inspector.boundary-nav.browser.test.tsx:14-16` — Docstring claims "the React adapter's drill-out chain detects equality and dispatches `app.dismiss`," but the test exercises `ArrowDown` / `ArrowUp` keys which dispatch `nav.down` / `nav.up`, not `nav.drillOut`. Only `nav.drillOut` (Escape) has the equality-→-`app.dismiss` fall-through. The actual assertion ("moniker stays put") is correct, but the docstring conflates the boundary-nav stay-put path with the Escape-driven dismiss path. Suggested fix: trim the docstring to describe what the test actually verifies — that boundary nav at the layer edge keeps focus on the same field — and move the dismiss-via-equality discussion to a test that actually fires Escape.
  - **Resolution (2026-04-28):** Docstring reworked to describe only the cardinal-direction stay-put path (`nav.down` / `nav.up`) that this test actually exercises. The Escape-driven `nav.drillOut` equality-→-`app.dismiss` discussion was dropped from this file; a one-line note redirects readers to "covered elsewhere" for that path.
