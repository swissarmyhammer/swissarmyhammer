# swissarmyhammer-focus

Headless spatial-navigation kernel. Per-decision navigation snapshots
ride on every focus-mutating call; the kernel reads scope geometry out
of the snapshot at the moment of a decision and holds no scope replica
between calls.

## Primitive

**`SnapshotScope`** — has a rect, optional `parent_zone`, and per-
direction navigation overrides. Identified by a
`FullyQualifiedMoniker` (the path through the focus hierarchy). Carried
inline on the wire as part of a `NavSnapshot`.

## Boundary

**`Layer`** — modal boundary. Layers form a per-window forest (window
root → inspector → dialog). Nav and drill operations never cross a
layer. Layers are the only structural state the kernel persists between
decisions.

## Operations

### up / down / left / right

Geometric beam pick across all snapshot scopes in the layer. For
direction D, candidates are the scopes that:

1. lie strictly in the half-plane of D from the focused rect,
2. overlap the focused rect on the cross axis (horizontal overlap for
   up/down, vertical overlap for left/right),
3. are not the focused scope itself.

Pick the candidate minimising the Android beam score
`13 * major² + minor²`. Tie-break: leaves win over scopes-with-children.

Empty candidate set → stay-put.

### drill down

Focus a child of the focused scope.

1. If the kernel's `last_focused_by_fq` map has an entry for the
   focused scope and the recorded target is in the snapshot, return it.
2. Else return the topmost-then-leftmost child in the snapshot.
3. Else (no children) stay-put.

### drill up

Return the focused scope's parent FQM. No parent → stay-put.

### first sibling / last sibling

Return the topmost-leftmost / bottommost-rightmost child of the focused
scope's parent. Focused scope at the layer root → stay-put.

## Overrides (rule 0)

Each scope carries a per-direction override map. It runs before
everything else. `Some(target_fq)` teleports; `None` is a wall
(stay-put).

## No-silent-dropout

Every operation returns a `FullyQualifiedMoniker`. Never `Option`,
never `Result`. "No motion possible" is signalled by echoing the
focused FQM — the React side detects stay-put by FQM equality.

## Coordinate system

All snapshot rects are viewport-relative, sampled by
`getBoundingClientRect()` on the React side at decision time. The
kernel ranks by raw rect; mixing coordinate frames silently picks the
wrong neighbor.

## Scrolling

Not in the kernel. When a cardinal nav returns stay-put and the
focused scope's nearest scrollable ancestor in D can scroll further,
the React layer scrolls one item-height, waits a frame for the
virtualizer to mount the freshly-revealed row, and re-dispatches the
same nav. The retry depth is capped at 1.
