---
assignees:
- claude-code
position_column: todo
position_ordinal: a480
project: spatial-nav
title: Inspector field Up/Down returns None — pin which silent-None path fires and fix
---
## What

Pressing **ArrowUp** or **ArrowDown** with focus on an inspector field zone produces a `None` from the kernel — focus stays put and the user reports "no navigation." This is a concrete instance of the silent-None pathology pinned by `01KQAW97R9XTCNR1PJAWYSKBC7` (eliminate `Option<Moniker>`). That broader architectural fix flips the contract so silence is impossible AND torn-state surfaces in tracing — but it doesn't tell us *why* the inspector field-to-field path is producing None today.

This ticket is the focused diagnose-and-fix: identify which kernel path is producing `None` for inspector field nav, and fix it in production. The fix is the same surface that `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (inspector field navigation end-to-end) covers, but here we're targeting the specific kernel-or-registration root cause first so vertical nav works in production immediately.

## Production tree as registered

Per `kanban-app/ui/src/components/inspectors-container.tsx:172` and `entity-inspector.tsx:325`:

```
inspector layer (FocusLayer name="inspector", parent = window layer)
└── panel zone (FocusZone moniker="panel:task:T1", parent_zone = None)
    └── ... InspectorFocusBridge → EntityInspector → SectionBlock divs ...
        ├── field zone 1 (FocusZone moniker="field:task:T1.title", parent_zone = ?)
        ├── field zone 2 (FocusZone moniker="field:task:T1.status", parent_zone = ?)
        └── field zone 3 (FocusZone moniker="field:task:T1.assignees", parent_zone = ?)
