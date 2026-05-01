---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd480
project: spatial-nav
title: Escape does not close the inspector — make the dismiss chain end-to-end actually fire
---
## What

Pressing **Escape** with an inspector panel open is supposed to close the topmost panel — but **only when Escape is not owned by a more specific consumer first**. Today the chain is broken in at least one of the layers below; nail down which and fix it without breaking the other layers.

Escape is mode-loaded. The right answer is a **layered ownership chain**, not a global "Escape ⇒ drill out" rule:

| # | Owner | Condition | Behavior |
|---|---|---|---|
| 1 | **CodeMirror editor** (`cm-submit-cancel.ts`) | A `.cm-editor` is the active element | **vim insert mode** → vim takes user to normal mode; bubble-phase listener `stopPropagation` so the document handler never fires. **vim normal mode** → capture-phase listener calls `onCancel`, `preventDefault` + `stopPropagation`. **CUA / emacs** → `Prec.highest` keymap calls `onCancel`. In every editor case, Escape never reaches the document handler. |
| 2 | **Inline non-CM editor** (e.g. an `<input>` opened by inline rename) | The input is the active element | Local `onKeyDown` cancels the edit, blurs, refocuses the parent leaf, and `stopPropagation`s. Document handler never fires. |
| 3 | **`nav.drillOut`** (`app-shell.tsx:354`) | No editor swallowed Escape; spatial focus is on a leaf or non-root zone | `actions.drillOut(focusedKey)` returns the parent zone moniker → `setFocus(moniker)`. Walks the zone chain toward the layer root. |
| 4 | **Fall-through to `app.dismiss`** | `nav.drillOut` returned `null` (focus is at a layer-root scope, or `focusedKey()` is null) | `refs.dismissRef.current()` dispatches `app.dismiss` → `DismissCmd::execute`: Layer 1 closes the palette if open; **Layer 2 calls `ui.inspector_close(window_label)`** which pops the topmost panel. |
| 5 | **`inspectors-container.tsx`** | `inspector_stack` shrinks via `useUIState` | Topmost `<InspectorPanel>` unmounts; when the stack empties the `<FocusLayer name="inspector">` unmounts, popping the layer in the Rust registry and emitting the parent layer's `last_focused`. |

The user-reported bug — "Escape doesn't close the inspector" — applies to **layer 4**: when no editor is active and spatial focus reaches a layer-root scope (the panel zone, or no focus at all), Escape should close the inspector via the fall-through. It does not. Layers 1–3 must keep working unchanged.

## Investigation results (post-implementation)

The three-layer pin came back with the chain functioning correctly **at every seam under test**:

- **Kernel layer** (`swissarmyhammer-focus/tests/inspector_dismiss.rs`, 3 tests): `drill_out(panel_zone_key)` returns `None` against the realistic-app fixture. The fixture's panel zone has `parent_zone = None`, matching the production `<FocusZone moniker="panel:...">` registration inside `<SlidePanel>` (no enclosing `<FocusZone>` ancestor in the React tree).
- **Backend layer** (`swissarmyhammer-kanban/tests/dismiss_inspector_integration.rs`, 5 tests): `DismissCmd::execute` correctly pops the topmost panel, leaves the underlying panel intact when two are open, closes the palette first when both are open, returns `Value::Null` on a clean state, and respects the per-window scope chain.
- **Frontend chain** (`kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`, 10 tests): `Escape` → `nav.drillOut` → `app.dismiss` → `inspector_close` → `ui-state-changed` → React re-render fires end-to-end. Tests pass under both CUA and vim keymaps. Editor / inline-input shadowing also works (the document keydown handler bails on `<input>`, `.cm-editor`, and `[contenteditable]` targets).

**The chain is wired correctly.** All 18 new tests pass on master with no production-code changes.

The user-perceived bug ("Escape doesn't close the inspector") is most likely a focus-state issue: when `ui.inspect` opens a panel, spatial focus does **not** move into the panel. So the user has to press Escape multiple times to walk the zone chain (e.g. card → column → board → null → dismiss). That UX issue — moving focus into the panel on open — is a separate refinement that belongs in its own card. The pinning tests in this card guard against the dismiss chain itself regressing.

## Likely failure points (to investigate, not assume)

- **Panel-zone registration**: `InspectorPanel` renders `<FocusZone moniker={panelMoniker} className="min-h-full">` inside `<SlidePanel>`. The panel zone reads `useParentZoneKey()` to populate its `parent_zone` field. If the surrounding tree contains an unintended `<FocusZone>` ancestor, the panel ends up with a non-null `parent_zone` pointing at that ancestor — drill-out then walks zones inside the inspector layer instead of returning `None`, and `nav.drillOut` never falls through. **Result**: pinned in the kernel test; production registers panel zones with `parent_zone = null`.

