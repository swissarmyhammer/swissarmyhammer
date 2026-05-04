# swissarmyhammer-focus

Headless spatial-navigation kernel for keyboard focus across 2-D
layouts. Generic and domain-free — nothing in here knows about any other application concept. 
Identities are `FullyQualifiedMoniker` paths produced by the consumer (the path
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

## Cardinal nav (geometric)

Cardinal navigation is **keyboard-as-mouse**. Pressing an arrow key
picks the registered scope (leaf or zone, in the same `layer_fq`)
whose rect minimises the Android beam score (`13 * major² + minor²`)
across ALL registered scopes in the layer that:

1. **Lie strictly in the half-plane of D** — the candidate's leading
   edge in the reverse of `direction` is at or past the focused
   entry's leading edge in `direction`. For `Down`: `cand.top >=
   from.bottom`. This filters out containing parent zones (which
   extend on both sides of the focused rect) and overlapping rects.
2. **Pass the in-beam test for D** — the candidate overlaps the
   focused rect on the cross axis (horizontal overlap for
   `Up`/`Down`, vertical overlap for `Left`/`Right`).
3. **Are not the focused entry itself.**

No structural filtering — `parent_zone` and `is_zone` are tie-breakers
and observability only. The pick is purely geometric: zones and
scopes are siblings, layered or not, deeply nested or not. A leaf
inside a zone three levels down is a candidate at the same flat level
as a layer-root chrome zone.

*Example — same-row card jump:* `Right` from `task:T1A` (top card in
column TODO) → `task:T1B` (top card in column DOING). Both share the
same y range, so T1B is in-beam for `Right`; T1B's leading edge is
the closest leading edge in the Right half-plane. The algorithm does
not stop at the column boundary, drill out to a parent zone, or
descend through a column header.

*Example — cross-zone left edge:* `Left` from the leftmost
perspective tab → a view-button leaf inside `ui:left-nav`. The
LeftNav sidebar lives at the layer root, peer to the perspective bar
in structural terms, but its view-button leaves sit visually directly
to the left of the leftmost tab and so are the geometrically nearest
in-beam Left candidates.

*Example — visual edge stay-put:* `Right` from a card in the
rightmost column has nothing strictly in the Right half-plane (every
column zone, the board zone, and the navbar/perspective bar all share
the same right edge or extend leftward). The geometric pick echoes
the focused FQM — the user is at the visual edge of the layer.

### Tie-break: leaves over zones

When two candidates have equal beam scores, **leaves win over
zones**. This ensures that when the geometric pick lands equally on a
`showFocusBar=false` container and an inner leaf, the user sees the
focus indicator paint on the leaf rather than the invisible
container.

### Why geometric (and not structural)

The kernel's prior algorithm ran an iter-0 / iter-1 / drill-out
cascade keyed on the focused entry's `parent_zone`. That cascade lost
the user's mental model — "the visually-nearest thing in direction D"
— whenever the visual neighbor lived in a different sub-tree.
Symptoms included `target=None`, `scope_chain=["engine"]`, and focus
collapsing to the layer root when pressing `Left` from the leftmost
perspective tab or `Up` from a board column. The geometric pick
replaces that cascade with a single layer-wide search, fixing the
cross-zone bug class by construction.

See `tests/cross_zone_geometric_nav.rs` for the four cross-zone
regression tests that pin this contract.

## Drill in

`SpatialRegistry::drill_in` is the kernel side of the Enter key —
descend one level into the focused zone. Cardinal nav is purely
geometric and ignores the zone tree; drill-in is the operation where
the tree shape earns its keep.

### Contract

- **Children** = registered scopes whose `parent_zone` is the focused
  scope's FQM.
- **First child** = the child whose rect is topmost; ties broken by
  leftmost. Same `(top, left)` ordering as `Direction::First`, so the
  "Enter" and "Home" keys agree on which child is first.
- **No children** → return the focused FQM. The React side detects
  the equality (`result == focused_fq`) and falls through to
  `onEdit?.()` for editable leaves, or no-op for read-only ones. This
  is the no-silent-dropout path; pressing Enter on a leaf doesn't
  produce a `null` blip on the React side.
- **Leaf focused** → return the focused FQM. Leaves have no
  registered children to drill into.
- **Unknown FQM** → emit `tracing::error!` and return the focused FQM.
  Torn registry state — observable in logs, user-visible behavior
  matches the no-children case.

### Option A — last-focused memory is primary, first-child is the fallback

A zone carries an optional `last_focused: Option<FullyQualifiedMoniker>`.
The navigator updates this slot as focus moves inside the zone, so it
records "the last spot the user was at, inside this zone". `drill_in`
consults that slot **first**:

1. **Live `last_focused`** — the stored FQM still resolves to a
   registered scope: return that FQM. Preserves the user's "I came
   back to where I was" expectation across drill-out / drill-in
   cycles.
2. **Stale `last_focused`** — the stored FQM no longer resolves
   (e.g. a card was deleted while focus was elsewhere): treat it as
   absent and fall through to step 3. No trace; this is well-formed
   eventual-consistency behavior, not torn state.
3. **No `last_focused` recorded** (cold start, no prior visit): return
   the first child by `(top, left)` ordering. Empty zone returns the
   focused FQM.

The alternative ("Option B — first-child is primary, `nav.resume` is a
separate op") was considered and rejected. The user's mental model is
that drill-in is symmetric with drill-out: drill into a zone, navigate
around inside, drill out, then later drill back in — and end up where
you were. Forcing the user to learn a separate "resume" key to get
that behavior splits one operation into two without earning anything;
the cold-start case (no `last_focused`) naturally degrades to the
first-child rule, so a single op covers both.

### Cross-references

- `SpatialRegistry::drill_in` in `src/registry.rs` — the
  implementation.
- `tests/drill.rs` — integration coverage for live / stale / absent
  `last_focused`, empty zone, leaf, unknown FQM, round-trip, and the
  inspector field-zone case (horizontal pill row, leftmost wins; empty
  field falls through to edit).
- `tests/no_silent_none.rs` — pins the "leaf returns leaf, no trace"
  and "unknown FQM emits trace AND echoes input" rules.
- `01KQQDXHANWGMBG872KZ3FZ86P` (Tauri adapter) — the React side's
  drill-into-editor fall-through happens **after** `drill_in` returns
  the focused FQM. The kernel does not "drill into nothing"; that
  decision belongs to the consumer.

## Drill out

`nav.drillOut` (Escape) is **focus the parent scope**. Like drill-in,
it is a pure registry query — `SpatialRegistry::drill_out(fq, focused_fq)`
— with one rule:

> Parent = the focused scope's `parent_zone`. Return that FQM.

The two non-motion paths follow the no-silent-dropout contract:

- **Layer-root edge — return `focused_fq`, no trace.** When the
  focused scope's `parent_zone` is `None`, the scope sits directly
  under its layer root and there is nothing to walk to. The kernel
  echoes the focused FQM (semantic stay-put). The React glue
  (`buildDrillCommands` in `app-shell.tsx`) detects the FQM-equality
  and falls through to `app.dismiss`, which closes the topmost modal
  layer (e.g. the inspector panel). This is how one Escape closes a
  modal even though the kernel itself never crosses the layer
  boundary.
- **Torn state — return `focused_fq`, trace error.** Two paths emit
  `tracing::error!` with `op = "drill_out"` and echo the input FQM:
  the input `fq` is unknown to the registry, OR the focused scope's
  `parent_zone` names an FQM that is not registered. User-observable
  behavior matches the layer-root edge (focus stays put, dismiss
  fall-through), but ops / devs can chase the inconsistency in logs.

`drill_out` does **not** consult sibling zones, geometric scoring,
overrides, or last-focused memory — it is a single hop up the
`parent_zone` chain. Repeated drill-outs walk the zone chain toward
the layer root one hop at a time before any dismiss fires; the
dismiss fall-through is the kernel's signal to the React layer, not
a synthesised "skip the chain" path. Zones drill out the same as
leaves — a nested zone returns its enclosing zone, not the layer
root.

### Pinned by

- `tests/drill.rs::drill_out_focusable_returns_parent_zone_fq` —
  leaf with parent.
- `tests/drill.rs::drill_out_zone_returns_parent_zone_fq` — nested
  zone walks one hop to the enclosing zone, not straight to the
  layer root.
- `tests/drill.rs::drill_out_at_layer_root_returns_focused_fq` —
  layer-root edge.
- `tests/drill.rs::drill_out_unknown_fq_echoes_focused_fq` — torn
  state, unknown FQM.
- `tests/no_silent_none.rs::drill_out_layer_root_returns_focused_fq_no_trace`
  — layer-root edge emits no error trace.
- `tests/no_silent_none.rs::drill_out_torn_parent_returns_focused_fq_and_traces_error`
  — torn `parent_zone` reference emits exactly one error trace.
- `tests/no_silent_none.rs::drill_out_unknown_fq_returns_focused_fq_and_traces_error`
  — unknown input FQM emits exactly one error trace.
- `tests/inspector_dismiss.rs` — the kernel-level seam that pins
  the React-side "Escape closes the inspector" chain
  (`nav.drillOut` → `app.dismiss` → `inspector_close`).

## First / Last

`nav.first` (Home) and `nav.last` (End) **focus the focused scope's
children**. They are the kernel side of "go to the start / end of
this thing". Like drill-in, they are operations where the tree shape
earns its keep — cardinal nav is purely geometric and ignores the
zone tree, but first / last anchor on a specific scope's children.

### Contract

- **Children** = registered scopes whose `parent_zone` is the focused
  scope's FQM.
- **First child** = the child whose rect is topmost; ties broken by
  leftmost. Same `(top, left)` ordering as `drill_in`'s cold-start
  fallback, so the "Enter" and "Home" keys agree on which child is
  first.
- **Last child** = the child whose rect is bottommost; ties broken by
  rightmost.
- **Kind is not a filter.** Both leaves and sub-zones are eligible
  children. A focused zone with one leaf child and one sub-zone child
  picks whichever sits topmost / bottommost — kind doesn't enter the
  decision.
- **Leaf focused** → return the focused FQM. Leaves have no
  registered children. This is the no-silent-dropout stay-put — no
  trace, just an FQM-equality signal that the React side can ignore
  or wire to a fall-through (e.g. "Home in a text field stays a text-
  field affordance").

### Shared semantics with drill-in

`nav.first` is identical to `nav.drillIn` when the focused zone has
no `last_focused` memory — both pick the topmost-then-leftmost
child. The two ops differ only in the key binding (Enter vs Home)
and the React-side editor-focus extension on Enter that `nav.first`
does not get.

The two paths now share the same `first_child_by_top_left` helper
in `src/registry.rs`, so divergence is structurally impossible. The
`first_matches_drill_in_first_child_fallback` unit test is the
behavioural backstop on that contract.

### `RowStart` / `RowEnd` are deprecated aliases

`Direction::RowStart` and `Direction::RowEnd` carry
`#[deprecated(since = "0.12.11", note = "use Direction::First")]` and
`#[deprecated(since = "0.12.11", note = "use Direction::Last")]`
respectively on the Rust enum and have been removed from the
TypeScript-side `Direction` union. They are kept on the Rust enum for one release so
external wire-format consumers can migrate; the variants evaluate to
the same children-of-focused-scope pick as `Direction::First` and
`Direction::Last`. The user's model has no separate "first in row"
concept — the focused scope IS the row, so "first in row" and
"first child" collapse to the same op.

The pre-redesign behavior was different: `RowStart` / `RowEnd`
filtered candidates by vertical-overlap with the focused leaf,
returning the leftmost / rightmost in-row sibling. That layered an
extra geometric filter on top of the same-kind sibling rule which
the redesign drops entirely. New code (Rust or TypeScript) must use
`First` / `Last`; the alias-equivalence is pinned by the
`deprecated_row_start_end_still_alias_first_last` unit test in
`src/navigate.rs` for the duration of the deprecation window.

### Override (rule 0) still runs first

The focused scope's per-direction `overrides` map short-circuits the
children pick exactly as it does for cardinal nav. A `direction →
None` override produces stay-put without consulting children; a
`direction → Some(target)` override teleports to the target FQM.

### Pinned by

- `src/navigate.rs::tests::first_last_on_leaf_returns_focused_self`
  — leaf no-op.
- `src/navigate.rs::tests::first_last_on_zone_with_one_child_returns_that_child`
  — single-child zone.
- `src/navigate.rs::tests::first_last_on_zone_with_row_of_children`
  — three children in a row, picks leftmost / rightmost.
- `src/navigate.rs::tests::first_last_on_zone_with_column_of_children`
  — three children in a column, picks topmost / bottommost.
- `src/navigate.rs::tests::first_last_considers_children_of_any_kind`
  — mixed leaf and sub-zone children, kind is not a filter.
- `src/navigate.rs::tests::deprecated_row_start_end_still_alias_first_last`
  — the deprecated `RowStart` / `RowEnd` aliases keep resolving to
  the same child pick as `First` / `Last` for the duration of the
  one-release deprecation window.
- `src/navigate.rs::tests::first_matches_drill_in_first_child_fallback`
  — `nav.first` and `drill_in`'s cold-start fallback share the
  topmost-then-leftmost ordering.
- `tests/navigate.rs::first_on_zone_picks_topmost_leftmost_child`
  and the surrounding integration cases — pin the children-of-focused-
  scope contract end-to-end through `BeamNavStrategy`.

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
previous focused FQM.

Two distinct stay-put paths feed this contract:

- **Stay-put when the half-plane is empty.** Cardinal nav from a
  scope at the visual edge of the layer (no candidate satisfies the
  strict half-plane and in-beam tests) returns the focused FQM
  silently, no log noise. This is the well-formed edge — pressing
  Right from the rightmost focusable thing in the layer produces no
  motion because there is nothing to the right.
- **Stay-put on override walls and torn state.** An explicit `None`
  override is a wall: stay-put, no trace. An unknown focused FQM is
  torn state: stay-put, AND `tracing::error!` fires so ops can chase
  the inconsistency in logs.

There is no `Option` or `Result` on these APIs; silence is
impossible. See `tests/no_silent_none.rs` for the regression suite
that pins each path.

## Scrolling

**The kernel is scroll-unaware.** It knows about layers, zones, scopes,
and the rectangles those primitives carry. It does not know about DOM
scroll containers, `overflow: auto`, virtualizers, or which rows are
currently mounted. The scroll-on-edge rule that lets cardinal nav
cross the boundary of a virtualized scroll container lives **in React
glue, not in this crate.**

Why the kernel stays out of it: the kernel's invariant is that
geometric pick operates on the rect set the registry currently holds.
Scrolling changes which rows a *virtualizer* mounts and therefore
which rects the registry holds, but that is a side-effect of the React
tree's render cycle — entirely outside the kernel's contract. The
kernel returns stay-put when the half-plane is empty (see
"No-silent-dropout" above); whether that stay-put reflects a true
visual edge or a virtualization boundary is a question only the React
layer can answer.

### The rule (React-side, not kernel-side)

When a cardinal `spatial_navigate` returns stay-put (the focused FQM
echoed back as `next_fq`) AND the focused scope's nearest scrollable
ancestor in direction *D* can scroll further in *D*, the React glue
scrolls that ancestor by one item-height in *D*, waits one animation
frame for the virtualizer to mount the freshly-revealed row, and
re-dispatches the same nav. The retry depth is capped at 1 — if the
second nav also returns stay-put, focus stays put. There is no
infinite loop.

The implementation lives in `kanban-app/ui/src/lib/scroll-on-edge.ts`
(`runNavWithScrollOnEdge` plus the `scrollableAncestorInDirection` /
`canScrollFurther` / `scrollByItemHeight` helpers); it is wired into
`buildNavCommands` in `kanban-app/ui/src/components/app-shell.tsx`.

### Implications for kernel contributors

- Do not add scroll-aware logic to `navigate.rs`, `state.rs`, or any
  other file in this crate. The kernel's job ends at "return the
  focused FQM when the half-plane is empty"; the React layer takes it
  from there.
- Do not add `scrollHeight` / `clientHeight` / `overflow` fields to
  `Rect`, `FocusScope`, or `FocusZone`. The kernel sees the
  same-shape rectangles regardless of whether the consumer's tree
  uses virtualization.
- Tests in this crate (`tests/no_silent_none.rs`,
  `tests/card_directional_nav.rs`, etc.) keep returning stay-put for
  the rectangle-only edge case. The React-side test
  `column-view.virtualized-nav.browser.test.tsx` pins the
  scroll-on-edge fall-through end-to-end.

## Kind is not a filter (anti-pattern callout)

The geometric pick is **even more committed** to the "kind is not a
filter" principle than the previous structural cascade. The cascade
had a same-kind iter-1 step that was justifiable (the parent IS a
zone, so its peers are structurally zones). The geometric pick has
no such step at all — there is one search, one rule, no kind-keyed
branches.

Future contributors: do **not** re-introduce kind filtering anywhere
in the cardinal-nav path. Doing so brings back the user-visible bugs
the geometric pick fixes. Specifically:

1. **Do not add `is_zone()` checks to the candidate filter inside
   `geometric_pick`.** Cardinal nav considers every registered scope
   in the layer regardless of kind. The kind discriminator is
   invisible to the user, who sees only rectangles in space. The
   leaves-over-zones tie-break is the only place `is_zone` is
   consulted, and it activates only on exact score ties.
2. **Do not re-introduce a `parent_zone` filter.** Two scopes with
   different `parent_zone` are still peers under the geometric pick.
   The cross-zone bug class — pressing `Left` from a leftmost tab
   landing on `target=None`, pressing `Up` from a column collapsing
   to the engine root — was caused by treating `parent_zone` as a
   structural barrier. Don't do it again.
3. **Do not "fix" a misrouted nav by tweaking parent_zone wiring on
   the React side.** The kernel ranks by rect; if pressing Right
   lands somewhere unexpected, look at the rects, not the
   `parent_zone` graph. (`parent_zone` still earns its keep for
   `drillIn` / `first` / `last` — those operations DO walk the zone
   tree.)
4. **Do not change edge commands' same-kind filter without changing
   the `## Edge commands` contract here.** `Home` / `End` semantics
   are deliberately different from cardinal nav: edge commands are
   level-bounded and same-kind by design.

## Coordinate system

> **All registered rects are viewport-relative, sampled by
> `getBoundingClientRect()`, and refreshed on ancestor scroll via
> `useTrackRectOnAncestorScroll`. The kernel's geometric pick is
> correct iff this invariant holds across all candidate rects in the
> same layer.**

Every cardinal-nav comparison the kernel makes is a beam score over
viewport-relative `(x, y, width, height)` quadruples. If half the
scopes in a layer were sampled in viewport space and the other half in
document space (e.g. someone read `node.offsetTop` instead of
`getBoundingClientRect().top`), beam search ranks across two
incompatible coordinate frames and silently picks the wrong neighbor.
No exception, no warning — just bad nav. The same applies to a stale
rect: a card that was sampled before its column scrolled is now at a
viewport-y the kernel thinks is meaningful but that the user no longer
sees.

This invariant is **load-bearing**. The validators below exist to
catch the bug class at the IPC boundary rather than letting it
manifest as a slow user complaint.

### Dev-mode validators (TS side)

`kanban-app/ui/src/lib/rect-validation.ts` wraps every spatial-nav
registration / update IPC adapter
(`SpatialFocusActions.registerScope`, `registerZone`, `updateRect`).
On `import.meta.env.DEV` only, every outgoing rect is checked for:

- finite `x`, `y`, `width`, `height` (no `NaN`, no `±Infinity`),
- `width > 0` and `height > 0`,
- coordinates inside `[-1e6, 1e6]` (anything beyond is almost certainly
  document-relative — desktop viewports never get that big),
- a fresh sample timestamp (no more than one animation frame ≈ 16 ms
  old, captured at the IPC adapter boundary).

Violations are logged via `console.error` (hard violations) or
`console.warn` (staleness), tagged with the op, FQM, and offending
property; the IPC `invoke` proceeds either way. The validator is
observability — it never throws. Production builds skip the check
entirely.

### Debug assertions (kernel side)

`swissarmyhammer-focus/src/registry.rs::validate_rect_invariants` is
called from `register_scope`, `register_zone`, and `update_rect`. In
`cfg(debug_assertions)` builds it emits one `tracing::error!` per
violation (finite, positive-dim, plausible-scale) tagged with the op
tag and the FQM. In release builds it is a no-op. The kernel never
panics on a bad rect — like the TS side, validation is observability,
not enforcement.

### Lazy coordinate-consistency walk

`SpatialRegistry::validate_coordinate_consistency(layer_fq)` runs a
robust per-layer scan: it computes the median position of all rect
centers in the layer (the median is robust to outliers in a way the
mean is not), and emits one `tracing::warn!` per scope whose distance
to that position is more than 10× the median distance. That magnitude
jump is a strong signal of a coordinate-system mismatch (half
viewport-relative, half document-relative).

The walk is **lazy**: the registry caches a `validated_layers` set,
and a re-validation is triggered only after a registration / update
mutates the layer (or after the layer is removed). Intended call site
is the navigator's first nav into the layer per session — paid for
once, not every frame. The check is observability-only and never
panics.

### Audit history

The audit that established the coordinate-system contract lives in
the implementation card `01KQQV2H8HW2BF3619DFXHX3RX`. Every TS-side
registration callsite was enumerated and confirmed to call
`getBoundingClientRect()` directly on the scope's own DOM element —
`focus-scope.tsx`, `focus-zone.tsx`, `use-track-rect-on-ancestor-scroll.ts`,
`spatial-focus-context.tsx`, and the placeholder batch in
`column-view.tsx`. No callsite ships a cached value, a parent's rect,
or a computed offset; the validators above are the safety net for
future regressions, not a fix for an outstanding bug.

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
- `tests/coordinate_invariants.rs` — integration test for the
  coordinate-system validators: confirms that the kernel does not
  panic and the no-silent-dropout contract still holds even when the
  registry is fed a mix of viewport-relative and document-relative
  rects.
