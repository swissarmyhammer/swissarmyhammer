---
assignees:
- wballard
depends_on:
- 01KQW643TXM5YFKRZTNB8JPVVC
position_column: todo
position_ordinal: d480
project: spatial-nav
title: 'spatial-nav redesign step 5: introduce last_focused_by_fq map and adapt record_focus to walk snapshot'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Stand up the `last_focused_by_fq` top-level map that will replace the per-scope `last_focused` field after cutover. Adapt `record_focus` to write to both during the transition. Adapt `record_focus` to walk a snapshot when one is provided.

## What to build

### New field

`SpatialRegistry::last_focused_by_fq: HashMap<FullyQualifiedMoniker, FullyQualifiedMoniker>` — keyed by ancestor FQM, value is the most-recent descendant FQM focused under that ancestor.

### Dual-write semantics during transition

`record_focus` walks the focused FQ's ancestor chain. For each ancestor scope, it writes:

- `registry.scopes[ancestor].last_focused = Some(fq)` (existing per-scope field — keep writing during transition)
- `registry.last_focused_by_fq.insert(ancestor, fq)` (new top-level map)

Layer ancestors continue to write only to `FocusLayer::last_focused` (unchanged from today).

### Read precedence

`resolve_fallback`'s `FallbackParentZoneLastFocused` arm should consult `last_focused_by_fq` first (which is authoritative going forward), falling back to per-scope `last_focused` if the map has no entry. The two should always be in sync after this step ships, but the fallback keeps existing tests stable while the new field is bedded in.

### Snapshot-walking variant

`record_focus` currently walks `registry.scopes[fq].parent_zone` recursively. Add an `Option<&IndexedSnapshot>` parameter. When `Some`:

- Walk the snapshot's `parent_zone_chain` for the focused FQ
- For each ancestor in the snapshot, write to `last_focused_by_fq` (and to `registry.scopes[ancestor].last_focused` if the ancestor still exists in `registry.scopes` for the dual-write — during cutover this becomes a no-op)
- Then walk the layer chain via `registry.layers[fq].parent` exactly as today

When `None`: existing behavior (walks registry).

## Tests

- After every focus mutation in existing tests, both `registry.scopes[ancestor].last_focused` and `registry.last_focused_by_fq.get(ancestor)` agree.
- Snapshot-walk record_focus produces the same `last_focused_by_fq` writes as registry-walk for the same scope set.
- `resolve_fallback`'s `FallbackParentZoneLastFocused` reads return the same FQM whether served from `last_focused_by_fq` or per-scope `last_focused`.

## Out of scope

- Deleting per-scope `last_focused` (step 12 — cutover)
- IPC commands (steps 6, 7, 8)
- Snapshot-driven call sites at the IPC boundary (steps 6, 7, 8)

## Acceptance criteria

- `cargo test -p swissarmyhammer-focus` green
- `last_focused_by_fq` populated and synchronized with per-scope `last_focused` after every mutation
- `record_focus` accepts optional snapshot; both paths produce equivalent state

## Files

- `swissarmyhammer-focus/src/registry.rs` — new field + dual-write in `record_focus`
- `swissarmyhammer-focus/src/state.rs` — `resolve_fallback` reads from `last_focused_by_fq` first #01KQTC1VNQM9KC90S65P7QX9N1