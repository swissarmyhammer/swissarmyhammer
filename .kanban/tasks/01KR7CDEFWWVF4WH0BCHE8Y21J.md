---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: 'Inspector layer fails to contain focus: `s`-jump marks the base layer, Escape leaks, arrow-nav escapes'
---
## What

The inspector overlay is supposed to act as a modal focus layer (`name="inspector"` mounted in `kanban-app/ui/src/components/inspectors-container.tsx:250`), but in practice focus is leaking out to the parent `window` layer while at least one inspector panel is open. Three observable symptoms confirm this:

1. **Jump-To paints pills on the base layer.** Pressing `s` (vim) or `Mod+G` (cua/emacs) opens `<JumpToOverlay>`, which enumerates scopes via `useJumpTargets` in `kanban-app/ui/src/components/jump-to-overlay.tsx:524-566`. That hook takes `priorFocusedFq` and resolves its layer via `spatial.layerFqOf(priorFocusedFq)`. If the inspector were holding focus, the resolved layer would be `/window/inspector` and pills would only land on inspector field zones. Instead, pills land on board / column / card scopes — i.e. `priorFocusedFq` was on `/window`, so the inspector layer never actually owned focus.

2. **Escape is "problematic" inside the inspector.** Unlike `<JumpToOverlay>` and `<CommandPalette>`, the inspector layer does NOT install a sentinel `<FocusScope>` with an `app.dismiss` shadow command. (Compare `kanban-app/ui/src/components/jump-to-overlay.tsx:242-275` where the jump-to layer wraps its body in a sentinel scope whose `commands` registers `app.dismiss` → `handleDismiss`.) Today, `inspectors-container.tsx:249-253` is just `<FocusLayer name=INSPECTOR_LAYER_NAME>{panelNodes}</FocusLayer>`. With nothing pinning `app.dismiss` at the inspector layer root, Escape's fall-through behavior depends on whether focus is actually inside the layer; with focus leaking to base, the drill-out chain walks the wrong tree.

3. **Arrow-key navigation walks out of the inspector and visibly focuses cards on the base layer.** The kernel's snapshot is layer-scoped (`swissarmyhammer-focus/src/snapshot.rs:86-94`: `pub layer_fq: FullyQualifiedMoniker; pub scopes: Vec<SnapshotScope>`), and `buildSnapshotForFocused` in `kanban-app/ui/src/lib/spatial-focus-context.tsx:536-548` picks whichever layer registry `has` the focused FQM. So the only way arrow keys can move focus from the inspector body to a card on the board is if the focused FQM is already on `/window` — which is exactly what symptom (1) implies.

### Likely root causes (investigate, don't assume)

- **The entity wrap is `<FocusScope>`, not `<FocusZone>`.** `inspectors-container.tsx:419` wraps each panel body in `<FocusScope moniker={entityZoneSegment}>`. The doc-comment block in the same file (lines 80-98) says it should be `<FocusZone>`, and `kanban-app/ui/src/components/focus-scope.tsx:50-68` documents a "scope-is-leaf invariant" — registering further scopes/zones underneath logs `scope-not-leaf`. Field rows DO register their own zones inside this wrap, so the kernel may be treating the field tree as leaks under a leaf, which would explain why first-field focus never sticks to the inspector layer.
- **No sentinel `app.dismiss` shadow at the inspector layer root.** Even if focus were inside the layer, drill-out has no terminal handler at the inspector edge. The jump-to and palette layers both install one and rely on it for clean dismissal; the inspector layer should follow the same pattern (dispatch `ui.inspector.close` on the inspector layer's `app.dismiss`, not on every panel's `<SlidePanel>` close button).
- **`useFirstFieldFocus` in `kanban-app/ui/src/components/entity-inspector.tsx:122-149` may be losing the race.** It calls `setFocus(firstFieldFq)` on mount, but the field zones it points at have to be registered with `LayerScopeRegistry` before `buildSnapshotForFocused` can find them. If the snapshot is built before any field zone has registered, `spatial_focus` is invoked with `snapshot: undefined` and the kernel rejects the claim (`state.rs:226-236`: `let layer = registry.layer(&snapshot.layer_fq)?;` returns `None`). The store stays at the previous focus (a card on the base layer), and the inspector layer is mounted without focus.

### Files to read / modify

