---
assignees:
- wballard
depends_on:
- 01KQW6JF6P7QHXFARAR5RTZVX4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff9c80
project: spatial-nav
title: 'spatial-nav redesign step 12.5: drill in / drill out IPCs must accept snapshot (gap from earlier plan)'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**. Inserted between step 12 and step 13 to fix a gap that wasn't covered by the original step plan.

## Why this exists

The original step plan (steps 6, 7, 8) added snapshot variants for `spatial_navigate`, `spatial_focus`, and `spatial_focus_lost`. **It missed `spatial_drill_in` and `spatial_drill_out`** — two separate Tauri IPCs (`kanban-app/src/commands.rs:2609, 2649`) that today delegate directly to `SpatialRegistry::drill_in` / `SpatialRegistry::drill_out`, which read `registry.scopes` and `registry.scopes[fq].last_focused`.

Step 12 deletes `registry.scopes` and the per-scope `last_focused` field. After step 12 lands, drill IPCs lose their backing source unless they have already been converted to the snapshot path.

**Before starting this task: check whether step 12 already addressed drill.** Step 12's task description was updated mid-flight to fold this work in. If `spatial_drill_in` / `spatial_drill_out` already accept a `snapshot: NavSnapshot` argument and the kernel-side drill methods read from snapshot + `last_focused_by_fq`, this task is a verification pass — confirm the acceptance criteria below all pass, then close. Otherwise, implement.

## What "drill" means in the kernel

- `drill_in(fq, focused_fq)` — pick the FQM to focus when the user drills into the scope at `fq`. Today reads:
  1. The "last focused descendant of `fq`" — was `registry.scopes[fq].last_focused`. Now reads `last_focused_by_fq.get(&fq)` (introduced in step 5, sole truth post-step-12).
  2. If no last_focused match: the topmost-then-leftmost child of `fq`. This is the same algorithm as `Direction::First` (already snapshot-aware after step 3 / step 6) — geometric pick over snapshot entries whose `parent_zone == fq`.

- `drill_out(fq, focused_fq)` — pick the FQM to focus when the user drills out of the scope at `fq`. Today reads `registry.scopes[fq].parent_zone`. After step 12 reads `snapshot.get(fq).parent_zone`.

Both are pure structural lookups on `(snapshot, last_focused_by_fq)`. Simpler than nav.

The no-silent-dropout contract is preserved: result equal to `focused_fq` means stay-put. Invariant unchanged.

The `first_matches_drill_in_first_child_fallback` test in `navigate.rs:1309` is the canonical regression that drill_in's cold-start fallback matches `Direction::First`. Both must use the same geometric algorithm against the same scope set. Update the test to pass a snapshot.

## What to build

### IPC signatures

`kanban-app/src/commands.rs` — both commands accept a `snapshot: NavSnapshot`:

```rust
#[tauri::command]
pub async fn spatial_drill_in(
    _window: Window,
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
    focused_fq: FullyQualifiedMoniker,
    snapshot: NavSnapshot,
) -> Result<FullyQualifiedMoniker, String>

#[tauri::command]
pub async fn spatial_drill_out(
    _window: Window,
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
    focused_fq: FullyQualifiedMoniker,
    snapshot: NavSnapshot,
) -> Result<FullyQualifiedMoniker, String>
```

### Kernel implementation

The drill methods read snapshot + `last_focused_by_fq`. Two reasonable homes:

(a) Keep them on `SpatialRegistry` since they read `last_focused_by_fq` (which lives there post-step-12). Add `&IndexedSnapshot` to their signatures.

(b) Move to `SpatialState` since they're decision-makers like `focus` / `navigate`.

Pick (a) — keeps the dependency footprint small. The methods become pure `(snapshot, registry) -> FQM` queries with no state mutation.

### React side

The React side already builds snapshots for nav and focus. Apply the same pattern to drill: read the active layer's `LayerScopeRegistry`, call `buildSnapshot(layerFq)`, pass it inline to the drill IPC.

## Tests

- Adapt the existing `first_matches_drill_in_first_child_fallback` test to pass a snapshot. Both `Direction::First` and `drill_in` cold-start must return the same FQM for the same scope set.
- New regression: `drill_in` honors `last_focused_by_fq` over the geometric fallback when both apply.
- New regression: `drill_out(fq)` returns `snapshot.get(fq).parent_zone` for a scope with a known parent_zone, and returns `focused_fq` (stay-put) for a scope at the layer root.
- `cargo test -p swissarmyhammer-focus` green.
- `pnpm -C kanban-app/ui test` green.

## README update

Step 14 is the docs-purge pass. For this step, just keep README guidance consistent with the rest: drill in / drill out IPCs accept a snapshot (same as every other focus-mutating IPC); no narrative changelog.

## Acceptance criteria

- `spatial_drill_in` and `spatial_drill_out` IPCs accept `snapshot: NavSnapshot`
- The React side builds a snapshot before invoking either drill IPC
- The drill kernel methods read from snapshot + `last_focused_by_fq` only — no references to `registry.scopes` (which is gone after step 12)
- Result for matching scope sets is identical to the pre-step-12 registry path
- The `first_matches_drill_in_first_child_fallback` regression still passes (drill_in cold-start ≡ Direction::First)
- `cargo test -p swissarmyhammer-focus` and `pnpm -C kanban-app/ui test` both green

## Files

- `kanban-app/src/commands.rs` — `spatial_drill_in` / `spatial_drill_out` accept snapshot
- `swissarmyhammer-focus/src/registry.rs` — `drill_in` / `drill_out` read snapshot + `last_focused_by_fq`
- `swissarmyhammer-focus/src/navigate.rs` — `first_matches_drill_in_first_child_fallback` test rewritten for snapshot
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — drill actions build snapshot

## Out of scope

- Behavioral changes to drill semantics — same contract as today
- Moving overlap warning (step 13)
- Docs purge (step 14) #stateless-nav