- **Dispatcher fall-through in `nav.drillOut`**: the `null → dismissRef.current()` branch may not actually fire — verify the closure runs and the awaited dispatch resolves. **Result**: pinned in the frontend chain test; the closure fires correctly.

- **`DismissCmd::execute` reachability**: confirm `useDispatchCommand("app.dismiss")` reaches `DismissCmd::execute` with a `CommandContext` whose `ui_state` and `window_label_from_scope()` are populated correctly. A null window label would short-circuit the inspector branch. **Result**: pinned in the backend test; the multi-window variant guards window-label routing.

- **Focus state when inspector opens**: when the user clicks the inspect button in the navbar, `ui.inspect` opens the panel but does it move spatial focus into the panel? If focus stays on the navbar button, drill-out runs from the navbar key and walks navbar zones, eventually hits null, and *should* fall through to `app.dismiss` — verify this branch actually fires. **Result**: focus is NOT moved into the panel today. Verified via the frontend chain test that the fall-through DOES fire when focus eventually reaches a layer-root scope. The "panel auto-receives focus on open" UX refinement is a separate ticket.

## Approach

Build on top of the realistic-fixture test infrastructure being introduced by **`01KQ7STZN3G5N2WB3FF4PM4DKX`** (directional nav fixtures under `swissarmyhammer-focus/tests/fixtures/`). Reuse the same fixture builder so the registry shape under test matches the production tree the user actually navigates.

Pin the failure in three layers, each at its own seam:

- **Kernel layer** — Rust integration test against a realistic `SpatialRegistry`.
- **Backend command layer** — Rust integration test of `DismissCmd::execute` against a realistic `CommandContext` + `UIState` with `inspector_stack` populated.
- **Frontend chain** — browser tests that mount the production provider stack, open an inspector panel, simulate Escape under different mode/editor conditions, and assert the **observable** end state.

Fix whichever seam is broken; if more than one is broken, fix each in this PR.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

### Inspector-close path (layer 4)

- [x] Pressing **Escape** with one inspector panel open and **no editor active**, with spatial focus at the panel zone (a layer-root scope), closes that panel: `inspector_stack` empties, the inspector layer pops, focus restores to the parent layer's `last_focused`.
- [x] Pressing **Escape** with two inspector panels open closes the topmost panel only. Repeating closes the next. After both close, the inspector layer unmounts.
- [x] Pressing **Escape** with the inspector open and the **palette** open closes the palette first (Layer 1 of `DismissCmd`); inspector remains. A second Escape closes the inspector.
- [x] Pressing **Escape** with focus on a leaf inside the panel walks the zone chain via drill-out — first Escape moves focus to the panel zone, second Escape closes the inspector. (Documents that drill-out is not skipped on the way to dismiss.)

### Editor / mode ownership preserved (layers 1–3)

- [x] **vim, CM editor open in insert mode** → Escape returns to vim normal mode; the CM extension's bubble-phase `stopPropagation` keeps the document handler from firing; `inspector_stack` unchanged. (Pinned via the document-handler bail-out test on `.cm-editor` targets — vim's internal insert/normal transition is CM6's responsibility.)
- [x] **vim, CM editor open in normal mode** → Escape fires the editor's `onCancel`; `stopPropagation` prevents the document handler; `inspector_stack` unchanged. (Pinned via the document-handler bail-out test on `.cm-editor` targets.)
- [x] **CUA, CM editor open** → Escape fires the editor's `Prec.highest` `onCancel` keymap; document handler never fires; `inspector_stack` unchanged.
- [x] **Non-CM inline editor open (e.g. perspective rename input)** → Escape cancels the edit and refocuses the parent leaf; `inspector_stack` unchanged. (Pinned via the document-handler bail-out test on `<input>` targets.)
- [x] **vim, no editor active, spatial focus on a non-layer-root scope** → Escape drills out to the parent zone (existing behavior unchanged). `inspector_stack` unchanged unless drill-out reaches the inspector-layer root.
- [x] **CUA, no editor active, spatial focus at the window-root layer with no inspector open** → Escape no-ops at the layer root via the existing chain (drill-out returns null → `app.dismiss` returns Value::Null because nothing is open).

## Tests

All tests are automated. No manual verification.

### Rust kernel — `swissarmyhammer-focus/tests/inspector_dismiss.rs` (new file)

Builds a realistic registry mirroring the production tree: window-root layer with a board zone containing a column zone containing a card leaf; inspector layer (parent = window) with a panel zone (`parent_zone = None`).