- `kanban-app/ui/src/components/inspectors-container.tsx` — layer mount + entity wrap + close commands.
- `kanban-app/ui/src/components/entity-inspector.tsx` — `useFirstFieldFocus`.
- `kanban-app/ui/src/components/jump-to-overlay.tsx` — sentinel pattern to mirror for the inspector.
- `kanban-app/ui/src/components/focus-scope.tsx` / `focus-zone.tsx` — confirm zone vs scope semantics for an entity-keyed wrap that contains nested zones.
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `buildSnapshotForFocused`, `enumerateScopesInLayer`, `layerFqOf`.
- `swissarmyhammer-focus/src/snapshot.rs`, `swissarmyhammer-focus/src/state.rs` — layer-scoped snapshot contract.
- `kanban-app/ui/src/components/inspectors-container.guards.node.test.ts` — current source-level guards (note: the "permits a single FocusScope" guard may need to relax to `FocusZone` once the wrap is corrected).

### Approach

1. Reproduce the bug with a Playwright/Vitest browser test that opens an inspector panel, asserts the focused FQM is under `/window/inspector`, then presses ArrowLeft/ArrowDown and asserts focus stays under the inspector layer (no `/window/board/...` FQM).
2. Confirm via the focus-debug overlay (toggle in dev) which layer actually owns focus immediately after opening a panel — that diagnostic alone tells you whether the bug is "claim never lands" or "claim lands but is later overridden".
3. Fix root cause(s):
   - Swap the entity wrap from `<FocusScope>` to `<FocusZone>` (matching the file's own doc comment) so nested field zones don't violate the leaf invariant.
   - Add a sentinel `<FocusScope>` inside the inspector `<FocusLayer>` registering an `app.dismiss` shadow that dispatches `ui.inspector.close` (mirror jump-to-overlay.tsx:242-275).
   - Make `useFirstFieldFocus` defer the `setFocus` claim until the first field zone has registered with the layer's registry (e.g. via a microtask / layout-effect that polls `registry.has(firstFieldFq)`, or via a registration-event hook on `LayerScopeRegistry`).
4. Update the `inspectors-container.guards.node.test.ts` source-level guards to match the corrected wrap shape (and pin a sentinel-scope guard so the dismiss path can't regress silently).

## Acceptance Criteria

- [ ] With at least one inspector panel open, `spatial.focusedFq()` returns an FQM whose path is under `/window/inspector` immediately after the panel mounts (verified by an automated test, NOT by manual inspection).
- [ ] Pressing the Jump-To key (`s` in vim, `Mod+G` in cua/emacs) while a panel is open opens `<JumpToOverlay>` and the enumerated targets all carry FQMs under `/window/inspector` — NOT under `/window/board/*`. Asserted in a browser test against `data-testid="jump-to-overlay"` and the rendered pill scope FQMs.
- [ ] Pressing ArrowLeft / ArrowRight / ArrowUp / ArrowDown inside an inspector field never moves focus to a scope under `/window/board` while the inspector is open. Asserted in a browser test that drives the keyboard and reads `spatial.focusedFq()` after each key.
- [ ] Pressing Escape inside an inspector field dismisses the topmost panel via `ui.inspector.close`, with no intermediate focus moves to base-layer scopes. Asserted in a browser test (extend the existing `inspector.close-restores-focus.browser.test.tsx` patterns).
- [ ] No `scope-not-leaf` warning is emitted for the inspector entity wrap during a panel-open / panel-close cycle (asserted by a test that captures `console.error`/`console.warn` and grep for the literal token).

## Tests

- [ ] New browser test `kanban-app/ui/src/components/inspectors-container.layer-containment.browser.test.tsx` that:
  - opens an inspector panel,
  - asserts the focused FQM starts under `/window/inspector`,
  - drives Arrow keys and asserts focus stays under `/window/inspector`,
  - opens Jump-To and asserts every rendered pill's FQM is under `/window/inspector`,
  - presses Escape and asserts the panel closed and focus restored to the prior board scope.
- [ ] New unit / source-level guard in `inspectors-container.guards.node.test.ts` pinning that the inspector layer wraps its body in a sentinel `<FocusScope>` whose `commands` array contains an `app.dismiss` entry (so a future refactor can't silently delete the dismiss shadow).
- [ ] Existing `inspectors-container.test.tsx`, `inspector.close-restores-focus.browser.test.tsx`, and `spatial-nav-jump-to.spatial.test.tsx` continue to pass (regression coverage).
- [ ] Run `cd kanban-app/ui && npm test -- inspectors-container && npm test -- jump-to` and confirm green.

## Workflow

- Use `/tdd` — write the failing layer-containment browser test first (it should fail today by showing focus on `/window/board/...`), then implement the fix(es) until it passes.
- Resist the urge to fix all three symptoms with three independent patches without first verifying which root cause the failing test pins. Investigate via the focus-debug overlay before patching.