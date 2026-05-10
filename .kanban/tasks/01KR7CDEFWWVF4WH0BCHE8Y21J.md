---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: Each open inspector panel must be its own `<FocusLayer>` (containment is broken — focus leaks to base)
---
## What

The inspector overlay is supposed to act as a modal focus boundary, but in practice focus leaks out to the parent `window` layer while at least one inspector panel is open. Three observable symptoms confirm this:

1. **Jump-To paints pills on the base layer.** Pressing `s` (vim) or `Mod+G` (cua/emacs) opens `<JumpToOverlay>`, which enumerates scopes via `useJumpTargets` (`kanban-app/ui/src/components/jump-to-overlay.tsx:524-566`). That hook resolves the layer of `priorFocusedFq` via `spatial.layerFqOf(priorFocusedFq)` and calls `enumerateScopesInLayer(...)`. If the inspector were holding focus, the resolved layer would be `/window/inspector/...` and pills would only land on inspector field scopes. Instead, pills land on board / column / card scopes — i.e. `priorFocusedFq` was on `/window`.

2. **Escape is "problematic" inside the inspector.** Drill-out walks the focused scope's parent_zone chain inside the snapshot's layer; if focus is on a card on `/window` (not on a field inside `/window/inspector`), `nav.drillOut` walks the wrong tree.

3. **Arrow-key navigation walks out of the inspector and visibly focuses cards on the base layer.** The kernel's snapshot is layer-scoped (`swissarmyhammer-focus/src/snapshot.rs:86-94`: `pub layer_fq: FullyQualifiedMoniker; pub scopes: Vec<SnapshotScope>`), and `buildSnapshotForFocused` in `kanban-app/ui/src/lib/spatial-focus-context.tsx:536-548` picks whichever layer registry `has` the focused FQM. Arrow-key nav from the inspector body to a card on the board can only happen if the focused FQM is on `/window` — which is exactly what symptom (1) implies.

## Root cause

`<FocusZone>` has been removed from the codebase (no `kanban-app/ui/src/components/focus-zone.*` file, no exported `FocusZone` symbol). With the container primitive gone, the two remaining primitives are:

- `<FocusLayer>` — modal containment boundary (kernel pushes/pops a layer; navigation never crosses).
- `<FocusScope>` — leaf in the spatial graph (`kanban-app/ui/src/components/focus-scope.tsx:50-68` documents the "scope-is-leaf invariant" — registering further scopes underneath logs `scope-not-leaf`).

Today `kanban-app/ui/src/components/inspectors-container.tsx:419` wraps each open panel body in `<FocusScope moniker={entityZoneSegment}>`. That scope is not a leaf — `<EntityInspector>` registers a field `<FocusScope>` per row underneath it. The kernel can't treat that wrapper as a real containment boundary because it isn't a layer; it's a leaf with rogue descendants. So the per-entity barrier the doc comment in `inspectors-container.tsx:80-98` describes ("Iter 0 of the kernel's beam-search cascade is confined to peers within the same entity") simply does not exist at runtime, and field focus claims regularly miss the inspector layer entirely (the prior board focus stays on `/window`).

**Fix shape.** Each open inspector panel must be its own `<FocusLayer>`, nested inside the existing `<FocusLayer name="inspector">`. That gives each panel a real containment boundary the kernel honors:

- The outer `<FocusLayer name="inspector" parentLayerFq={windowLayerFq}>` stays as the inspector tier (z-index, focus-debug overlay tier).
- Inside, replace `<FocusScope moniker={entityZoneSegment}>{body}</FocusScope>` with `<FocusLayer name={entityZoneSegment} parentLayerFq={inspectorLayerFq}>{body}</FocusLayer>`. The layer's name segment IS the entity moniker (`task:T1`, `tag:bug`, etc.), so the FQM composes to `/window/inspector/task:T1` — same path the current scope produces, but now as a real layer with its own `LayerScopeRegistry`.
- Field `<FocusScope>` rows inside register against THAT entity-panel layer's registry, so navigation, jump-to enumeration, and drill-out all operate within the panel.
- `useFirstFieldFocus` in `kanban-app/ui/src/components/entity-inspector.tsx:122-149` claims into a field that lives in the panel-layer's registry, so `buildSnapshotForFocused` finds it and the kernel commits the claim.
- Stack semantics: opening a second panel pushes a sibling layer; the kernel's per-layer parent chain (inspector → window) plus the new panel-tier means drill-out from a deep field walks panel → inspector → window in that order. Closing the topmost panel pops its layer and emits `last_focused` for the parent, restoring focus correctly without any React-side `useRestoreFocus` hack.

The `FocusLayer` push/pop machinery (`kanban-app/ui/src/components/focus-layer.tsx:176-237`, `swissarmyhammer-focus/src/state.rs`) already supports nested layers under a non-window parent — `parentLayerFq` is the explicit prop for exactly this case (the palette and inspector layers already use it).