```

Field zones' `parent_zone` is determined by `useParentZoneKey()` — they should all see the panel zone as their parent because no intermediate `<FocusZone>` exists between them. If that's true, beam-search iter 0 (same-kind peers sharing parent) should pick the next field zone by rect.

## Candidate seams (to investigate, not assume)

Per the silent-None enumeration we walked through, six paths in `BeamNavStrategy::next` can produce None. For inspector field nav specifically, the candidates are:

1. **Iter 0 finds nothing AND escalation hits the layer root → None.** If field zones have `parent_zone = None` (somehow not seeing the panel), then iter 0 matches no peers (no other field at layer root in this layer), and `parent_zone_in_same_layer?` returns None at the escalation step. This is the most likely path if a registration bug has field zones at the layer root.

2. **Iter 0 finds nothing AND escalation succeeds, iter 1 finds nothing, drill-out returns the parent moniker (NOT None).** This case actually doesn't return None — it returns the panel zone's moniker. So if the user gets None, this isn't it.

3. **Stale rects on every field zone → all rejected by beam.** If the inspector body has scrolled OR if `getBoundingClientRect()` returned zeros at registration time, every candidate's rect fails the in-beam test for "below the focused rect." iter 0 returns None, escalation succeeds (panel exists), iter 1 has no sibling panels, so cardinal_cascade returns the panel moniker — NOT None. Same as #2.

4. **All field zones share the same y-coordinate due to stale rects.** Beam search rejects every candidate that isn't "below" — if every rect has the same `top`, no candidate passes. iter 0 misses, iter 1 has no sibling panels, cascade returns panel moniker.

So if the user is genuinely seeing `None` (not the panel moniker), the cause is most likely **#1 — field zones have the wrong `parent_zone`**, OR there's something more subtle (e.g., field zones registered in a different layer than the panel).

If the user is actually seeing "focus moves to the panel zone but the indicator paints somewhere unexpected," that's #2/#3/#4 — the kernel returned the panel moniker, not None, but the visual feedback looks like None.

## Approach

### 1. Add tracing to the kernel cascade

Per `01KQAW97R9XTCNR1PJAWYSKBC7`'s "torn state → trace" principle, add `tracing::error!` on every silent-None path in `cardinal_cascade`. Even if the architectural ticket lands later, this small addition gets us observability immediately.

`swissarmyhammer-focus/src/navigate.rs`:

```rust
// After iter 0 misses and escalation fails (focused at layer root with no peers)
let parent = match parent_zone_in_same_layer(reg, focused) {
    Some(p) => p,
    None => {
        tracing::warn!(
            focused = %focused.key(),
            moniker = %focused.moniker(),
            ?direction,
            "cardinal_cascade: no in-beam peer and no parent zone in same layer; returning None"
        );
        return None;
    }
};
```

Distinguish "layer-root well-formed" from "torn parent reference" — the well-formed case can be `tracing::debug!`, the torn case should be `tracing::error!`. Match the contract laid out in `01KQAW97R9XTCNR1PJAWYSKBC7`.

### 2. Inspector-tree integration test

`swissarmyhammer-focus/tests/inspector_field_nav.rs` (new file) — build a realistic fixture mirroring the production tree:

- Window layer at root.
- Inspector layer (parent = window).
- Panel zone (parent_zone = None, in inspector layer).
- Three field zones (parent_zone = panel, in inspector layer) at vertically-progressing rects.

Call `BeamNavStrategy.next(field_2_key, Direction::Down)`. Expect `field_3.moniker`. Symmetric for ArrowUp.

If this test **passes**, the kernel is correct and the bug is in production registration — proceed to step 3.

If this test **fails**, the kernel cascade itself has a bug for this realistic shape — fix it in `navigate.rs`.

### 3. Frontend snapshot of production registry state

`kanban-app/ui/src/components/entity-inspector.field-up-down.diagnostic.browser.test.tsx` (new file) — mount the inspector for a task in the production provider stack against the per-test backend. After mount and one tick, snapshot the kernel-stored registry:

- For every field zone in the rendered inspector, log: SpatialKey, moniker, parent_zone (None or some key), layer_key, rect.
- Assert all field zones share the same `parent_zone` AND that `parent_zone` resolves to the panel zone.
- Assert all field zones share the same `layer_key` AND it matches the inspector layer's key.
- Assert no field zone has a rect with `width === 0` or `height === 0`.

If any of these assertions fail, that's the bug — fix the registration site.

If all pass, drive the actual nav: focus field zone 1, dispatch ArrowDown, log the kernel's response (None vs moniker), assert it returns field zone 2's moniker.

### 4. Fix at the failing seam

Don't pre-emptively patch. The diagnostic tests dictate the fix.

Likely fixes if registration is the culprit:
- **Field zones registered before panel zone** (race) — if the field zone's register effect fires before the panel zone's, the field's `useParentZoneKey()` might read `null` from the React context. Fix: ensure the panel zone wraps the field rendering tree by the time field zones mount. (Probably already correct via React parent-before-child mount order — but verify.)
- **Multiple `<FocusZone>` ancestors between panel and fields** — if some intermediate component introduced a `<FocusZone>` (e.g., a section wrap), the field's `parent_zone` would point at that intermediate, not the panel. Fix: remove the intermediate or accept it (then iter 0 finds same-section peers, escalation walks up to panel — still works).

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [ ] **Kernel test passes**: `BeamNavStrategy::next(field_n_key, Direction::Down)` against a realistic fixture (three field zones siblings under a panel zone) returns `field_(n+1).moniker`. Symmetric for ArrowUp.
- [ ] **Production registry diagnostic test passes**: every field zone in a rendered inspector reports the same `parent_zone` (the panel zone's key), the same `layer_key` (inspector layer), and a non-zero rect.
- [ ] **Production nav test passes**: in a mounted inspector, focusing field zone 1 and dispatching `nav.down` produces a focus change to field zone 2 (`useFocusedScope()` reports `field:task:T1.<next>`).
- [ ] If any of the above fail, the fix targets the failing seam — the test that fails before the fix passes after.
- [ ] Tracing emits a `WARN` or `ERROR` (per `01KQAW97R9XTCNR1PJAWYSKBC7`'s contract) on the path that previously returned None silently. Visible in `tauri dev` logs.
- [ ] No regression on inspector horizontal nav (within a field, between pills) — covered by `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`.

## Tests

All tests are automated. No manual verification.

### Rust kernel — `swissarmyhammer-focus/tests/inspector_field_nav.rs` (new file)

Reuses the realistic-fixture builder from `01KQ7STZN3G5N2WB3FF4PM4DKX` under `swissarmyhammer-focus/tests/fixtures/`.

- [ ] `down_from_field_1_lands_on_field_2_in_inspector_panel` — three field zones siblings under a panel zone in the inspector layer; `next(field_1, Direction::Down)` returns `field_2.moniker`.
- [ ] `down_from_field_2_lands_on_field_3` — same fixture; `next(field_2, Direction::Down)` returns `field_3.moniker`.
- [ ] `up_from_field_2_lands_on_field_1` — symmetric.
- [ ] `down_from_last_field_returns_panel_moniker_via_drill_out` — `next(field_3, Direction::Down)`: iter 0 misses (no field below), iter 1 has no sibling panels, drill-out returns panel zone moniker (NOT None). Pins that the cascade does NOT fall through to None for this shape.

Test command: `cargo test -p swissarmyhammer-focus --test inspector_field_nav` — all four pass.

### Frontend — `kanban-app/ui/src/components/entity-inspector.field-up-down.diagnostic.browser.test.tsx` (new file)

- [ ] `inspector_field_zones_share_panel_as_parent_zone` — open inspector for a task, snapshot every `field:task:<id>.<name>` zone's `parent_zone` via the spatial actions debug API, assert all share the panel zone's SpatialKey.
- [ ] `inspector_field_zones_share_inspector_layer_key` — same shape; assert all share the inspector layer's `LayerKey`.
- [ ] `inspector_field_zones_have_non_zero_rects` — assert every field zone's rect has `width > 0 && height > 0`.
- [ ] `down_from_focused_field_in_production_inspector_lands_on_next_field` — focus first field via `spatial_focus`, dispatch `keydown { key: "ArrowDown" }`, assert `useFocusedScope()` reports the next field's moniker.

Test command: `bun run test:browser entity-inspector.field-up-down.diagnostic.browser.test.tsx` — all four pass.

### Coordination

- [ ] If the kernel test passes but the frontend test fails, the registration site has the bug — fix in `kanban-app/ui/src/components/fields/field.tsx`, `inspectors-container.tsx`, or wherever field zones are mounted.
- [ ] If the kernel test fails, the cascade has the bug — fix in `swissarmyhammer-focus/src/navigate.rs`.

## Workflow

- Use `/tdd` — write the kernel test first, watch it pass or fail. Then write the diagnostic browser test, watch it pass or fail. The pattern of pass/fail tells you which seam to fix.
- Single ticket — one diagnose-and-fix concern, with the diagnostic split between kernel and frontend so the implementer can localize the bug fast.
- Coordinates with `01KQAW97R9XTCNR1PJAWYSKBC7` (the architectural elimination of silent None) and `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` (the broader inspector field interaction surface). This ticket can land first because it's focused; the architectural and surface tickets land afterward without conflict.
- Do not duplicate the work in `01KQ9ZJHRXCY8Z5YT6RF4SG6EK` — that ticket pins icon-in-zone, vertical nav, and Enter drill-in as a single surface concern. This ticket is purely the kernel-level diagnostic for why vertical nav specifically returns None today.
