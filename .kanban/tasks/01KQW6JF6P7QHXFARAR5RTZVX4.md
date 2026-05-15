---
assignees:
- wballard
depends_on:
- 01KQW6H3397154YJWDPD6TDYZ3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffaf80
project: spatial-nav
title: 'spatial-nav redesign step 12: cutover (3/4) — shrink SpatialRegistry, delete scopes map and per-scope last_focused'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**. Third of four cutover steps.

## Goal

Delete the kernel-side scope registry and everything that depended on it. After this step, `SpatialRegistry` only holds layers, last_focused_by_fq, and focus_by_window. The `Option<&NavSnapshot>` parameter on pathfinding/fallback/record_focus becomes a required `&NavSnapshot`.

## What to delete

### Fields and methods on SpatialRegistry

`swissarmyhammer-focus/src/registry.rs`:

- Delete field: `scopes: HashMap<FullyQualifiedMoniker, FocusScope>`
- Delete struct: `FocusScope` (the kernel-side per-scope record — not to be confused with the React `<FocusScope>` component, which keeps its name)
- Delete methods: `register_scope`, `unregister_scope`, `update_rect`, `find_by_fq`, `check_overlap_warning`
- Delete fields: `validated_layers`, `overlap_warn_partner`
- The `last_focused: Option<FullyQualifiedMoniker>` field that was on the per-scope record dies with the struct

`last_focused_by_fq` (added in step 5) becomes the sole storage. Remove the dual-write code from `record_focus`; only the map is written. Remove the per-scope-fallback read code from `resolve_fallback`; only the map is read.

### State methods

`swissarmyhammer-focus/src/state.rs`:

- Delete `state.handle_unregister` (replaced by `focus_lost` in step 8)
- Delete `state.resolve_fallback` registry-path branch
- Make `Option<&NavSnapshot>` parameters required `&NavSnapshot` everywhere (`focus`, `navigate`, `record_focus`, `resolve_fallback`)

### Pathfinding

`swissarmyhammer-focus/src/navigate.rs`:

- Delete the `NavScopeView` impl for `&SpatialRegistry` (registry no longer has a scopes map to iterate)
- Keep the trait + the snapshot impl
- Or simpler: remove the trait entirely now that there's only one impl, inline `IndexedSnapshot` access into pathfinding

Recommended: drop the trait. Replace with direct `&IndexedSnapshot` arguments. Cleanest result.

## What survives

- `layers: HashMap<FQM, FocusLayer>`
- `last_focused_by_fq: HashMap<FQM, FQM>`  ← was added in step 5, now sole truth
- `focus_by_window: HashMap<WindowLabel, FQM>` (lives on `SpatialState`)
- Pathfinding (`geometric_pick`, `BeamNavStrategy`) — takes `&IndexedSnapshot`
- `resolve_fallback` — takes `&IndexedSnapshot`
- `record_focus` — takes `&IndexedSnapshot`
- `state.focus`, `state.navigate`, `state.focus_lost`, `state.clear_focus`
- Layer ops: `push_layer`, `pop_layer`, `remove_layer`

## Tests

- Every kernel test that previously built scopes via `registry.register_scope(...)` is rewritten to build a `NavSnapshot` directly and pass it to the kernel call. (Many tests will need touch-up; this is expected.)
- The full `cargo test -p swissarmyhammer-focus` suite passes against the slimmed kernel.
- e2e tests on the React side stay green — they were already exercising the snapshot path.

## Out of scope

- Moving overlap warning to JS (step 13)

## Acceptance criteria

- `SpatialRegistry` is significantly smaller; no `scopes` field, no per-scope `last_focused`
- All snapshot parameters that were `Option<&NavSnapshot>` are now required `&NavSnapshot`
- `cargo test -p swissarmyhammer-focus` green
- `pnpm -C kanban-app/ui test` green

## Files

- `swissarmyhammer-focus/src/registry.rs` — major shrink
- `swissarmyhammer-focus/src/state.rs` — delete handle_unregister, tighten signatures
- `swissarmyhammer-focus/src/navigate.rs` — drop trait, take `&IndexedSnapshot`
- `swissarmyhammer-focus/tests/*` — rewrite scope-building helpers to use snapshots #stateless-nav

## Review Findings (2026-05-07 15:44)

### Warnings