## Files to read / modify

- `kanban-app/ui/src/components/inspectors-container.tsx` — replace the per-panel `<FocusScope>` wrap with a nested `<FocusLayer>` keyed by the entity moniker; thread the inspector layer FQM through to set `parentLayerFq`.
- `kanban-app/ui/src/components/focus-layer.tsx` — confirm the `LAYER_Z_TIERS` table (lines 107-116) handles a panel-tier descendant under `inspector` correctly (it already falls back to `parentTier + 20` for unnamed tiers).
- `kanban-app/ui/src/components/entity-inspector.tsx` — `useFirstFieldFocus` should "just work" once each panel is its own layer; verify with the new test.
- `kanban-app/ui/src/components/inspectors-container.guards.node.test.ts` — the existing source-level guard "permits a single FocusScope import" (line 101 onwards) is now stale and must flip to forbidding `<FocusScope>` here and asserting the nested `<FocusLayer>` wrap.
- `kanban-app/ui/src/components/inspectors-container.test.tsx` and `inspector.close-restores-focus.browser.test.tsx` — assertions that pin the old single-layer-with-scope shape need updating.
- `swissarmyhammer-focus/src/snapshot.rs`, `swissarmyhammer-focus/src/state.rs` — read-only; confirms layer-scoped snapshot and parent-layer fallback semantics.

## Acceptance Criteria

- [ ] No `<FocusScope>` import or element appears in `kanban-app/ui/src/components/inspectors-container.tsx` (the per-panel wrap is a `<FocusLayer>`, not a scope).
- [ ] Each open inspector panel mounts its own `<FocusLayer>` whose `name` segment is the entity moniker (e.g. `task:T1`) and whose `parentLayerFq` is the inspector layer FQM (`/window/inspector`). Verified by an automated test that inspects the layer registry after opening one and then two panels.
- [ ] With at least one inspector panel open, `spatial.focusedFq()` returns an FQM whose path starts with `/window/inspector/<entity-moniker>/...` immediately after the panel mounts.
- [ ] Pressing `s` (vim) / `Mod+G` (cua/emacs) while a panel is open opens `<JumpToOverlay>` and every enumerated target FQM lives under that panel's layer — NOT under `/window/board/*`.
- [ ] Pressing ArrowLeft / ArrowRight / ArrowUp / ArrowDown inside an inspector field never moves focus to a scope outside the panel layer while the inspector is open.
- [ ] Pressing Escape inside an inspector field dismisses the topmost panel via `ui.inspector.close` (popping that panel's layer; the kernel restores focus to the parent layer's `last_focused`).
- [ ] Opening a second panel while the first is still open creates a sibling layer under `/window/inspector` (not nested inside the first panel's layer); arrow-keys cross between the two panels via the inspector layer's iter-1 escalation.
- [ ] No `scope-not-leaf` warning is emitted during a panel-open / panel-close cycle (asserted by capturing `console.error` / `console.warn` and grepping for the literal token).

## Tests

- [ ] New browser test `kanban-app/ui/src/components/inspectors-container.layer-containment.browser.test.tsx` that:
  - opens an inspector panel,
  - asserts the layer registry contains a fresh layer at `/window/inspector/<entity-moniker>`,
  - asserts the focused FQM starts with that layer's FQM,
  - drives Arrow keys and asserts focus stays under the panel layer,
  - opens Jump-To and asserts every rendered pill's FQM is under the panel layer,
  - presses Escape and asserts the panel layer was popped and focus restored to the prior board scope.
- [ ] Update `inspectors-container.guards.node.test.ts`: replace the "permits a single FocusScope import" guard with a "must NOT import FocusScope" guard, and add a guard pinning the nested `<FocusLayer name=…>` wrap with `parentLayerFq` set to the inspector layer FQM.
- [ ] Existing `inspectors-container.test.tsx`, `inspector.close-restores-focus.browser.test.tsx`, `spatial-nav-jump-to.spatial.test.tsx`, and `inspector.kernel-focus-advance.browser.test.tsx` continue to pass (regression coverage — update assertions where they pinned the scope shape).
- [ ] Run `cd kanban-app/ui && npm test -- inspectors-container && npm test -- jump-to && npm test -- entity-inspector` and confirm green.

## Workflow

- Use `/tdd` — write the failing layer-containment browser test first (it should fail today by showing focus on `/window/board/...`), then convert the per-panel wrap to a `<FocusLayer>` until the test passes.
- Don't add a sentinel `app.dismiss` scope as a workaround — the focus-leak symptoms (Escape included) are downstream of the real bug. Make the per-panel layer real and verify the symptoms disappear before reaching for any additional plumbing.