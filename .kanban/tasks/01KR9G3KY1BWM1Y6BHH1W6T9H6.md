---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Inspector first-field auto-focus only fires on the FIRST inspect of a session
---
## Bug

User report: "inspecting isn't reliably focusing the inspector layer — seems to only work the first time".

After the modal-layer refactor in `01KR7CDEFWWVF4WH0BCHE8Y21J`, the first time an inspector opens in a session, `useFirstFieldFocus` correctly dispatches `nav.focus` and focus lands inside the inspector layer. On subsequent opens (close inspector → click another card to inspect) focus does NOT move into the inspector — it stays on the clicked card.

## Status

**BLOCKED — harness cannot reproduce; needs live verification.**

### Work delivered this iteration

1. **Kernel simulator enhanced** to mirror the real Rust kernel's behavior more faithfully (`kanban-app/ui/src/test-helpers/kernel-simulator.ts`):
   - `LayerRecord` now carries a `lastFocused: FullyQualifiedMoniker | null` slot.
   - `spatial_focus` walks `snapshot.layer_fq` up the parent chain and writes the focused FQM into each ancestor layer's `lastFocused` (mirrors `swissarmyhammer-focus/src/registry.rs::record_focus`).
   - `spatial_pop_layer` returns the popped layer's `lastFocused` (matches `kanban-app/src/commands.rs::spatial_pop_layer`'s contract).
   - New `KernelSimulatorOptions { strictFocusValidation }` flag. When true, `spatial_focus` rejects undefined snapshots, snapshots whose `layer_fq` isn't pushed, and FQMs not in `snapshot.scopes` (mirrors `state.rs::focus`'s validation). Kept off by default so existing React-bridge tests that mock setFocus without a layer push (e.g. `entity-focus.kernel-projection.test.tsx`) keep working.

2. **Test rewritten to match production flow** (`kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx`):
   - Renders two real `<FocusScope moniker="task:TA">` / `task:TB` card scopes inside the window layer, alongside `<InspectorsContainer>`.
   - Each "inspect" cycle dispatches `nav.focus(cardFq)` first (via a captured `useDispatchCommand` ref) and then pushes the entity onto `inspector_stack` + emits `ui-state-changed`. Mirrors the production gesture (FocusScope click + Inspectable double-click both fire from the same DOM event).
   - Three-cycle scenario: open A, close, open B, close, open A again. Each open asserts a `spatial_focus` IPC against the new entity's first field's FQM under `/window/inspector/...` AND that the entity-focus probe reflects the new field's FQM.
   - Opts into `strictFocusValidation: true` so the simulator's `record_focus` walk fires only for accepted commits — exactly the path the real kernel takes.

### Result

**Test passes**, even with the harness extensions. The simulator faithfully reproduces production's `spatial_focus` / `spatial_pop_layer` IPC trace (verified via tracing diagnostic during development), but the auto-focus-fails-on-second-open behavior the user reports does not manifest under the kernel-simulator + happy-DOM + Chromium browser-mode harness.

### What this means

The harness models the kernel's layer push/pop, `last_focused` walk, and snapshot-driven `spatial_focus` validation. With those mirrors in place, every inspector mount in the test dispatches `nav.focus` against the right FQM AND the dispatch is accepted by the simulator. The IPC trace shows the auto-focus correctly fires on cycles 1, 2, and 3.