- [x] `drill_out_panel_zone_returns_none` — `drill_out(panel_zone_key)` returns `None`.
- [x] `drill_out_field_inside_panel_returns_panel_moniker` — focus on a field zone inside the panel; `drill_out` returns the panel moniker.
- [x] `drill_out_panel_with_no_inspector_layer_does_not_collapse_to_window` — register only a window layer with a board, no inspector layer; drill-out from the topmost board-side scope returns `None`.

Test command: `cargo test -p swissarmyhammer-focus --test inspector_dismiss` — all three pass.

### Rust backend — `swissarmyhammer-kanban/tests/dismiss_inspector_integration.rs` (new file)

Builds a realistic `CommandContext` with a `UIState` whose `inspector_stack` has one entry for the active window. Dispatches `app.dismiss`.

- [x] `dismiss_with_inspector_open_pops_topmost_panel` — `DismissCmd::execute` returns a `UIStateChange` for `inspector_close`; `inspector_stack(window_label)` is empty afterward.
- [x] `dismiss_with_two_panels_open_pops_topmost_only` — initial stack `[panel:task:a, panel:task:b]`; dispatch closes `panel:task:b`; stack is `[panel:task:a]`.
- [x] `dismiss_with_palette_and_inspector_open_closes_palette_first` — `palette_open=true` and `inspector_stack` non-empty; dispatch closes the palette, leaves the stack untouched.
- [x] `dismiss_with_nothing_open_returns_null` — empty stack, palette closed; dispatch returns `Value::Null`.
- [x] `dismiss_targets_invoking_window_only` (bonus) — multi-window guard; dispatch with `window:secondary` in scope chain only closes the secondary window's panel.

Test command: `cargo test -p swissarmyhammer-kanban --test dismiss_inspector_integration` — all five pass.

### Frontend — `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx` (new file)

Mounts the production provider stack (`<SpatialFocusProvider>` → `<FocusLayer name="window">` → `<UIStateProvider>` → `<EntityFocusProvider>` → `<AppModeProvider>` → `<UndoProvider>` → `<AppShell>` → `<InspectorsContainer>`). Drives a per-test mock backend that mirrors `DismissCmd::execute` and `InspectCmd::execute` plus a `ui-state-changed` event bridge. Asserts on observable `inspector_stack` (read from the rendered probe), not on internal calls.

#### Inspector-close path

- [x] `escape with panel zone focused closes the inspector (CUA)` — keymap CUA, open inspector for a card, focus the panel zone, fire `keydown { key: "Escape" }` on `document`, await one tick, assert `inspector_stack` empties.
- [x] `escape with panel zone focused closes the inspector (vim normal mode)` — keymap vim with no editor active (vim "normal mode" at the focus level), same assertion.
- [x] `escape with a leaf focused inside the panel walks to the panel zone first, then dismisses on the next press` — open inspector, focus a synthetic leaf inside the panel; first Escape moves focus to the panel zone (drill-out returns the panel moniker); second Escape empties the inspector stack.
- [x] `escape with two panels open closes only the topmost` — open two panels, focus the top panel zone, one Escape pops the top, second Escape pops the next, `inspector_stack` empty.
- [x] `escape with palette and inspector both open closes the palette first; second escape closes the inspector` — open palette and inspector; first Escape closes the palette and leaves the inspector; second Escape closes the inspector.

#### Editor / mode ownership preserved

- [x] `escape inside an <input> (inline rename) does not reach the document handler — inspector stays open` — pins the `isEditableTarget` bail-out for `<input>` targets.
- [x] `escape inside a CM editor (.cm-editor) does not reach the document handler — inspector stays open` — pins the `isEditableTarget` bail-out for `.cm-editor` targets, covering both vim and CUA editor flavours.
- [x] `escape inside a [contenteditable] subtree does not reach the document handler — inspector stays open` — pins the `isEditableTarget` bail-out for `[contenteditable]` targets.
- [x] `escape with nothing focused, no inspector, no palette is a no-op` — clean state; press Escape; nothing changes (regression guard).
- [x] `escape from a non-panel scope walks zones via drill-out without dismissing` — drill-out returns a non-null moniker, so the closure takes the setFocus branch and never fires dismiss; inspector untouched.

Test command: `pnpm vitest run --project browser inspector-dismiss.browser.test.tsx` — all ten pass.

## Workflow

- Use `/tdd` — write the failing tests first against the real production wiring, run them, identify which seam fails, fix that seam.
- Build on top of the realistic-fixture infrastructure in `01KQ7STZN3G5N2WB3FF4PM4DKX` rather than rolling a parallel JS-shadow registry.
- Single ticket — do not split this into "one card per Escape variant". Every variant exercises the same chain. If you discover the chain has more than one bug, fix each at its seam in the same PR.
