---
assignees:
- claude-code
depends_on:
- 01KQZ7WR2SYN4W9DSF9JKH6FQ3
position_column: todo
position_ordinal: f480
project: spatial-nav
title: 'stateless: card 4 — migrate React to spatial_decide(snapshot) and retire per-op IPCs from the call sites'
---
## Why this is card 4

Cards 1–3 leave the new `decide()` kernel sitting alongside the old stateful `SpatialRegistry` — both compile, no consumer migrated. This card flips the React side over to the new path: every nav keystroke builds a `NavSnapshot` from the React-owned `LayerScopeRegistry`, calls a single `spatial_decide(window, op, snapshot)` Tauri command, and applies the returned `FocusState` / `FocusChangedEvent`. Old per-op IPCs stay defined during this card so a revert is a one-commit rollback; their deletion happens in card 5.

This is a wire-shape migration, not a behavior change. Every motion-validation suite (the eight `nav.<op>` cards) and the umbrella `spatial-nav-end-to-end.spatial.test.tsx` must stay green throughout.

## What to build

### 1. `spatial_decide` Tauri command

`kanban-app/src/commands.rs` — add at the end of the spatial block (after the `spatial_drill_out` definition around line 2693):

```rust
#[tauri::command]
pub async fn spatial_decide(
    window: Window,
    op: FocusOp,
    snapshot: NavSnapshot,
    state: State<'_, AppState>,
) -> Result<FocusDecisionEvent, String> { ... }
```

The body locks the same `AppState` mutex the legacy commands use, reads `&FocusState` from a new field `app_state.focus_state: tokio::sync::Mutex<FocusState>`, calls `swissarmyhammer_focus::stateless::decide(&state, &op, &snapshot, &window_label)`, writes `decision.next` back into the mutex, and emits `decision.event` if `Some`. Single command, no per-op handlers.

Register in `kanban-app/src/main.rs` immediately after the existing spatial registrations (after line 86), keeping legacy commands present:

```rust
commands::spatial_decide,
```

### 2. Build `NavSnapshot` from `LayerScopeRegistry`