The remaining gap likely involves one or more of:
- Real DOM focus events (the simulator doesn't model `<input>`-style native focus reclaim).
- Real Tauri IPC ordering (async over a different scheduler than the JS event loop).
- A kernel-side `focus_lost` or `focus_by_window` race the JS simulator's synchronous walk doesn't model.
- An interaction with the actual `<Inspectable>` double-click handler (not used in the test — the test fakes the inspect-open by mutating `inspector_stack` and emitting `ui-state-changed`).

### Acceptance Criteria

- [x] Open inspector A → focus lands in A's first field. Close A. Open B → focus lands in B's first field. Close B. Open A again → focus lands in A's first field. **Verified by `inspector.repeat-open-focus.browser.test.tsx` (passes).**
- [ ] No `spatial_focus failed:` console.error during normal inspect/close/inspect cycles. **Cannot verify under the simulator harness — needs live `cargo tauri dev` reproduction.**
- [x] Full UI suite green. **2074/2074 pass.**
- [x] Rust workspace green. **13482/13482 pass.**
- [x] tsc clean.
- [x] clippy clean.

### Tests

- [x] New browser test `kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx` — exercises three-cycle close/open with full card-click fidelity. Stands as a regression guard for the React-side `useFirstFieldFocus` contract: nav.focus fires for the new entity's first field on every inspector mount.
- [x] Existing `inspectors-container.auto-focus-on-mount.browser.test.tsx` continues to pass.
- [x] Existing `inspector.close-restores-focus.browser.test.tsx` continues to pass.
- [x] Existing `entity-focus.kernel-projection.test.tsx` continues to pass (the new `strictFocusValidation` flag was added as opt-in to preserve this).

### What to do next

The harness reaches its faithfulness limit. To make further progress:

a) **Reproduce by hand against a running app build** (`cargo tauri dev`) to confirm the bug still manifests today. The previous implementer noted this was option (a). If it doesn't manifest, close as obsolete.

b) If the bug DOES still manifest live, add `RUST_LOG=swissarmyhammer_focus=trace,kanban_app=trace` and capture the IPC + kernel event trace from a real reproduction. Compare against the simulator trace logged at the bottom of the development session for this card. The first divergence is the root cause.

c) The fix is still expected to be small and local (per the original hypotheses). Most likely candidates remain (1) layer push/pop race and (2) `last_focused`-driven kernel restore racing the auto-focus dispatch. Without a live trace, picking between them is speculation.

## Files touched

- `kanban-app/ui/src/test-helpers/kernel-simulator.ts` — added `lastFocused` per-layer tracking, `record_focus` walk on `spatial_focus`, `last_focused` return on `spatial_pop_layer`, opt-in strict snapshot validation.
- `kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx` — rewritten to render two real card scopes, dispatch `nav.focus` per click, exercise three inspect cycles, and opt into strict simulator validation.

---

## Original investigation guidance (preserved for reference)

### Likely paths to investigate

This is a state/ordering bug, not a structural one. Hypotheses ranked by likelihood:

#### (1) Layer push/pop race when panel stack briefly drops to 0

`kanban-app/ui/src/components/inspectors-container.tsx` mounts `<FocusLayer name="inspector">` only when `panelStack.length > 0`. When the user closes one inspector and immediately opens another, the path through `panelStack` can be 1 → 0 → 1 within React's commit ordering.

#### (2) `last_focused` state on the parent layer drives a kernel-side focus restore that fights the React-side `nav.focus` dispatch

When the inspector layer pops, the kernel returns the popped layer's `last_focused` (which is the field that was focused inside the layer). The React side dispatches a follow-up `spatial_focus`. When the next inspector immediately mounts, `useFirstFieldFocus`'s deferred `nav.focus` dispatch races the kernel's restore.

#### (3) `mountedRef` retention or the `prevFocusRef` capture missing across opens of the SAME entity

`kanban-app/ui/src/components/entity-inspector.tsx::useFirstFieldFocus`.

#### (4) `firstFieldFq` identity stable across opens — useEffect doesn't re-run

`useMemo<FullyQualifiedMoniker>` in `entity-inspector.tsx`.

#### (5) The deferred `queueMicrotask` cancellation flag is sticky

Should be fine.

### Files most likely involved

- `kanban-app/ui/src/components/entity-inspector.tsx::useFirstFieldFocus`
- `kanban-app/ui/src/components/inspectors-container.tsx`
- `kanban-app/ui/src/components/focus-layer.tsx`
- `kanban-app/ui/src/lib/spatial-focus-context.tsx`
- `kanban-app/src/commands.rs::spatial_push_layer` / `spatial_pop_layer`
- `swissarmyhammer-focus/src/state.rs`

### Reproduction recipe (per user)

1. Start the app with focus on a card on the board.
2. Click that card to inspect → assert focus is inside the inspector field (works).
3. Press Escape (or click backdrop) to dismiss.
4. Click a DIFFERENT card to inspect → focus stays on the card; pressing arrow keys navigates the board.

The user's exact phrasing: "seems to only work the first time".