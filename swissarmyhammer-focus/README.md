# swissarmyhammer-focus

Headless spatial-navigation kernel for keyboard focus across 2-D
layouts. Generic and domain-free — nothing in here knows about kanban
tasks, columns, or any other application concept. Identities are
`FullyQualifiedMoniker` paths produced by the consumer (the path
through the focus hierarchy); the kernel only sees rectangles, layers,
and zones.

This README is the **canonical prose document for the navigation
contract**. Read this before touching any kernel code. The Rust source
in `src/navigate.rs` is the source of truth for *behavior*; this README
is the source of truth for *intent*.

## Primitives

The kernel exposes three peer types:

- **`FocusLayer`** — modal boundary. Layers form a per-window forest
  (e.g. window root → inspector → dialog). Spatial nav, fallback
  resolution, and zone-tree walks never cross a layer.
- **`FocusZone`** — navigable container within a layer. Zones group
  children (other zones or scopes), own a `last_focused` slot for
  drill-out memory, and form a tree rooted at the layer root.
- **`FocusScope`** — leaf in the spatial graph. Atomic focusable
  surface (a button, a pill, a text input). Has no children.

Identities are `FullyQualifiedMoniker` paths composed by the consumer.
Each primitive carries an FQM (the registry key) and a `SegmentMoniker`
(the relative segment, used for human-readable logs only).

```text
window layer
├── ui:navbar (zone)
│   ├── ui:navbar.board-selector (leaf)
│   ├── ui:navbar.inspect (leaf)
│   ├── field:board:b1.percent_complete (zone)
│   └── ui:navbar.search (leaf)
├── ui:perspective-bar (zone)
│   ├── perspective_tab:p1 (leaf)
│   └── perspective_tab:p2 (leaf)
└── ui:board (zone)
    └── column:TODO (zone)
        ├── field:column:TODO.name (zone)
        ├── task:T1A (zone)
        │   ├── card.drag-handle:T1A (leaf)
        │   ├── field:task:T1A.title (zone)
        │   └── card.inspect:T1A (leaf)
        └── task:T2A (zone)
```

## The sibling rule

> **Within a parent FocusZone, child FocusScope leaves and child
> FocusZone containers are siblings. Cardinal navigation treats them
> as peers.**

In short: **zones and scopes are siblings** under a parent zone. This
is the kernel's load-bearing contract; the geometrically best
candidate wins regardless of kind.

Concrete example — the card layout:

```text
parent: task:T1A (zone)
┌────────────────────────────────────────────┐
│ [drag-leaf]  [title-zone]  [inspect-leaf]  │  ← top row, all peers
│                                            │
│ [tags-zone]                                │  ← below
│ [add-tags-zone]                            │
└────────────────────────────────────────────┘
```

Cardinal nav from any of the top-row entries:

- `Right` from `drag-leaf` → `title-zone` (the geometrically closest
  Right peer of any kind), NOT `inspect-leaf` (further away).
- `Left` from `inspect-leaf` → `title-zone` (closest of any kind), NOT
  `drag-leaf` (further away).
- `Down` from `drag-leaf` (or `inspect-leaf`) → `tags-zone` in the same
  card (in-zone Down peer of any kind), NOT a peer of the card.

A pre-fix kernel that filtered iter 0 by kind would (incorrectly) jump
over the title zone and leave the card. That bug is the reason this
contract exists.

## The cascade

Cardinal navigation runs a four-step cascade per direction key:

1. **Iter 0 — any-kind in-zone peer search.** Candidates are ANY
   registered scope (leaf or zone) sharing the focused entry's
   `parent_zone`, geometrically in `direction`. Pick the best by
   Android beam score (`13 * major² + minor²`).

   *Example:* `Right` from `drag-leaf` → `title-zone` (closer than
   `inspect-leaf`, both are in-zone siblings under the card).

2. **Iter 1 — same-kind peer-zone search.** When iter 0 misses,
   escalate to the focused entry's `parent_zone` and search ITS peers.
   The parent IS a zone, so candidates are zones by construction —
   iter 1's same-kind filter is structural, not a kind policy.

