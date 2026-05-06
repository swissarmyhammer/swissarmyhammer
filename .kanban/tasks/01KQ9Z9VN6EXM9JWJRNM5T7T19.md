---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd780
project: spatial-nav
title: Inspector opens with focus stuck on the source element — should drill into the panel zone
---
## What

When the user opens an inspector panel — by clicking the navbar `Inspect` button, double-clicking a card, right-click → inspect, etc. — `ui.inspect` updates the backend `inspector_stack` and the React side renders the panel. **Spatial focus stays on the source element** (the navbar button, the card, the perspective tab, …) — it does NOT move into the new panel zone.

The user-visible symptom: pressing Escape does not appear to close the inspector. In reality the dismiss chain works correctly (pinned by [`01KQ9TVZYXN65JHA479D1CS91T`]'s tests), but because focus is on the source element, drill-out walks the source element's zone chain (e.g. `task:T1A` → `column:TODO` → `ui:board` → null → dismiss) before the dismiss step fires. That's three Escapes for a card-driven open, two for a navbar-driven open. The user reasonably expects one.

## Acceptance Criteria

- [x] After dispatching `ui.inspect` on any source element (card, navbar button, perspective tab, right-click target), spatial focus moves to the newly-mounted panel zone (`panel:<entityType>:<entityId>`). One Escape from there empties the inspector stack via the existing chain.
- [x] The panel-zone focus claim shows the visible `<FocusIndicator>` (the affordance the user reads as "this is the focused thing").
- [x] When the inspector closes, focus returns to the parent layer's `last_focused` — i.e. wherever it was before the user opened the inspector. (Same restore behavior the inspector layer already provides on unmount.)
- [x] Double-clicking a card to inspect it still leaves the card visibly focused for the brief moment between the dispatch and the panel mount, but once the panel is mounted, focus is on the panel zone.

## Tests

### Frontend — extend `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`

- [x] `inspect_dispatch_moves_focus_to_panel_zone` — render the AppShell + InspectorsContainer chain. Dispatch `ui.inspect` for a task with the navbar inspect button focused. After the panel mounts, assert the `focus-changed` event with `next_key = panel-zone-key` was emitted (or check the `data-focused="true"` attribute on the panel zone).
- [x] `escape_after_inspect_closes_in_one_press_from_navbar_open` — same setup, but press Escape after the panel mounts. Assert `inspector_stack` empties on the FIRST Escape (today it takes two).
- [x] `escape_after_inspect_closes_in_one_press_from_card_open` — set up a card focus, double-click → inspect, press Escape. Assert `inspector_stack` empties on the FIRST Escape (today it takes three).
- [x] `inspector_close_restores_previous_focus` — open inspector with focus on a known card, close inspector with Escape, assert focus is back on the card via `focus-changed` events.

### Backend — extend `swissarmyhammer-kanban/tests/dismiss_inspector_integration.rs` (or similar)

- [x] `inspect_emits_focus_change_to_panel` — N/A: the focus move is dispatched by the **frontend** (per the implementation note), not initiated by the backend. The backend's `InspectCmd::execute` and its `UIStateChange` payload remain unchanged. The new `<ClaimPanelFocusOnMount>` React helper handles the focus advance via `spatial_focus(panelKey)` once the panel zone is registered with the kernel. The end-to-end behavior is pinned by the four frontend tests above; no new backend assertions are needed.

## Implementation notes

The natural place for the focus move is on the React side: `InspectorPanel` already mounts the panel zone and registers a `SpatialKey`; an `useEffect` that fires `spatial_focus(panelKey)` on first mount would do it. The challenge is timing — the spatial registration happens in a `useEffect`, so the focus call has to wait for that effect to settle. A `useEffect` with the panel zone's key as dependency, gated on first run, is the simplest shape.

Alternative: the backend `InspectCmd::execute` could include a focus-target hint in its `UIStateChange` payload, and the React side would honour it on the next render cycle. That centralises the "what should be focused next" decision but adds wiring to the UIStateChange schema. Pick whichever fits the architecture better.

## Implementation chosen

Frontend approach. A new `<ClaimPanelFocusOnMount>` helper component is mounted as a child of each `<InspectorPanel>`'s `<FocusZone>` in `kanban-app/ui/src/components/inspectors-container.tsx`. It:

- Reads the panel zone's `SpatialKey` via `useParentZoneKey()` (the `<FocusZone>` mints its key internally and exposes it only through `FocusZoneContext`).
- On first mount, calls `spatial_focus(panelKey)` deferred via `queueMicrotask` so the parent `<FocusZone>`'s register effect (which fires AFTER this child effect, per React's bottom-up effect ordering on mount) has a chance to synchronously enqueue `spatial_register_zone(panelKey, …)` before this helper enqueues `spatial_focus(panelKey)`. Tauri serializes IPC commands through `with_spatial`'s mutex on the Rust side, so register + focus are processed in order.
- The panel `<InspectorPanel>` is keyed by `${entityType}-${entityId}` so each new entity gets a fresh helper that fires again — preserving drill-from-A-into-B behavior.

## Related

Discovered while implementing [`01KQ9TVZYXN65JHA479D1CS91T`]. The dismiss chain itself works correctly — the bug is upstream of it.

[`01KQ9TVZYXN65JHA479D1CS91T`]: # "Escape does not close the inspector — make the dismiss chain end-to-end actually fire"