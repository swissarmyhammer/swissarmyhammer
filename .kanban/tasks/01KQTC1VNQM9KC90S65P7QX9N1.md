---
assignees:
- wballard
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffb180
project: spatial-nav
title: 'spatial-nav: redesign — React-side scope registry, kernel as stateless FSM over per-nav snapshots'
---
## Premise

The current spatial-nav design replicates React's scope tree into a Rust kernel via `spatial_register_scope` / `spatial_unregister_scope` / `spatial_update_rect`. The replica drifts under:

- Async IPC failures (silent `.catch(console.error)` in `focus-scope.tsx:389-394`)
- Async ordering races (concurrent `update_rect` from sibling reflow)
- Missing parent-cascade on unmount (kernel removes single FQ, not subtree)
- Drag-drop and animation (rects shift without scopes mounting/unmounting; ResizeObserver only fires on size, not position)
- Sibling-reflow staleness (a card moves because a sibling reordered — its size didn't change, ResizeObserver doesn't fire, kernel rect stays stale)

Symptom: `check_overlap_warning` logs `two entries share (x, y)` for two different live tasks' chips — one rect is correct, the other is a kernel ghost (cascade leak) or a sibling-reflow stale rect.

## ✅ Closeout (2026-05-07)

**The redesign is complete.** All 13 implementation steps shipped to `origin/kanban`:

| Step | Commit | What |
|---|---|---|
| 1 | `0fd47753d` | LayerScopeRegistry context (additive) |
| 2 | `635593eaf` | NavSnapshot/SnapshotScope/IndexedSnapshot Rust types |
| 3 | `18d30f60a` | NavScopeView trait, pathfinding parameterized |
| 4 | `e3a21de91` | resolve_fallback_with_snapshot + LostFocusContext |
| 5 | `ab0b996f5` | last_focused_by_fq map + dual-write |
| 6 | `83c9d8a1b` | spatial_navigate(snapshot) IPC + React handler |
| 7 | `0c0ba30e2` | spatial_focus(snapshot) + layer-pop round-trip |
| 8 | `0802461b5` | spatial_focus_lost + unmount detection (with `useLayoutEffect` ref-nullification fix) |
| 9 | `475379968` | Divergence harness consolidation + soak suite |
| 10 | `a58af37f3` | Cutover 1/4: drop continuous rect tracking |
| 11 | `a8a990e06` | Cutover 2/4: delete per-scope IPC umbilical |
| 12 | `efe45028b` | Cutover 3/4: shrink kernel — snapshot is sole source |
| 13 | `89f4d881c` | Cutover 4/4: JS dev-mode needless-nesting detector |

Cumulative diff: large net-negative line count. The cutover sequence (steps 10-13) alone deleted ~16,000+ Rust lines.

The original `kanban-app[49073]` overlap-warning symptom is fixed: drag-drop no longer triggers the warning because detection runs only on `LayerScopeRegistry.add` (mount), not on every `update_rect`. The structural-bug detection is preserved in its right home (React, dev-mode only) and dead-code-eliminated from production builds.

The kernel is now snapshot-driven with no scope replica:
- `SpatialRegistry` retains only `layers: HashMap<FQM, FocusLayer>` and `last_focused_by_fq: HashMap<FQM, FQM>`.
- `SpatialState` retains only `focus_by_window: HashMap<WindowLabel, FQM>`.
- All focus-mutating IPCs carry a fresh `NavSnapshot` built from the React-side `LayerScopeRegistry` at decision time.

**Recommended follow-ups (not part of this epic):**
1. Re-evaluate `01KQSF0VCEWW523VXCBTYX4W0B` (nav.left collapse to engine root) — likely fixed or cleanly diagnosable now.
2. Update `MEMORY.md` if any entries point to the old replicated-kernel architecture.
3. Archive `01KQZ9Q54HZ98K2T57DBBBPSSH` — moot since `tests/navigate.rs` was deleted in step 12.

---

## The redesign

Stop replicating. The Rust kernel becomes **stateless with respect to scope geometry and structure** — it holds only the focus state machine (current focus, layer stack, focus-restoration history) and the pathfinding + fallback algorithms. There is no scope registry in Rust. Two changes accomplish this:

### 1. Move the scope registry into React.

Each `<FocusLayer>` provides a `LayerScopeRegistry` via context. The registry is a `Map<FullyQualifiedMoniker, ScopeEntry>` held in a ref on the layer.

### 2. Build the kernel's view of the world per-decision.

Every kernel call that touches focus carries a fresh snapshot of the active layer. The kernel never reads scope state out-of-band, because it doesn't have any.

## Why this is the right answer

Drift is a property of replication. The kernel and React were two replicas of the same logical tree, kept in sync via async IPC. *Any* replication strategy under async IPC has failure modes — cascade leaks, ordering races, silent drops. Reconciliation/sync protocols add complexity to mitigate failure modes that wouldn't exist if there were nothing to replicate.

The kernel doesn't fundamentally need to know what scopes exist between decisions. It needs to know *at the moment of a decision*. So push that knowledge across exactly when it's needed. State that doesn't exist can't be stale.

Plus: virtualization caps snapshot size, so the cost argument against this design (which would have been real with thousands of off-screen scopes) doesn't apply here. The architecture matches the runtime characteristics. #stateless-nav