`kanban-app/ui/src/lib/spatial-focus-context.tsx` — add `buildNavSnapshot(layerFq: FullyQualifiedMoniker): NavSnapshot` that reads from `LayerScopeRegistry` (the parallel session's React-side registry, already wired in `kanban-app/ui/src/lib/layer-scope-registry-context.tsx`). The shape mirrors the Rust types committed by card 2:

```ts
{
  layer_fq: FullyQualifiedMoniker;
  scopes: Array<{ fq, rect, parent_zone, overrides }>;
}
```

This helper is the single source of snapshot construction — every nav action funnels through it.

### 3. Replace per-op closures with single dispatch

Three call sites change:

- **`kanban-app/ui/src/components/app-shell.tsx::buildNavCommands`** (lines 288–299) — replace `actions.navigate(focusedFq, spec.direction)` with `actions.decide({ Cardinal: { dir: spec.direction } })` (or `EdgeFirst` / `EdgeLast` for `first` / `last`).
- **`kanban-app/ui/src/components/app-shell.tsx::buildDrillCommands`** (lines 344–389) — replace `actions.drillIn(...)` and `actions.drillOut(...)` with `actions.decide({ DrillIn })` / `actions.decide({ DrillOut })`. The fall-through to `app.dismiss` when the kernel echoes the focused FQM stays intact (the closure compares the returned FQM after `decide` resolves).
- **`kanban-app/ui/src/lib/spatial-focus-context.tsx`** — add `actions.decide(op: FocusOp): Promise<FullyQualifiedMoniker>` that calls `buildNavSnapshot(currentLayerFq())`, awaits `invoke("spatial_decide", { window, op, snapshot })`, applies the returned `FocusState` to local React state, and resolves to the new focused FQM. Existing per-op methods (`navigate`, `drillIn`, `drillOut`, `focus`) become thin shims that call `decide` so any straggler call sites keep working — those shims get removed in card 5.

### 4. `Click`, `FocusLost`, `ClearFocus`, `PushLayer`, `PopLayer`

The same migration extends to the non-keyboard ops. Each existing dispatch site routes through `actions.decide` with the matching `FocusOp` variant:

- `actions.focus(fq)` → `decide({ Click: { fq } })`
- focused-scope unmount detection (already wired by card `01KQYWM5BHFRPCRD70GF8YRCGY`'s sibling `01KQW6JF6P7QHXFARAR5RTZVX4`) → `decide({ FocusLost: { lost, lost_parent_zone, lost_layer } })`
- `actions.clearFocus()` → `decide({ ClearFocus })`
- focus-layer mount/unmount in `kanban-app/ui/src/components/focus-layer.tsx` → `decide({ PushLayer { fq, allow_pierce_below } })` / `decide({ PopLayer { fq } })`

### 5. Test invariants

- All eight motion-validation suites (`spatial-nav-{up,down,left,right,first,last,drillin,drillout}.spatial.test.tsx`) green against the new path.
- `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` green — the harness's `mockInvoke` will see `spatial_decide` calls; update `kanban-app/ui/src/test/spatial-shadow-registry.ts` to handle `spatial_decide` by running the same Rust kernel under a real `decide()` call (or routing to a TS port if no test-side Rust is available; document the choice).
- New Rust integration test `kanban-app/tests/spatial_decide_integration.rs`: mount the command, dispatch each `FocusOp` variant, assert the returned `FocusDecisionEvent` matches the expected next FQM.

## Out of scope

- Deleting any of the old per-op IPCs (`spatial_navigate`, `spatial_drill_in`, etc.) — that's card 5.
- Deleting `SpatialRegistry`, `BeamNavStrategy`, or the `state.rs::SpatialState` mutex — card 5.
- Removing the per-op shim methods on `actions` (`actions.navigate`, etc.) — card 5.
- Changing kernel algorithms — card 1 owns the in-band fix and decide() owns it on the new path.

## Acceptance Criteria

- [ ] `spatial_decide` Tauri command exists in `kanban-app/src/commands.rs` and is registered in `kanban-app/src/main.rs`.
- [ ] `actions.decide(op)` is the single execution path used by `buildNavCommands`, `buildDrillCommands`, focus-layer mount/unmount, focused-scope unmount detection, `actions.focus`, and `actions.clearFocus`.
- [ ] `buildNavSnapshot` reads exclusively from `LayerScopeRegistry` (no reads from the Rust `SpatialRegistry`).
- [ ] All eight motion-validation suites pass against `spatial_decide` (each test asserts the IPC name has switched from per-op to `spatial_decide` with the expected `FocusOp` variant).
- [ ] `spatial-nav-end-to-end.spatial.test.tsx` Family 2, 3, 4, and 7 still pass with the new dispatch path.
- [ ] Old per-op IPCs and `actions.navigate / drillIn / drillOut / focus / clearFocus` shims still exist, still compile, but are unused by production call sites (asserted by an `eslint --rule no-restricted-syntax` rule that flags new `actions.navigate(` / `actions.drillIn(` / `actions.drillOut(` calls — or a grep-based test).
- [ ] `cargo nextest run -p swissarmyhammer-focus -p kanban-app` green; `cd kanban-app/ui && bun test` green.

## Tests

- [ ] New: `kanban-app/tests/spatial_decide_integration.rs` — exercises every `FocusOp` variant end-to-end through the registered Tauri command.
- [ ] Update: `kanban-app/ui/src/test/spatial-shadow-registry.ts` — handle `spatial_decide` invocations.
- [ ] Update each motion-validation suite's IPC assertion to expect `spatial_decide` with the matching `FocusOp` variant.
- [ ] Update `spatial-nav-end-to-end.spatial.test.tsx` — Families 2 and 3 currently assert `spatialNavigateCalls` / `spatialDrillCalls`; rename or extend the helpers to match `spatial_decide` shape.
- [ ] Test command: `cargo nextest run -p kanban-app spatial_decide_integration && cd kanban-app/ui && bun test` — all green.

## Workflow

- Use `/tdd` — write `spatial_decide_integration.rs` first against the new command signature; let it fail; implement `spatial_decide`; migrate the React call sites; re-run the eight motion-validation suites and the end-to-end test until green.

#stateless-rebuild