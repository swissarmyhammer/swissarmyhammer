//! Pluggable navigation strategy and the default Android-style beam
//! search.
//!
//! [`NavStrategy`] abstracts the algorithm that picks the next focus
//! target given the current registry state, the currently focused
//! [`SpatialKey`] paired with its [`Moniker`], and the requested
//! [`Direction`]. Consumers that want the default behavior use
//! [`BeamNavStrategy`]; tests and specialised layouts can swap in a
//! custom impl without touching [`SpatialState`].
//!
//! # No-silent-dropout contract
//!
//! Nav and drill APIs always return a [`Moniker`]. "No motion possible"
//! is communicated by returning the focused entry's own moniker — the
//! React side detects "stay put" by comparing the returned moniker to
//! the previous focused moniker. Torn state (unknown key, orphan parent
//! reference) emits `tracing::error!` and echoes the input moniker so
//! the call site has a valid result. There is no `Option` or `Result`
//! on these APIs; silence is impossible.
//!
//! Two principles distinguish the two non-motion paths:
//!
//! - **No motion → return focused moniker (no trace).** A semantic
//!   "stay put" — wall override, layer-root edge, leaf with no
//!   children, drill-out at root. The kernel returns the focused
//!   entry's own moniker. Observable: focus stays where it was, no
//!   `null` blip on the React side, no log noise.
//! - **Torn state → trace error AND echo input.** A genuine error —
//!   unknown [`SpatialKey`], orphan parent reference, registry
//!   inconsistency. The kernel emits `tracing::error!` with the
//!   operation, the relevant key(s), and the moniker being echoed
//!   back, then returns the input moniker so the call site has a
//!   valid value. User-observable behavior is identical to the "no
//!   motion" case (focus stays put), but ops / devs can chase the
//!   error in logs.
//!
//! The trait returns a [`Moniker`] rather than a [`SpatialKey`] so
//! consumers that key off entity identity (the same reason
//! [`crate::state::FocusChangedEvent::next_moniker`] exists) can act on
//! the result without an extra reverse-lookup through the registry.
//!
//! # Algorithm — unified edge drill-out cascade
//!
//! The unified-policy supersession card
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`] replaced the old per-direction tactical
//! rules (rule-1 within-zone, rule-2 cross-zone leaf fallback, plus a
//! separate `navigate_zone` for zone-level focus) with **one** cascade
//! that applies to leaves and zones alike.
//!
//! Within a single layer (the focused entry's `layer_key` is the hard
//! boundary — nav never crosses it), the cascade runs:
//!
//! 1. **Iter 0 — same-kind peer search** at the focused entry's level:
//!    candidates are scopes of the same kind as `focused` (leaf for
//!    leaf, zone for zone) sharing its `parent_zone`. If a candidate
//!    satisfies the in-beam Android score (`13 * major² + minor²`),
//!    return it.
//! 2. **Escalate** to the focused entry's `parent_zone` (with a
//!    layer-boundary guard — escalation never crosses `LayerKey`). If
//!    the focused entry has no parent zone (it sits at the layer
//!    root), the cascade returns `None`.
//! 3. **Iter 1 — sibling-zone peer search** at the parent's level:
//!    candidates are zones (the parent is itself a zone, so iter 1's
//!    same-kind filter restricts to zones) sharing the parent's
//!    `parent_zone` — i.e. the focused entry's grandparent in the
//!    zone tree. Same beam scoring. If a candidate matches, return it.
//! 4. **Drill-out fallback**: when no peer matches at iter 0 *or* iter
//!    1, return the parent zone itself. A single key press moves at
//!    most one zone level out from the focused entry; the user is
//!    never "stuck" returning `None` unless the focused entry sits at
//!    the very root of its layer.
//!
//! Same-kind filtering at iter 0 is intentional. In production, a
//! `<Field>` zone mounted inside a `<FocusScope>` card body inherits
//! the card's enclosing `parent_zone` (the column), so the field zone
//! and the card leaf are sibling-registered even though visually the
//! field is *inside* the card. Pulling both kinds into iter 0 would
//! let pressing Down from a focused card land on a field zone inside
//! the next card (vertically aligned because the field is inside the
//! card), stealing the press from the card-leaf neighbor. Same-kind
//! filtering keeps "Down from a card" landing on the next card; users
//! cross the kind boundary via drill-in / drill-out, not via cardinal
//! nav.
//!
//! Edge commands ([`Direction::First`], [`Direction::Last`],
//! [`Direction::RowStart`], [`Direction::RowEnd`]) keep their
//! level-bounded behavior — no escalation cascade. They pick the
//! boundary candidate from the focused entry's siblings only. The
//! drill-out semantics are specific to cardinal directions where the
//! user is steering visually; row/page commands stay where they are.
//!
//! Override (rule 0) still runs first — the focused scope's
//! per-direction `overrides` map short-circuits the cascade entirely.
//!
//! # Why one cascade replaces the per-direction rules
//!
//! The old split (leaf rule 1 / leaf rule 2 / zone-only nav) treated
//! each surface independently and accumulated five distinct user-
//! reported bugs (each a symptom of "navigation in direction X has no
//! in-beam candidate at the focused entry's level"). The unified cascade
//! is one rule: when there's no peer at the current level, escalate;
//! if escalation finds a peer at the parent's level take that, else
//! drill out to the parent zone itself. Cross-column horizontal nav
//! works because escalation lands on the column-zone level and finds
//! the next column zone as a peer; vertical nav out of a card body
//! works because escalation surfaces the column header (a leaf at the
//! focused entry's iter-0 level) when the focused entry is the topmost
//! card. The same cascade handles both without per-direction
//! special-casing.
//!
//! # Scoring rationale
//!
//! For cardinal directions the beam test is a **hard filter** — a
//! candidate's rect must overlap the focused rect's cross-axis
//! projection, otherwise it is dropped from consideration entirely.
//! Among the in-beam candidates, the Android-derived score
//! `13 * major² + minor²` selects the closest aligned target (zero
//! minor wins ties; lateral drift breaks them in favor of the closer
//! one). See [`pick_best_candidate`] for the rationale on why in-beam
//! is a hard filter — short answer: out-of-beam fallbacks let the
//! navigator pick a navbar leaf when the user pressed `right` from the
//! rightmost card on the board, which is jarring and not what a
//! cardinal direction is meant to do.
//!
//! [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"

use crate::registry::SpatialRegistry;
use crate::scope::{FocusZone, RegisteredScope};
use crate::types::{pixels_cmp, Direction, LayerKey, Moniker, Pixels, Rect, SpatialKey};

/// Pluggable navigation algorithm.
///
/// Given the current registry state, the focused [`SpatialKey`] paired
/// with its [`Moniker`], and a [`Direction`], return the [`Moniker`] of
/// the next focus target. When motion is not possible (visual edge of
/// the layout, override wall, layer root, or torn-state errors), the
/// strategy returns `focused_moniker` itself — never `None`. See the
/// module docs for the no-silent-dropout contract this enables.
///
/// Implementations are `Send + Sync` so adapters can store them behind
/// an `Arc<dyn NavStrategy>` shared across async tasks.
pub trait NavStrategy: Send + Sync {
    /// Pick the next focus target.
    ///
    /// # Parameters
    /// - `registry` — the current registry. Strategies typically read
    ///   [`SpatialRegistry::scope`] for `focused` to discover its rect
    ///   and layer, then iterate [`SpatialRegistry::scopes_in_layer`]
    ///   for candidates.
    /// - `focused` — the [`SpatialKey`] of the currently focused scope.
    /// - `focused_moniker` — the [`Moniker`] paired with `focused`.
    ///   Echoed back when no motion is possible or torn state is
    ///   detected, so the caller never sees a silent `None`.
    /// - `direction` — the direction the user pressed.
    ///
    /// # Returns
    /// The [`Moniker`] of the next focus target. When the strategy has
    /// a real target (peer match, drill-out fallback to a parent zone,
    /// override redirect), that target's moniker is returned. When the
    /// strategy declines (override wall, layer root, unknown key, torn
    /// parent reference) the returned moniker equals `focused_moniker`
    /// — the call site detects "stay put" by equality comparison.
    /// Torn-state paths additionally emit `tracing::error!` before
    /// returning so the issue is observable in logs.
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused: &SpatialKey,
        focused_moniker: &Moniker,
        direction: Direction,
    ) -> Moniker;
}

/// Default Android-beam-search navigation strategy.
///
/// Implements the unified cascade described in the module docs:
/// override (rule 0) → iter-0 peer search at the focused entry's
/// level → iter-1 peer search at the parent's level → drill-out to
/// the parent zone itself when no peer matches at either level.
/// Edge commands ([`Direction::First`], [`Direction::Last`],
/// [`Direction::RowStart`], [`Direction::RowEnd`]) keep their
/// level-bounded behavior — no escalation cascade for those.
#[derive(Debug, Default, Clone, Copy)]
pub struct BeamNavStrategy;

impl BeamNavStrategy {
    /// Construct a fresh `BeamNavStrategy`. Equivalent to
    /// `BeamNavStrategy::default()` — provided for symmetry with other
    /// `new`-flavored constructors in the kernel.
    pub fn new() -> Self {
        Self
    }
}

impl NavStrategy for BeamNavStrategy {
    /// Run the override-first cascade: rule 0 consults the focused
    /// scope's per-direction `overrides` map; on no-op fall-through, the
    /// unified two-level cascade with drill-out fallback fires for
    /// cardinal directions, and the level-bounded edge command fires
    /// for `First` / `Last` / `RowStart` / `RowEnd`.
    ///
    /// Layer is the absolute boundary throughout — every candidate set
    /// is filtered by `candidate.layer_key == focused.layer_key` before
    /// any scoring runs, and escalation refuses to cross from one
    /// `LayerKey` to another (the inspector layer is captured-focus).
    ///
    /// # No-silent-dropout contract
    ///
    /// Per the module docs, this method always returns a [`Moniker`]:
    /// either the next focus target, or `focused_moniker` itself when
    /// no motion is possible. An unknown `focused` key is treated as
    /// torn state — `tracing::error!` fires and `focused_moniker` is
    /// echoed back. See the module docs for the full distinction
    /// between "no motion" (silent) and "torn state" (traced).
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused: &SpatialKey,
        focused_moniker: &Moniker,
        direction: Direction,
    ) -> Moniker {
        let Some(entry) = registry.entry(focused) else {
            // Torn state: caller passed a key that has no registry
            // entry. Trace the operation and echo the input moniker.
            tracing::error!(
                op = "nav",
                focused_key = %focused,
                focused_moniker = %focused_moniker,
                ?direction,
                "unknown focused key passed to BeamNavStrategy::next"
            );
            return focused_moniker.clone();
        };

        // Rule 0: per-direction override on the focused scope.
        //
        // The outer `Option` distinguishes "did the override apply?":
        //   - `Some(target)`  → redirect override; that target wins.
        //   - `Some(None-ish)` (handled below) → wall; stay put.
        //   - `None`          → override did not apply; fall through.
        match check_override(registry, entry, direction) {
            Some(Some(target)) => return target,
            Some(None) => {
                // Explicit wall — semantic "stay put", not torn state.
                // Return focused_moniker without tracing.
                return focused_moniker.clone();
            }
            None => {} // fall through to cascade
        }

        match direction {
            Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
                cardinal_cascade(registry, entry, focused_moniker, direction)
            }
            Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
                edge_command(registry, entry, focused_moniker, direction)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule 0: per-direction override on the focused scope.
// ---------------------------------------------------------------------------

/// Resolve the per-direction override on `focused`, if any.
///
/// Each registered scope carries a `HashMap<Direction, Option<Moniker>>`
/// of navigation overrides. The outer [`Option`] of the return value
/// encodes "did an override apply?", and the inner [`Option<Moniker>`]
/// encodes the answer when it did:
///
/// - **`None`** — no entry for `direction` on the focused scope (or the
///   entry names a target that does not resolve in the focused scope's
///   layer). The override didn't apply; the caller must fall through
///   to the beam-search cascade.
/// - **`Some(None)`** — explicit "wall": the override map maps
///   `direction → None`. Navigation is blocked; the strategy returns
///   `None` without consulting beam search.
/// - **`Some(Some(target_moniker))`** — redirect: the override map maps
///   `direction → Some(target)` *and* `target` is registered in the
///   focused scope's layer. Returns the target moniker; beam search
///   does not run.
///
/// Layer scoping is enforced here, not at registration: a target that
/// names a moniker registered in a *different* layer is treated as
/// "unresolved" and the override falls through to beam search. Cross-
/// layer teleportation is never allowed, even via override.
fn check_override(
    registry: &SpatialRegistry,
    focused: &RegisteredScope,
    direction: Direction,
) -> Option<Option<Moniker>> {
    let entry = focused.overrides().get(&direction)?;
    match entry {
        // Explicit `None` — block navigation in this direction.
        None => Some(None),
        // `Some(target)` — resolve only within the focused scope's layer.
        // A target in a different layer (or unregistered entirely) makes
        // the override fall through to beam search.
        Some(target) => {
            let target_in_layer = registry
                .entries_in_layer(focused.layer_key())
                .any(|s| s.moniker() == target);
            if target_in_layer {
                Some(Some(target.clone()))
            } else {
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cardinal-direction navigation: unified two-level cascade with drill-out.
// ---------------------------------------------------------------------------

/// Run the unified cardinal-direction cascade from `focused` in
/// `direction`.
///
/// The cascade has three observable outcomes:
///
/// 1. **Iter 0 — peer match at the focused entry's level.** A scope
///    of the **same kind as `focused`** (leaf for leaf, zone for zone)
///    sharing `focused`'s `parent_zone` and matching the beam test for
///    `direction` wins. This is the common case for in-zone moves
///    (vim's `j`/`k` between cards stacked in a column, the field-row
///    chain inside an inspector panel, the navbar buttons laid out
///    left-to-right).
///
/// 2. **Iter 1 — peer match at the parent's level.** When iter 0 finds
///    nothing, the cascade escalates to `focused.parent_zone` (with a
///    layer-boundary guard — escalation refuses to cross `LayerKey`).
///    The parent is always a zone, so iter 1 searches sibling zones
///    sharing the parent's `parent_zone`. This handles cross-column
///    nav: `Right` from a card lands on the next column zone because
///    the column-zone level is where the horizontal peer chain lives.
///
/// 3. **Drill-out — return the parent zone itself.** When neither
///    iter 0 nor iter 1 finds an in-beam peer, the cascade returns the
///    parent zone's moniker. The user never gets stuck on a key press
///    unless the focused entry sits at the very root of its layer
///    (`focused.parent_zone == None`) — in that case the cascade
///    returns `focused_moniker` (semantic "stay put"; not traced).
///    A torn parent reference (`parent_zone` points at an unregistered
///    or wrong-kind scope) is a kernel bug surface — the cascade
///    returns `focused_moniker` AND emits `tracing::error!`.
///
/// # Same-kind candidate filtering at iter 0
///
/// At iter 0 the candidate set is filtered to the same kind as
/// `focused`. The reason is concrete: in production, a `<FocusScope>`
/// leaf card and a `<FocusZone>` field can both be registered with the
/// **same `parent_zone`** (the enclosing column), because field zones
/// mounted inside the card body inherit the same enclosing zone — the
/// card body is a leaf and so is invisible to the `parent_zone` walk.
/// If the cascade pulled both kinds into iter 0, pressing Down from a
/// focused card could land on a field zone inside the next card
/// (vertically aligned because the field is *inside* the card),
/// stealing the press from the card-leaf neighbor. Same-kind filtering
/// preserves the user's mental model: pressing Down from a card lands
/// on the next card, not on a zone *inside* the next card. If a
/// consumer wants to descend into a card's contents, they drill in
/// (Enter / `<spatial_drill_in>`) instead.
///
/// At iter 1 the candidates are inherently zones because the parent of
/// any registered scope is a `FocusZone`. The same-kind filter still
/// applies (zones only) but is a no-op against a zone-only candidate
/// pool.
///
/// # Why one cascade, not separate leaf / zone paths
///
/// The unified-policy supersession card
/// [`01KQ7S6WHK9RCCG2R4FN474EFD`] collapsed three previously separate
/// rules — leaf rule 1 (in-zone leaves), leaf rule 2 (cross-zone leaf
/// fallback), zone-only sibling-zone beam — into this single cascade.
/// All three are subsumed by "search same-kind peers, escalate, search
/// parent's same-kind peers, fall back to parent". See the module docs
/// for the full rationale.
///
/// [`01KQ7S6WHK9RCCG2R4FN474EFD`]: # "unified-policy supersession card"
fn cardinal_cascade(
    reg: &SpatialRegistry,
    focused: &RegisteredScope,
    focused_moniker: &Moniker,
    direction: Direction,
) -> Moniker {
    let focused_is_zone = focused.is_zone();

    // Iter 0: same-kind peers of the focused entry sharing its
    // parent_zone.
    if let Some(target) = beam_among_siblings(
        reg,
        focused.layer_key(),
        focused.rect(),
        focused.parent_zone(),
        focused.key(),
        focused_is_zone,
        direction,
    ) {
        return target;
    }

    // Escalate. The layer-boundary guard refuses to cross `LayerKey` —
    // an inspector layer's panel zone never lifts focus into the
    // window layer that hosts ui:board.
    //
    // Two distinct "no parent" cases here, distinguished for tracing:
    //
    // - Layer-root edge (`focused.parent_zone() == None`): well-formed,
    //   the focused entry sits at the very top of its layer. Stay put.
    // - Torn parent reference (`parent_zone == Some(k)` but no zone is
    //   registered under `k`, or it's in a different layer): kernel
    //   inconsistency. Trace and stay put.
    let parent = match parent_zone_resolution(reg, focused) {
        ParentResolution::Found(zone) => zone,
        ParentResolution::LayerRoot => {
            // Well-formed edge — no parent zone to drill out to. Stay
            // put without tracing.
            return focused_moniker.clone();
        }
        ParentResolution::Torn { parent_key } => {
            tracing::error!(
                op = "nav",
                focused_key = %focused.key(),
                focused_moniker = %focused_moniker,
                parent_zone_key = %parent_key,
                ?direction,
                "parent_zone references unregistered or cross-layer scope"
            );
            return focused_moniker.clone();
        }
    };

    // Iter 1: same-kind peers of the parent zone sharing its
    // parent_zone. The parent is always a zone, so this is the
    // sibling-zone beam.
    if let Some(target) = beam_among_siblings(
        reg,
        &parent.layer_key,
        &parent.rect,
        parent.parent_zone.as_ref(),
        &parent.key,
        true, /* parent is always a zone */
        direction,
    ) {
        return target;
    }

    // Drill-out fallback: return the parent zone itself. A single key
    // press moves at most one zone level out from the focused entry.
    parent.moniker.clone()
}

/// Outcome of resolving a focused scope's parent zone, distinguishing
/// the well-formed "no parent" edge from torn-state inconsistencies.
///
/// The cascade reads this to decide whether escalation is possible
/// ([`ParentResolution::Found`]), to terminate silently at a layer root
/// ([`ParentResolution::LayerRoot`]), or to terminate noisily on torn
/// state ([`ParentResolution::Torn`]). The distinction is the kernel's
/// no-silent-dropout principle in action: well-formed edges are silent,
/// kernel-inconsistency edges trace.
enum ParentResolution<'a> {
    /// Parent zone resolved cleanly within the same layer.
    Found(&'a FocusZone),
    /// Focused scope sits at the layer root (`parent_zone = None`).
    /// Well-formed; the cascade should stay put without tracing.
    LayerRoot,
    /// `parent_zone` references a key that is unregistered, registered
    /// as a leaf rather than a zone, or in a different layer. The
    /// cascade should stay put AND trace before returning, since this
    /// is a kernel-inconsistency surface (race during virtualizer
    /// remount, stale registration, layer-boundary violation).
    Torn { parent_key: SpatialKey },
}

/// Resolve the focused entry's parent zone, enforcing the layer-
/// boundary guard and distinguishing layer-root edges from torn state.
///
/// See [`ParentResolution`] for the variant semantics. Compared to a
/// plain `Option<&FocusZone>`, this enum lets the cascade trace torn
/// state without false-positive tracing on the well-formed layer-root
/// edge (a leaf at the layer root is a normal shape, not a bug).
fn parent_zone_resolution<'a>(
    reg: &'a SpatialRegistry,
    focused: &RegisteredScope,
) -> ParentResolution<'a> {
    let Some(parent_key) = focused.parent_zone() else {
        return ParentResolution::LayerRoot;
    };
    let Some(parent) = reg.zone(parent_key) else {
        // `parent_zone` names a key, but nothing is registered there
        // (or it's registered as a leaf). Torn state.
        return ParentResolution::Torn {
            parent_key: parent_key.clone(),
        };
    };
    if parent.layer_key != *focused.layer_key() {
        // `parent_zone` resolves but lives in a different layer — a
        // layer-boundary violation. Treat as torn state so the
        // discrepancy is logged.
        return ParentResolution::Torn {
            parent_key: parent_key.clone(),
        };
    }
    ParentResolution::Found(parent)
}

/// Beam-search candidates that share `from_parent` (excluding `from_key`),
/// filtered by `layer` and by kind matching `expect_zone`.
///
/// The cascade calls this twice: once for the focused entry's level
/// (iter 0) and, on no match, once for the parent zone's level (iter 1
/// after escalation). At each level the candidate set is restricted to
/// scopes of the same kind as the search origin (`expect_zone == true`
/// → zones only, `expect_zone == false` → leaves only).
///
/// # Why same-kind filtering
///
/// Mixing leaves and zones in a single candidate pool produces
/// surprising results in production: a `<Field>` zone mounted inside
/// a `<FocusScope>` card body inherits the card's enclosing
/// `parent_zone` (the column), so the field zone and the card leaf are
/// registered as siblings even though visually the field is *inside*
/// the card. Pressing Down from the focused card would land on a field
/// zone in the *next* card rather than the next card itself, stealing
/// the press from the card-leaf neighbor. Same-kind filtering keeps
/// in-zone leaf navigation moving between leaves and in-zone zone
/// navigation moving between zones; consumers cross the kind boundary
/// via drill-in / drill-out, not via cardinal-direction nav.
///
/// # Layer boundary
///
/// Layer is the absolute boundary: every candidate must satisfy
/// `candidate.layer_key == layer`. This filter applies even when
/// `from_parent` is `None` (the layer root): only scopes at the same
/// root level in the same layer are candidates.
///
/// `from_key` is the candidate to exclude from the result set — when
/// the cascade calls this from iter 1 with the parent's identity, the
/// parent zone itself is not a peer of itself and must not be
/// considered.
fn beam_among_siblings(
    reg: &SpatialRegistry,
    layer: &LayerKey,
    from_rect: &Rect,
    from_parent: Option<&SpatialKey>,
    from_key: &SpatialKey,
    expect_zone: bool,
    direction: Direction,
) -> Option<Moniker> {
    pick_best_candidate(
        from_rect,
        direction,
        reg.entries_in_layer(layer).filter_map(|s| {
            if s.is_zone() != expect_zone {
                return None;
            }
            if s.parent_zone() == from_parent && s.key() != from_key {
                Some((s.moniker(), *s.rect()))
            } else {
                None
            }
        }),
    )
}

// ---------------------------------------------------------------------------
// Edge commands: First / Last / RowStart / RowEnd.
//
// Edge commands keep their level-bounded behavior — no escalation
// cascade for those. The drill-out semantics are specific to cardinal
// directions where the user is steering visually; row/page commands
// stay where they are.
// ---------------------------------------------------------------------------

/// Run an edge command from `focused` in `direction`.
///
/// Candidates are same-kind scopes sharing `focused`'s `parent_zone`
/// **including the focused entry itself**; the chosen candidate is the
/// boundary one per `direction`. Including the focused entry in the
/// candidate set makes "already at boundary" a no-op: when the focused
/// scope is itself the topmost-leftmost in its zone,
/// [`Direction::First`] picks it, and the resolver in
/// [`crate::state::SpatialState::navigate_with`] short-circuits via the
/// "already focused → no event" check in
/// [`crate::state::SpatialState::focus`].
///
/// Edge commands always include the focused entry in their candidate
/// pool, so the candidate set is non-empty whenever `focused` is
/// registered. The cascade therefore always has a winner to return —
/// `focused_moniker` is the fallback only when the helper fails
/// internally (cardinal-direction guard short-circuit, vacuous
/// candidate set).
///
/// The kind filter mirrors the cascade's iter-0 same-kind filter
/// (`leaves only` for a leaf-focused entry, `zones only` for a zone-
/// focused entry). See [`beam_among_siblings`] for the rationale on
/// why mixing kinds in a single candidate pool surprises users.
fn edge_command(
    reg: &SpatialRegistry,
    focused: &RegisteredScope,
    focused_moniker: &Moniker,
    direction: Direction,
) -> Moniker {
    let layer = focused.layer_key();
    let from_rect = focused.rect();
    let from_parent = focused.parent_zone();
    let expect_zone = focused.is_zone();

    let candidates = reg.entries_in_layer(layer).filter_map(|s| {
        if s.is_zone() != expect_zone {
            return None;
        }
        if s.parent_zone() == from_parent {
            Some((s.moniker(), *s.rect()))
        } else {
            None
        }
    });
    edge_command_from_candidates(from_rect, direction, candidates)
        .unwrap_or_else(|| focused_moniker.clone())
}

/// Pick the boundary candidate from `candidates` per `direction`.
///
/// `First` / `Last` use the topmost-leftmost / bottommost-rightmost
/// rect ordering. `RowStart` / `RowEnd` filter to candidates whose
/// vertical extent overlaps `from`, then pick the leftmost / rightmost.
///
/// Candidate iterators yield borrowed [`Moniker`] references; only the
/// chosen winner is cloned.
fn edge_command_from_candidates<'a>(
    from_rect: &Rect,
    direction: Direction,
    candidates: impl Iterator<Item = (&'a Moniker, Rect)>,
) -> Option<Moniker> {
    match direction {
        Direction::First => {
            // Topmost first; ties broken by leftmost.
            candidates
                .min_by(|(_, a), (_, b)| {
                    pixels_cmp(a.top(), b.top()).then(pixels_cmp(a.left(), b.left()))
                })
                .map(|(m, _)| m.clone())
        }
        Direction::Last => {
            // Bottommost first; ties broken by rightmost.
            candidates
                .max_by(|(_, a), (_, b)| {
                    pixels_cmp(a.top(), b.top()).then(pixels_cmp(a.left(), b.left()))
                })
                .map(|(m, _)| m.clone())
        }
        Direction::RowStart => candidates
            .filter(|(_, r)| vertical_overlap(from_rect, r))
            .min_by(|(_, a), (_, b)| pixels_cmp(a.left(), b.left()))
            .map(|(m, _)| m.clone()),
        Direction::RowEnd => candidates
            .filter(|(_, r)| vertical_overlap(from_rect, r))
            .max_by(|(_, a), (_, b)| pixels_cmp(a.left(), b.left()))
            .map(|(m, _)| m.clone()),
        // Cardinal directions never reach this helper.
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => None,
    }
}

// ---------------------------------------------------------------------------
// Beam math: candidate filtering, scoring, picking.
// ---------------------------------------------------------------------------

/// Score one candidate against the focused rect for a cardinal
/// direction.
///
/// Returns:
/// - `None` if the candidate is on the wrong side of `from` (e.g.
///   not "below" when navigating `Down`) or its rect collapses into
///   `from` along the major axis.
/// - `Some((in_beam, score))` otherwise. Lower `score` is better.
///
/// The `in_beam` flag reports whether the candidate overlaps `from`
/// on the cross axis (horizontal extent for `Up`/`Down`, vertical
/// extent for `Left`/`Right`). [`pick_best_candidate`] uses it as a
/// **hard filter**: out-of-beam candidates are dropped before any
/// score comparison runs. See [`pick_best_candidate`] for the
/// rationale on hard-filtering rather than tier-preferring.
fn score_candidate(from: &Rect, cand: &Rect, direction: Direction) -> Option<(bool, f64)> {
    let (major, minor, in_beam) = match direction {
        Direction::Down => {
            // Candidate must be strictly below: its top edge sits at
            // or below `from.bottom` OR it strictly extends below
            // `from.bottom`. The strict-extends case handles the
            // common UI pattern of overlapping rects (e.g. a header
            // row whose status pill nestles up against the title row).
            let major = cand.top() - from.bottom();
            if major.value() < 0.0 && cand.bottom().value() <= from.bottom().value() {
                return None;
            }
            // Major: distance along the y axis. We use the leading-
            // edge gap when positive; when the candidate overlaps,
            // we fall back to the gap between centers so the score
            // grows with how much further the candidate sits.
            let major = if major.value() >= 0.0 {
                major
            } else {
                center_y(cand) - center_y(from)
            };
            let minor = cross_axis_minor(from, cand, MinorAxis::Horizontal);
            let in_beam = horizontal_overlap(from, cand);
            (major, minor, in_beam)
        }
        Direction::Up => {
            let major = from.top() - cand.bottom();
            if major.value() < 0.0 && cand.top().value() >= from.top().value() {
                return None;
            }
            let major = if major.value() >= 0.0 {
                major
            } else {
                center_y(from) - center_y(cand)
            };
            let minor = cross_axis_minor(from, cand, MinorAxis::Horizontal);
            let in_beam = horizontal_overlap(from, cand);
            (major, minor, in_beam)
        }
        Direction::Right => {
            let major = cand.left() - from.right();
            if major.value() < 0.0 && cand.right().value() <= from.right().value() {
                return None;
            }
            let major = if major.value() >= 0.0 {
                major
            } else {
                center_x(cand) - center_x(from)
            };
            let minor = cross_axis_minor(from, cand, MinorAxis::Vertical);
            let in_beam = vertical_overlap(from, cand);
            (major, minor, in_beam)
        }
        Direction::Left => {
            let major = from.left() - cand.right();
            if major.value() < 0.0 && cand.left().value() >= from.left().value() {
                return None;
            }
            let major = if major.value() >= 0.0 {
                major
            } else {
                center_x(from) - center_x(cand)
            };
            let minor = cross_axis_minor(from, cand, MinorAxis::Vertical);
            let in_beam = vertical_overlap(from, cand);
            (major, minor, in_beam)
        }
        // Edge commands never reach this helper — they have their own
        // candidate-picking logic.
        Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
            return None;
        }
    };

    // Android's score: `13 * major² + minor²`. Lower is better.
    let major_v = major.value().max(0.0);
    let minor_v = minor.value();
    let score = 13.0 * major_v * major_v + minor_v * minor_v;
    Some((in_beam, score))
}

/// Pick the best candidate from `candidates` for `direction`.
///
/// Cardinal-direction navigation **requires the in-beam test to pass** —
/// out-of-beam candidates are dropped on the floor. The in-beam test is
/// rect projection on the cross axis: a `Down` candidate must overlap
/// the focused rect's horizontal extent, a `Right` candidate must
/// overlap the focused rect's vertical extent, and so on. Among the
/// remaining in-beam candidates, the one with the lowest score
/// (`13 * major² + minor²`) wins — Android's preference for aligned
/// targets, applied as a tie-break inside the in-beam tier.
///
/// # Why in-beam is a hard filter, not a soft tie-break
///
/// The directional-nav card [`01KQ7STZN3G5N2WB3FF4PM4DKX`] surfaced this
/// rule at the realistic-app fixture in
/// `swissarmyhammer-focus/tests/card_directional_nav.rs`: from the top
/// card in the rightmost column (`task:T1C`), pressing `right` should
/// return `None`, but the kernel was instead picking up an out-of-beam
/// `ui:navbar.search` leaf far above and to the right because it was
/// the only direction-valid candidate in the same layer. Letting an
/// out-of-beam candidate win when no in-beam candidate exists creates
/// jarring visual jumps — pressing `right` lifts the user out of the
/// board and into the navbar, which violates the user's mental model
/// of cardinal navigation as "move within the visually-aligned strip".
///
/// The previous version of this helper applied in-beam as a soft
/// preference (in-beam tier wins, out-of-beam tier as fallback). That
/// matched Android's FocusFinder, but for the kanban-app's denser-than-
/// Android layouts (a navbar above a board, both on the same focus
/// layer) the soft preference let visually disconnected candidates win.
/// Tightening to a hard filter is the minimal kernel change that fixes
/// the user-visible bug without disturbing any other test in the suite —
/// every existing test that asserts a specific cardinal-direction target
/// already places that target in-beam, so the assertions hold unchanged.
///
/// # No effect on edge commands
///
/// Edge commands ([`Direction::First`], [`Direction::Last`],
/// [`Direction::RowStart`], [`Direction::RowEnd`]) take a different
/// helper ([`edge_command_from_candidates`]) and are unaffected. This
/// helper is only reached for cardinal directions; the direction
/// parameter is plumbed through so the score formula picks the right
/// axis but the in-beam filter applies uniformly to all four cardinals.
///
/// Candidates carry borrowed [`Moniker`] references so the helper does
/// not allocate per-candidate; only the winning moniker is cloned.
///
/// [`01KQ7STZN3G5N2WB3FF4PM4DKX`]: # "directional-nav supersession card"
fn pick_best_candidate<'a>(
    from_rect: &Rect,
    direction: Direction,
    candidates: impl Iterator<Item = (&'a Moniker, Rect)>,
) -> Option<Moniker> {
    let mut best: Option<(&Moniker, f64)> = None;
    for (moniker, rect) in candidates {
        let Some((in_beam, score)) = score_candidate(from_rect, &rect, direction) else {
            continue;
        };
        // Hard in-beam filter for cardinal directions: an out-of-beam
        // candidate is never a valid answer, even when it is the only
        // direction-valid candidate. The cross-axis projection test
        // (horizontal overlap for Up/Down, vertical overlap for
        // Left/Right) is the kernel's notion of "visually aligned in
        // the requested direction". See the function docs for why this
        // is a hard filter rather than the soft tier preference the
        // implementation used to apply.
        if !in_beam {
            continue;
        }
        match best.as_ref() {
            None => best = Some((moniker, score)),
            Some((_, best_score)) => {
                if score < *best_score {
                    best = Some((moniker, score));
                }
            }
        }
    }
    best.map(|(m, _)| m.clone())
}

// ---------------------------------------------------------------------------
// Geometric helpers.
// ---------------------------------------------------------------------------

/// `true` if two rects overlap on the x axis (their horizontal
/// extents intersect on a non-empty interval).
fn horizontal_overlap(a: &Rect, b: &Rect) -> bool {
    a.left().value() < b.right().value() && b.left().value() < a.right().value()
}

/// `true` if two rects overlap on the y axis (their vertical extents
/// intersect on a non-empty interval). Used by the `Right`/`Left` beam
/// test (where the cross axis is vertical) and by the `RowStart` /
/// `RowEnd` edge commands (which keep the candidate set on the focused
/// row).
fn vertical_overlap(a: &Rect, b: &Rect) -> bool {
    a.top().value() < b.bottom().value() && b.top().value() < a.bottom().value()
}

/// Center x coordinate of a rect.
fn center_x(r: &Rect) -> Pixels {
    r.left() + r.width / 2.0
}

/// Center y coordinate of a rect.
fn center_y(r: &Rect) -> Pixels {
    r.top() + r.height / 2.0
}

/// Which axis is the *minor* (cross) axis of a beam search.
///
/// For `Up` / `Down` navigation the major axis is vertical, so the
/// minor axis is [`MinorAxis::Horizontal`]; for `Left` / `Right` the
/// reverse. Carrying the choice as an enum (instead of a `bool`)
/// preserves the meaning at every call site without the
/// `/* horizontal_minor = */` comment workaround.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MinorAxis {
    /// Minor axis runs horizontally — used for vertical (`Up`/`Down`) navigation.
    Horizontal,
    /// Minor axis runs vertically — used for horizontal (`Left`/`Right`) navigation.
    Vertical,
}

/// Compute the minor-axis distance between two rects.
///
/// The minor distance is the absolute gap between the cross-axis
/// centers — zero when centered, growing with lateral drift.
fn cross_axis_minor(from: &Rect, cand: &Rect, minor_axis: MinorAxis) -> Pixels {
    let (a, b) = match minor_axis {
        MinorAxis::Horizontal => (center_x(from), center_x(cand)),
        MinorAxis::Vertical => (center_y(from), center_y(cand)),
    };
    let raw = a.value() - b.value();
    Pixels::new(raw.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::FocusLayer;
    use crate::scope::FocusScope;
    use crate::types::{LayerKey, LayerName, Pixels, Rect, WindowLabel};
    use std::collections::HashMap;

    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        }
    }

    /// Lonely leaf — nothing else to navigate to. Returns the
    /// focused moniker (semantic "stay put" at the layer root).
    #[test]
    fn beam_returns_focused_moniker_for_known_start_with_no_neighbors() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(FocusLayer {
            key: LayerKey::from_string("L"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string("main"),
            last_focused: None,
        });
        reg.register_scope(FocusScope {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            overrides: HashMap::new(),
        });

        let strategy = BeamNavStrategy::new();
        let focused_moniker = Moniker::from_string("ui:k");
        let result = strategy.next(
            &reg,
            &SpatialKey::from_string("k"),
            &focused_moniker,
            Direction::Right,
        );
        assert_eq!(result, focused_moniker);
    }

    /// Unknown starting key echoes the input moniker — torn state is
    /// surfaced to logs, not as `None`.
    #[test]
    fn beam_returns_focused_moniker_for_unknown_start() {
        let reg = SpatialRegistry::new();
        let strategy = BeamNavStrategy::new();
        let focused_moniker = Moniker::from_string("ui:ghost");
        let result = strategy.next(
            &reg,
            &SpatialKey::from_string("ghost"),
            &focused_moniker,
            Direction::Up,
        );
        assert_eq!(result, focused_moniker);
    }

    /// `score_candidate` rejects candidates on the wrong side of the
    /// focused rect for the requested direction.
    #[test]
    fn score_rejects_wrong_side_candidates() {
        let from = Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        };
        // Candidate strictly above: invalid for Direction::Down.
        let above = Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(-50.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        };
        assert!(score_candidate(&from, &above, Direction::Down).is_none());

        // Candidate strictly to the left: invalid for Direction::Right.
        let to_left = Rect {
            x: Pixels::new(-50.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        };
        assert!(score_candidate(&from, &to_left, Direction::Right).is_none());
    }

    /// In-beam candidate beats out-of-beam candidate regardless of raw
    /// score — Android's beam test as a hard tier.
    #[test]
    fn in_beam_wins_over_out_of_beam() {
        let from = Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(20.0),
            height: Pixels::new(20.0),
        };
        // Out-of-beam, very close.
        let near = Rect {
            x: Pixels::new(50.0),
            y: Pixels::new(30.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        };
        // In-beam, far.
        let far = Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(200.0),
            width: Pixels::new(20.0),
            height: Pixels::new(20.0),
        };

        let (near_in_beam, _) = score_candidate(&from, &near, Direction::Down).unwrap();
        let (far_in_beam, _) = score_candidate(&from, &far, Direction::Down).unwrap();
        assert!(!near_in_beam, "near candidate is laterally offset");
        assert!(far_in_beam, "far candidate is directly below");
    }
}