- [x] `swissarmyhammer-focus/src/snapshot.rs:7-18` — Module-level doc comment narrates the redesign as a multi-step plan ("Step 2 of the spatial-nav redesign described in card `01KQTC1VNQM9KC90S65P7QX9N1`. … this module has no production callers in step 2. That is intentional: steps 3–5 will adapt `geometric_pick`, `resolve_fallback`, and `record_focus` to take a snapshot argument, and steps 6–8 will plumb the IPC."). This violates the doc-comments rule on three axes: it references task/step IDs, it tells point-in-time history that belongs in a commit message, and after this cutover its central factual claim ("no production callers") is **wrong** — every focus-mutating IPC now ships a snapshot. Suggested fix: rewrite to describe the current state of the module ("Per-decision navigation snapshot — the wire shape every focus-mutating IPC carries; the kernel reads scope geometry out of the snapshot at decision time and holds no replica between calls.") and drop the staged-cutover narrative.
- [x] `swissarmyhammer-focus/src/snapshot.rs:63, 97` — Per-type doc comments on `SnapshotScope` and `NavSnapshot` say they "Mirror the TypeScript `SnapshotScope` introduced by step 1 of the parent card" / "introduced by step 1". Same doc-rot pattern — drop the step reference. The "mirrors the TypeScript wire shape" half of the sentence is the durable contract; the "introduced by step 1" half rots.
- [x] `swissarmyhammer-focus/src/snapshot.rs:427, 558` — Test doc comments mention "the TypeScript mirror in step 1" and "Step 3+ callers". Same fix — describe what the test pins, not what release brought it in.
- [x] `swissarmyhammer-focus/src/types.rs:25-31` — Module-level doc comment cites kanban task `01KQD6064G1C1RAXDFPJVT1F46` and walks through the old `find_by_moniker` lookup-collision history as design rationale. The point-in-time pointer ("see the parent path-monikers card") and the historical narrative both rot in source. The structural rationale ("FQMs eliminate the collision by construction") can stand on its own without the historical tour. Suggested fix: keep the one-sentence FQM rationale, drop the kanban-id reference and the "with flat `Moniker`s the inspector's `field:T1.title` zone collided…" archaeology.
- [x] `ARCHITECTURE.md:27` — Describes `swissarmyhammer-focus` as owning "the registry of focusable scopes, and the pluggable extension traits (`NavStrategy`, `FocusEventSink`)". After this cutover both halves are wrong: there is no kernel-side scope registry (it lives on the React side as `LayerScopeRegistry`), and `NavStrategy` was deleted along with `BeamNavStrategy`. The cutover is what makes the doc-rot visible. Update the sentence to reflect the snapshot-driven kernel — "owns the focus state machine (per-window focus, layer forest, `last_focused_by_fq` memory) and the snapshot-driven pathfinder; scope geometry lives in React and rides on every IPC as a `NavSnapshot`. The only pluggable extension trait that survives is `FocusEventSink` for adapter-side event delivery."

### Nits

- [x] `kanban-app/src/commands.rs:2259, 2329, 2460, 2486` — Four Tauri command wrappers (`spatial_focus`, `spatial_navigate`, `spatial_drill_in`, `spatial_drill_out`) take `snapshot: Option<NavSnapshot>` and silently `return Ok(())` (or `Ok(focused_fq)` for the drill commands) on `None`. The kernel-level APIs are correctly tightened to required `&NavSnapshot` per the acceptance criterion — this is wire-boundary defensiveness against React-side unmount races, and the doc comments justify it. But the silent drop is invisible: a `tracing::debug!` (or `tracing::warn!`) on the `None`-arrival path would let operators see how often the race actually fires. `spatial_focus_lost` already takes a required `NavSnapshot`; the four `Option`-tolerant commands could either follow suit or at minimum log when they short-circuit.

## Review Findings (2026-05-07 16:30)

Second-pass review. The first review's six checkboxes were all addressed correctly — the targeted rewrites at the named line numbers are clean. However, the same doc-rot patterns survive at three additional sites in the same file the first review only sampled, and the rewritten ARCHITECTURE.md paragraph reintroduces one "used to" pattern. These are genuine doc-rot issues by the same rules the first review applied; flagging them so the file ends up clean rather than half-clean.

### Warnings

- [x] `swissarmyhammer-focus/src/snapshot.rs:256-260` — Test module-level doc comment still says "no production callers exist yet (steps 3–5 will introduce them) so the bar is 'the helpers behave as documented', not 'every kernel decision is covered'". After this cutover that factual claim is **wrong** — every focus-mutating IPC ships a snapshot through production callers — and the "(steps 3–5 will introduce them)" parenthetical is a step reference. Same pattern the first review flagged on the file's outer module doc; the implementer fixed the outer one and missed the inner one. Suggested fix: replace with a single sentence about what the tests cover (e.g. "Unit coverage for the snapshot data types and the [`IndexedSnapshot`] walks — round-trip serde, indexed FQM lookup, parent-zone chain walks (linear, missing-edge, cycle).") and drop the staged-cutover language.
- [x] `swissarmyhammer-focus/src/snapshot.rs:451-454` — `nav_snapshot_json_uses_snake_case_field_names` test doc says "the snake_case names the TypeScript step-1 builder produces". The step-1 reference is the same pattern the first review flagged on `nav_snapshot_round_trips_through_serde` (line 405) and `parent_zone_chain_empty_for_missing_fq` (line 542) and the implementer corrected on both — but missed this one. The "TypeScript builder" half is durable; "step-1" rots. Suggested fix: drop "step-1" — "the snake_case names the TypeScript builder produces".
- [x] `swissarmyhammer-focus/src/snapshot.rs:621-624` — `indexed_snapshot_handles_empty_scopes` test doc says "pins the contract so step-3 callers don't have to special-case empty input". Same step-reference doc-rot. Suggested fix: drop "step-3" — "pins the contract so callers don't have to special-case empty input".
- [x] `ARCHITECTURE.md:27` — The rewritten paragraph reads cleanly for the first half but ends with "The path-monikers identity model is what eliminates the duplicate-registration ambiguity that flat string monikers used to surface as 'nav crosses layers'". The phrase "used to surface" is the explicit "used to" doc-rot pattern the doc-comments rule names. The structural rationale ("FQMs eliminate the duplicate-registration ambiguity by construction") stands on its own without the point-in-time pointer to the old failure mode. Suggested fix: drop the "used to surface as 'nav crosses layers'" tail — "The path-monikers identity model eliminates the duplicate-registration ambiguity that a flat string moniker would otherwise admit." Or simply drop the whole sentence; the FQM/SegmentMoniker pair is described two sentences earlier and the rationale doesn't add new information.