3. **Cross-zone drill-in.** When iter 1 lands on a sibling zone, the
   cascade descends into that zone's natural child in the search
   direction — rightmost child for `Left`, leftmost for `Right`,
   bottom-most for `Up`, top-most for `Down` — recursing until it
   reaches a leaf (or a zone with no children, which is then returned
   as-is). The returned FQM identifies a leaf the focus indicator can
   paint on, not the destination zone itself. This matches the user's
   mental model — pressing `Left` lands them on something visible
   inside the leftward zone, not on the zone wrapper whose indicator
   may be suppressed.

   *Example:* `Down` from `tags-zone` (the bottom-most child of the
   card) → the title leaf of the next card below in the column. Iter 1
   finds `task:T2A` as `task:T1A`'s peer zone at `column:TODO`'s level,
   then the drill-in step descends into `task:T2A` and picks its
   top-most child for `Down` (the title leaf), which is what the user
   sees focus paint on.

4. **Drill-out fallback.** When iter 1 also misses, return the parent
   zone's FQM. A single key press moves at most one zone level out
   from the focused entry; the user is never "stuck" returning a
   stay-put unless the focused entry sits at the very root of its
   layer.

   *Example:* `Right` from `task:T1C` (rightmost column, no peer
   columns to its right) → `column:DONE` (the parent zone).

## Edge commands

`Direction::First`, `Direction::Last`, `Direction::RowStart`, and
`Direction::RowEnd` keep **level-bounded same-kind** semantics — no
escalation cascade, and only siblings of the focused entry's kind enter
the search. `Home` in a row of cells means "first cell", not "the row's
container zone"; that's the right semantics for those keys.

This is the one place same-kind filtering is correct policy (rather
than structural). Cardinal nav and edge commands are different
abstractions and have different sibling rules.

## Overrides (rule 0)

Each registered scope carries a `HashMap<Direction, Option<FullyQualifiedMoniker>>`
of per-direction navigation overrides. The override map runs BEFORE
the cascade:

- `direction → Some(target_fq)` — redirect: nav returns `target_fq`
  (when `target_fq` resolves in the focused scope's layer).
- `direction → None` — explicit "wall": nav returns the focused FQM
  (semantic stay-put), no beam search runs.
- absent — fall through to the cascade.

See `FocusScope::overrides` and `FocusZone::overrides`.

## No-silent-dropout

Nav and drill APIs always return a `FullyQualifiedMoniker`. "No motion
possible" is signalled by returning the focused entry's own FQM — the
React side detects "stay put" by comparing the returned FQM to the
previous focused FQM. Torn state (unknown FQM, orphan parent
reference) emits `tracing::error!` and echoes the input FQM so the
call site has a valid result. There is no `Option` or `Result` on
these APIs; silence is impossible. See `tests/no_silent_none.rs` for
the regression suite that pins each path.

## Kind is not a filter (anti-pattern callout)

Future contributors: do **not** re-introduce kind filtering at iter 0.
Doing so brings back the user-visible bugs the sibling rule fixes.
Specifically:

1. **Do not add `is_zone()` checks to `beam_among_in_zone_any_kind`**
   or to any iter-0 candidate filter. Cardinal nav at iter 0 considers
   all in-zone siblings together. The kind discriminator is invisible
   to the user, who sees only rectangles in space.
2. **Do not "fix" cross-card spillover by re-segregating kinds.** The
   correct fix for an in-card field zone leaking to the next card is
   to make the inner field a child of the card zone (so it does NOT
   share `parent_zone` with the card itself), not to add a kind
   filter to the kernel.
3. **Do not extend iter 1 with kind filters as policy.** Iter 1's
   same-kind filter is a structural fact — the parent IS a zone, so
   peers of the parent are zones. If you find yourself wanting to
   override iter 1's filter, you've found a different bug.
4. **Do not change edge commands' same-kind filter without changing
   the contract here.** `Home` / `End` semantics are deliberately
   different from cardinal nav.

## Cross-references

- `src/navigate.rs` — the algorithm. The module-level docstring leads
  with the sibling rule before the cascade walkthrough.
- `src/registry.rs` — the storage shape, including the path-prefix
  scope-is-leaf invariant that catches structural mistakes at
  registration time.
- `src/scope.rs` — the `FocusScope` and `FocusZone` peer types.
- `tests/in_zone_any_kind_first.rs` — synthetic regression suite for
  the any-kind iter-0 sibling rule, mirroring the card layout from the
  bug report.
- `tests/card_directional_nav.rs` — realistic-app trajectory tests for
  cardinal nav from a focused card.
- `tests/column_header_arrow_nav.rs` — interaction between the card,
  the column-name field zone, and the column zone.
- `tests/navbar_arrow_nav.rs` — Left/Right walks through a mixed-kind
  navbar (leaves + a percent-complete field zone).
