---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8780
project: spatial-nav
title: 'Spatial nav: prove layer isolation end-to-end with multi-inspector tests (or find the leak)'
---
## Status — AUDIT CONFIRMED

Layer isolation is verified at the spatial-state machine level. Three-layer isolation and parent-scope-across-layer edge cases both pass; all new Rust unit tests and both new parity cases are green in both Rust and JS runs. The browser test for the 3-open-inspectors topology is written and in place, but cannot currently execute because the working tree has an unrelated WIP refactor in `entity-focus-context.tsx` (renames `registerClaim` → `registerSpatialKey`) that was never propagated to `focus-scope.tsx`. This break affects ALL browser spatial-nav tests, not just the new one — see new task `01KPTHCVBS5E7CAH4JXBAR3EWP` which captures that separate issue. Once that card is done the multi-inspector browser test will run automatically.

**Next diagnostic step for the "some fields work, some don't" symptom**: see task `01KPS22R2T4Q5QT9A71E7ZWAAP`. The layer filter is proven; the symptom is elsewhere (candidates: inspector entity scope `spatial={true}` shadowing fields — now confirmed still present, ResizeObserver miss on dynamic field editors, portaled FocusScopes in specific field types).

---

## What

The user reports inspector nav broken in a way that's "hard to describe — working for some fields and not others," with the hunch that Rust layer isolation isn't properly implemented, or the inspector container is missing a layer wrapper. A deep audit of both suggests the code is **structurally correct**, but the test coverage doesn't prove it for the real-world "multiple inspectors open" case. Close that gap — either the tests prove isolation works and we can redirect the diagnosis elsewhere, or the tests fail and we've found the bug.

### Audit result — what we already know

#### ✅ Rust layer isolation is implemented correctly on every nav path

Concrete evidence in `swissarmyhammer-spatial-nav/src/spatial_state.rs`:

- `spatial_search()` — filters candidates: `active_layer_key.as_deref().is_none_or(|lk| e.layer_key == lk)`. Called by `navigate()`.
- `fallback_to_first()` — `find_top_left(|e| e.layer_key == active_layer_key)`. Null/stale recovery respects the active layer.
- `apply_override()` — override lookup filters by `entry.layer_key == active_layer_key`.
- `save_focus_memory()` — per-layer `last_focused`, stored in `LayerEntry` by layer key.
- `parent_scope` container-first search in `spatial_nav.rs` — receives **pre-filtered** candidates, so the walk is already bounded by active layer.
- Existing test `navigate_only_sees_active_layer` — 2 layers, confirms no leak on `Right`/`Down`.
- JS shim mirrors identically at `kanban-app/ui/src/test/spatial-shim.ts`.

#### ✅ InspectorsContainer's layer structure is also correct

`kanban-app/ui/src/components/inspector-focus-bridge.tsx:109` — every `InspectorFocusBridge` wraps its content in `<FocusLayer name="inspector">`. With 3 inspectors open, the layer stack is `[window, inspector-L1, inspector-L2, inspector-L3(active)]` — each with a unique ULID key. The Rust filter means only L3's scopes are navigable.

## Acceptance Criteria

- [x] Rust unit test `navigate_with_three_layers_only_sees_topmost` added and passing
- [x] Rust unit test for `parent_scope` crossing a layer boundary added and passing (`parent_scope_in_lower_layer_is_ignored`)
- [x] Parity cases for multi-inspector and overlapping-rect scenarios added to `spatial-parity-cases.json` and green in both Rust and JS parity runs
- [x] Browser test for multi-inspector isolation added (`spatial-nav-multi-inspector.test.tsx` + `spatial-multi-inspector-fixture.tsx`). Execution blocked on unrelated pre-existing refactor gap (new task `01KPTHCVBS5E7CAH4JXBAR3EWP`).
- [x] All Rust + parity tests pass → layer isolation verified at the state-machine level.
- [x] Task description updated with "layer isolation verified — see task `01KPS22R2T4Q5QT9A71E7ZWAAP` for next diagnostic step"

## Tests

- [x] `cargo test -p swissarmyhammer-spatial-nav` — 68 unit tests + parity test + doc tests all green (2 new unit tests added; parity test iterates 27 cases including the 2 new ones)
- [x] `npx vitest run spatial-shim-parity` — 27 parity cases green (2 new + 25 existing)
- [ ] `npx vitest run spatial-nav-multi-inspector` — blocked on `01KPTHCVBS5E7CAH4JXBAR3EWP`; test file and fixture are in place and structurally correct

## Files modified

- `swissarmyhammer-spatial-nav/src/spatial_state.rs` — added 2 unit tests (`navigate_with_three_layers_only_sees_topmost`, `parent_scope_in_lower_layer_is_ignored`)
- `kanban-app/ui/src/test/spatial-parity-cases.json` — added 2 parity cases (multi-inspector three-layer and identical-rects-different-layers)
- `kanban-app/ui/src/test/spatial-multi-inspector-fixture.tsx` — new fixture with 3 stacked `FocusLayer` inspectors
- `kanban-app/ui/src/test/spatial-nav-multi-inspector.test.tsx` — new browser test covering j/k/h/l isolation, clamp behavior, active-layer contract, and candidate-pool verification

## Relationship to other tasks

- `01KPS22R2T4Q5QT9A71E7ZWAAP` — the "inspector-from-grid broken" symptom. This audit confirms layer isolation is not the cause; that task should investigate the remaining candidates (inspector entity scope, per-field-type FocusScopes, ResizeObserver).
- `01KPTHCVBS5E7CAH4JXBAR3EWP` — must be completed before the new browser test can execute.