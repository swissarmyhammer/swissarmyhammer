//! Pluggable navigation strategy and the default Android-style beam
//! search.
//!
//! [`NavStrategy`] abstracts the algorithm that picks the next focus
//! target given the current registry state, the currently focused
//! [`FullyQualifiedMoniker`] paired with its [`SegmentMoniker`], and
//! the requested [`Direction`]. Consumers that want the default
//! behavior use [`BeamNavStrategy`]; tests and specialised layouts can
//! swap in a custom impl without touching [`SpatialState`].
//!
//! # No-silent-dropout contract
//!
//! Nav and drill APIs always return a [`FullyQualifiedMoniker`]. "No
//! motion possible" is communicated by returning the focused entry's
//! own FQM — the React side detects "stay put" by comparing the
//! returned FQM to the previous focused FQM. Torn state (unknown FQM,
//! orphan parent reference) emits `tracing::error!` and echoes the
//! input FQM so the call site has a valid result. There is no `Option`
//! or `Result` on these APIs; silence is impossible.
//!
//! Two principles distinguish the two non-motion paths:
//!
//! - **No motion → return focused FQM (no trace).** A semantic
//!   "stay put" — wall override, layer-root edge, leaf with no
//!   children, drill-out at root. The kernel returns the focused
//!   entry's own FQM. Observable: focus stays where it was, no
//!   `null` blip on the React side, no log noise.
//! - **Torn state → trace error AND echo input.** A genuine error —
//!   unknown FQM, orphan parent reference, registry inconsistency. The
//!   kernel emits `tracing::error!` with the operation, the relevant
//!   FQM(s), and the FQM being echoed back, then returns the input
//!   FQM so the call site has a valid value. User-observable behavior
//!   is identical to the "no motion" case (focus stays put), but ops /
//!   devs can chase the error in logs.
//!
//! The trait returns a [`FullyQualifiedMoniker`] — the canonical
//! identity. Callers that need the relative segment (for human-readable
//! logs or local-only display) read it from the registry by FQM.
//!
//! # The sibling rule — zones and scopes are siblings under a parent zone
//!
//! Within a parent [`crate::scope::FocusZone`], child
//! [`crate::scope::FocusScope`] leaves and child
//! [`crate::scope::FocusZone`] containers are **siblings**. Cardinal
//! navigation must treat them as peers — never filter by kind at the
//! in-zone (iter 0) level. This is the load-bearing contract of the
//! kernel; see the crate README (`swissarmyhammer-focus/README.md`) for
//! the prose version with diagrams.
//!
//! Translated to algorithm terms:
//!
//! - **Iter 0 — any-kind in-zone peers**: candidates are ANY registered
//!   scope (leaf OR zone) sharing the focused entry's `parent_zone`,
//!   geometrically in `direction`. Pick best by Android beam score.
//!   The kernel must NOT use kind as a candidate filter at iter 0 —
//!   doing so re-introduces the bug where pressing Right from a card's
//!   drag-handle leaf jumps over the title field zone to the inspect
//!   leaf, or pressing Down from a card's chrome leaves the card.
//! - **Iter 1 — same-kind peer zones**: when iter 0 misses, escalate
//!   to the focused entry's `parent_zone` and search its peers. The
//!   parent IS a zone, so its peers are zones by construction — iter
//!   1's same-kind filter is structural, not a kind policy.
//!
//! # Algorithm — unified edge drill-out cascade
//!
//! Within a single layer (the focused entry's `layer_fq` is the hard
//! boundary — nav never crosses it), the cascade runs:
//!
//! 1. **Iter 0 — any-kind in-zone peer search** at the focused entry's
//!    level: candidates are ANY scope (leaf or zone) sharing
//!    `parent_zone` with the focused entry. If a candidate satisfies
//!    the in-beam Android score (`13 * major² + minor²`), return its
//!    FQM. The geometrically best candidate wins regardless of kind.
//! 2. **Escalate** to the focused entry's `parent_zone` (with a
//!    layer-boundary guard — escalation never crosses the layer FQM).
//!    If the focused entry has no parent zone (it sits at the layer
//!    root), the cascade stays put.
//! 3. **Iter 1 — same-kind sibling-zone peer search** at the parent's
//!    level: candidates are zones (the parent is itself a zone, so
//!    iter 1's same-kind filter restricts to zones) sharing the
//!    parent's `parent_zone` — i.e. the focused entry's grandparent in
//!    the zone tree. Same beam scoring. If a candidate matches, return
//!    its FQM.
//! 4. **Drill-out fallback**: when no peer matches at iter 0 *or* iter
//!    1, return the parent zone's FQM. A single key press moves at
//!    most one zone level out from the focused entry; the user is
//!    never "stuck" returning a stay-put unless the focused entry sits
//!    at the very root of its layer.
//!
//! Edge commands ([`Direction::First`], [`Direction::Last`],
//! [`Direction::RowStart`], [`Direction::RowEnd`]) keep their
//! level-bounded behavior AND keep their same-kind filter — `Home` in
//! a row of cells means "first cell in the row", not "the row's
//! container zone". The level-bounded "first/last among siblings of my
//! kind" semantics are correct for those keys.
//!
//! Override (rule 0) still runs first — the focused scope's
//! per-direction `overrides` map short-circuits the cascade entirely.

use crate::registry::SpatialRegistry;
use crate::scope::{FocusZone, RegisteredScope};
use crate::types::{pixels_cmp, Direction, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker};

/// Pluggable navigation algorithm.
///
/// Given the current registry state, the focused [`FullyQualifiedMoniker`]
/// paired with its [`SegmentMoniker`], and a [`Direction`], return the
/// FQM of the next focus target. When motion is not possible (visual
/// edge of the layout, override wall, layer root, or torn-state errors),
/// the strategy returns `focused_fq` itself — never `None`. See the
/// module docs for the no-silent-dropout contract this enables.
///
/// Implementations are `Send + Sync` so adapters can store them behind
/// an `Arc<dyn NavStrategy>` shared across async tasks.
pub trait NavStrategy: Send + Sync {
    /// Pick the next focus target.
    ///
    /// # Parameters
    /// - `registry` — the current registry. Strategies typically read
    ///   [`SpatialRegistry::find_by_fq`] for `focused` to discover its
    ///   rect and layer, then iterate
    ///   [`SpatialRegistry::leaves_in_layer`] /
    ///   [`SpatialRegistry::zones_in_layer`] for candidates.
    /// - `focused_fq` — the FQM of the currently focused scope.
    /// - `focused_segment` — the relative segment paired with
    ///   `focused_fq`. Carried for human-readable logs only — the
    ///   strategy keys on FQMs.
    /// - `direction` — the direction the user pressed.
    ///
    /// # Returns
    /// The FQM of the next focus target. When the strategy has a real
    /// target (peer match, drill-out fallback to a parent zone, override
    /// redirect), that target's FQM is returned. When the strategy
    /// declines (override wall, layer root, unknown key, torn parent
    /// reference) the returned FQM equals `focused_fq` — the call site
    /// detects "stay put" by equality comparison. Torn-state paths
    /// additionally emit `tracing::error!` before returning so the
    /// issue is observable in logs.
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused_fq: &FullyQualifiedMoniker,
        focused_segment: &SegmentMoniker,
        direction: Direction,
    ) -> FullyQualifiedMoniker;
}

/// Default Android-beam-search navigation strategy.
///
/// Implements the unified cascade described in the module docs:
/// override (rule 0) → iter-0 ANY-KIND peer search at the focused
/// entry's level → iter-1 same-kind peer-zone search at the parent's
/// level → drill-out to the parent zone itself when no peer matches
/// at either level. The any-kind iter-0 rule realises the sibling
/// contract: a child [`crate::scope::FocusZone`] and a child
/// [`crate::scope::FocusScope`] under the same parent zone are peers,
/// not segregated kinds.
///
/// Edge commands ([`Direction::First`], [`Direction::Last`],
/// [`Direction::RowStart`], [`Direction::RowEnd`]) keep their
/// level-bounded same-kind behavior — no escalation cascade and `Home`
/// in a row of cells still picks "first cell", not "row container".
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
    /// is filtered by `candidate.layer_fq == focused.layer_fq` before
    /// any scoring runs, and escalation refuses to cross from one
    /// layer FQM to another (the inspector layer is captured-focus).
    ///
    /// # No-silent-dropout contract
    ///
    /// Per the module docs, this method always returns an FQM:
    /// either the next focus target, or `focused_fq` itself when no
    /// motion is possible. An unknown `focused_fq` is treated as torn
    /// state — `tracing::error!` fires and `focused_fq` is echoed back.
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused_fq: &FullyQualifiedMoniker,
        focused_segment: &SegmentMoniker,
        direction: Direction,
    ) -> FullyQualifiedMoniker {
        let Some(entry) = registry.entry(focused_fq) else {
            // Torn state: caller passed an FQM that has no registry
            // entry. Trace the operation and echo the input FQM.
            tracing::error!(
                op = "nav",
                focused_fq = %focused_fq,
                focused_segment = %focused_segment,
                ?direction,
                "unknown focused FQM passed to BeamNavStrategy::next"
            );
            return focused_fq.clone();
        };

        // Rule 0: per-direction override on the focused scope.
        match check_override(registry, entry, direction) {
            Some(Some(target)) => return target,
            Some(None) => {
                // Explicit wall — semantic "stay put", not torn state.
                return focused_fq.clone();
            }
            None => {} // fall through to cascade
        }

        match direction {
            Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
                cardinal_cascade(registry, entry, focused_fq, focused_segment, direction)
            }
            Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
                edge_command(registry, entry, focused_fq, direction)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule 0: per-direction override on the focused scope.
// ---------------------------------------------------------------------------

/// Resolve the per-direction override on `focused`, if any.
///
/// Each registered scope carries a `HashMap<Direction, Option<FullyQualifiedMoniker>>`
/// of navigation overrides. The outer [`Option`] of the return value
/// encodes "did an override apply?", and the inner [`Option<FullyQualifiedMoniker>`]
/// encodes the answer when it did:
///
/// - **`None`** — no entry for `direction` on the focused scope (or the
///   entry names a target that does not resolve in the focused scope's
///   layer). The override didn't apply; the caller must fall through
///   to the beam-search cascade.
/// - **`Some(None)`** — explicit "wall": the override map maps
///   `direction → None`. Navigation is blocked; the strategy returns
///   the focused FQM without consulting beam search.
/// - **`Some(Some(target_fq))`** — redirect: the override map maps
///   `direction → Some(target)` *and* `target` is registered in the
///   focused scope's layer. Returns the target FQM; beam search does
///   not run.
///
/// Layer scoping is enforced here, not at registration: a target that
/// names an FQM registered in a *different* layer is treated as
/// "unresolved" and the override falls through to beam search. Cross-
/// layer teleportation is never allowed, even via override.
fn check_override(
    registry: &SpatialRegistry,
    focused: &RegisteredScope,
    direction: Direction,
) -> Option<Option<FullyQualifiedMoniker>> {
    let entry = focused.overrides().get(&direction)?;
    match entry {
        // Explicit `None` — block navigation in this direction.
        None => Some(None),
        // `Some(target_fq)` — resolve only within the focused scope's layer.
        // A target in a different layer (or unregistered entirely) makes
        // the override fall through to beam search.
        Some(target) => {
            let target_in_layer = registry
                .entries_in_layer(focused.layer_fq())
                .any(|s| s.fq() == target);
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
/// See the module-level docstring for the full cascade contract; this
/// helper composes the three observable outcomes (iter 0, iter 1,
/// drill-out fallback) plus the layer-root and torn-state edges.
///
/// Iter 0 uses [`beam_among_in_zone_any_kind`] — any sibling of the
/// focused entry under the same `parent_zone`, regardless of kind.
/// Iter 1 uses [`beam_among_siblings`] with `expect_zone = true` —
/// only sibling zones of the parent zone enter the search, which is
/// structural (the parent is itself a zone), not a kind policy.
///
/// See `swissarmyhammer-focus/README.md` for the prose contract.
fn cardinal_cascade(
    reg: &SpatialRegistry,
    focused: &RegisteredScope,
    focused_fq: &FullyQualifiedMoniker,
    focused_segment: &SegmentMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    // Iter 0: ANY-kind peers of the focused entry sharing its
    // parent_zone. A child FocusZone and a child FocusScope under the
    // same parent FocusZone are peers — the geometrically best
    // candidate wins regardless of kind.
    if let Some(target) = beam_among_in_zone_any_kind(
        reg,
        focused.layer_fq(),
        focused.rect(),
        focused.parent_zone(),
        focused.fq(),
        direction,
    ) {
        return target;
    }

    // Escalate. The layer-boundary guard refuses to cross the layer
    // FQM — an inspector layer's panel zone never lifts focus into the
    // window layer that hosts ui:board.
    let parent = match parent_zone_resolution(reg, focused) {
        ParentResolution::Found(zone) => zone,
        ParentResolution::LayerRoot => {
            // Well-formed edge — no parent zone to drill out to. Stay
            // put without tracing.
            return focused_fq.clone();
        }
        ParentResolution::Torn { parent_fq } => {
            tracing::error!(
                op = "nav",
                focused_fq = %focused.fq(),
                focused_segment = %focused_segment,
                parent_zone_fq = %parent_fq,
                ?direction,
                "parent_zone references unregistered or cross-layer scope"
            );
            return focused_fq.clone();
        }
    };

    // Iter 1: same-kind peers of the parent zone sharing its
    // parent_zone. The parent IS always a zone, so this restricts
    // candidates to zones — structural fact, not a kind policy.
    if let Some(target) = beam_among_siblings(
        reg,
        &parent.layer_fq,
        &parent.rect,
        parent.parent_zone.as_ref(),
        &parent.fq,
        true, /* parent is always a zone */
        direction,
    ) {
        return target;
    }

    // Drill-out fallback: return the parent zone's FQM. A single key
    // press moves at most one zone level out from the focused entry.
    parent.fq.clone()
}

/// Outcome of resolving a focused scope's parent zone, distinguishing
/// the well-formed "no parent" edge from torn-state inconsistencies.
enum ParentResolution<'a> {
    /// Parent zone resolved cleanly within the same layer.
    Found(&'a FocusZone),
    /// Focused scope sits at the layer root (`parent_zone = None`).
    /// Well-formed; the cascade should stay put without tracing.
    LayerRoot,
    /// `parent_zone` references an FQM that is unregistered, registered
    /// as a leaf rather than a zone, or in a different layer. The
    /// cascade should stay put AND trace before returning.
    Torn { parent_fq: FullyQualifiedMoniker },
}

/// Resolve the focused entry's parent zone, enforcing the layer-
/// boundary guard and distinguishing layer-root edges from torn state.
fn parent_zone_resolution<'a>(
    reg: &'a SpatialRegistry,
    focused: &RegisteredScope,
) -> ParentResolution<'a> {
    let Some(parent_fq) = focused.parent_zone() else {
        return ParentResolution::LayerRoot;
    };
    let Some(parent) = reg.zone(parent_fq) else {
        // `parent_zone` names an FQM, but nothing is registered there
        // (or it's registered as a leaf). Torn state.
        return ParentResolution::Torn {
            parent_fq: parent_fq.clone(),
        };
    };
    if parent.layer_fq != *focused.layer_fq() {
        // `parent_zone` resolves but lives in a different layer — a
        // layer-boundary violation. Treat as torn state so the
        // discrepancy is logged.
        return ParentResolution::Torn {
            parent_fq: parent_fq.clone(),
        };
    }
    ParentResolution::Found(parent)
}

/// Beam-search candidates that share `from_parent` (excluding `from_fq`),
/// filtered by `layer` and by kind matching `expect_zone`.
///
/// **Caller contract — used by iter 1 only.** The same-kind filter on
/// this helper is appropriate when the focused entry is the parent
/// zone in the iter-1 escalation: the parent IS a zone, so peer zones
/// of the parent are zones by construction. Restricting candidates to
/// `expect_zone == true` is structural at iter 1, NOT a kind policy.
///
/// Iter 0 must NOT use this helper — iter 0 is any-kind in-zone (see
/// [`beam_among_in_zone_any_kind`]). The `expect_zone` parameter is
/// retained because edge commands ([`edge_command`]) also rely on
/// kind-bounded candidates by design (`Home` in a row of cells means
/// "first cell", not "row container"); but cardinal nav's iter 0 has
/// no business filtering by kind.
///
/// See `swissarmyhammer-focus/README.md` for the prose contract.
fn beam_among_siblings(
    reg: &SpatialRegistry,
    layer: &FullyQualifiedMoniker,
    from_rect: &Rect,
    from_parent: Option<&FullyQualifiedMoniker>,
    from_fq: &FullyQualifiedMoniker,
    expect_zone: bool,
    direction: Direction,
) -> Option<FullyQualifiedMoniker> {
    pick_best_candidate(
        from_rect,
        direction,
        reg.entries_in_layer(layer).filter_map(|s| {
            if s.is_zone() != expect_zone {
                return None;
            }
            if s.parent_zone() == from_parent && s.fq() != from_fq {
                Some((s.fq(), *s.rect()))
            } else {
                None
            }
        }),
    )
}

/// Beam-search ANY-KIND candidates sharing `from_parent` (excluding
/// `from_fq`), filtered by `layer` only.
///
/// **Caller contract — used by iter 0 only.** Implements the sibling
/// rule: within a parent [`crate::scope::FocusZone`], child
/// [`crate::scope::FocusScope`] leaves and child
/// [`crate::scope::FocusZone`] containers are peers. Cardinal nav at
/// iter 0 considers both kinds together and lets the Android beam
/// score pick the geometrically best candidate.
///
/// See `swissarmyhammer-focus/README.md` for the prose contract — and
/// for the anti-pattern callout warning future contributors NOT to
/// re-introduce a `is_zone()` filter at iter 0.
fn beam_among_in_zone_any_kind(
    reg: &SpatialRegistry,
    layer: &FullyQualifiedMoniker,
    from_rect: &Rect,
    from_parent: Option<&FullyQualifiedMoniker>,
    from_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> Option<FullyQualifiedMoniker> {
    pick_best_candidate(
        from_rect,
        direction,
        reg.entries_in_layer(layer).filter_map(|s| {
            if s.parent_zone() == from_parent && s.fq() != from_fq {
                Some((s.fq(), *s.rect()))
            } else {
                None
            }
        }),
    )
}

// ---------------------------------------------------------------------------
// Edge commands: First / Last / RowStart / RowEnd.
// ---------------------------------------------------------------------------

/// Run an edge command from `focused` in `direction`.
fn edge_command(
    reg: &SpatialRegistry,
    focused: &RegisteredScope,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    let layer = focused.layer_fq();
    let from_rect = focused.rect();
    let from_parent = focused.parent_zone();
    let expect_zone = focused.is_zone();

    let candidates = reg.entries_in_layer(layer).filter_map(|s| {
        if s.is_zone() != expect_zone {
            return None;
        }
        if s.parent_zone() == from_parent {
            Some((s.fq(), *s.rect()))
        } else {
            None
        }
    });
    edge_command_from_candidates(from_rect, direction, candidates)
        .unwrap_or_else(|| focused_fq.clone())
}

/// Pick the boundary candidate from `candidates` per `direction`.
fn edge_command_from_candidates<'a>(
    from_rect: &Rect,
    direction: Direction,
    candidates: impl Iterator<Item = (&'a FullyQualifiedMoniker, Rect)>,
) -> Option<FullyQualifiedMoniker> {
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
fn score_candidate(from: &Rect, cand: &Rect, direction: Direction) -> Option<(bool, f64)> {
    let (major, minor, in_beam) = match direction {
        Direction::Down => {
            let major = cand.top() - from.bottom();
            if major.value() < 0.0 && cand.bottom().value() <= from.bottom().value() {
                return None;
            }
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
/// out-of-beam candidates are dropped on the floor.
fn pick_best_candidate<'a>(
    from_rect: &Rect,
    direction: Direction,
    candidates: impl Iterator<Item = (&'a FullyQualifiedMoniker, Rect)>,
) -> Option<FullyQualifiedMoniker> {
    let mut best: Option<(&FullyQualifiedMoniker, f64)> = None;
    for (fq, rect) in candidates {
        let Some((in_beam, score)) = score_candidate(from_rect, &rect, direction) else {
            continue;
        };
        if !in_beam {
            continue;
        }
        match best.as_ref() {
            None => best = Some((fq, score)),
            Some((_, best_score)) => {
                if score < *best_score {
                    best = Some((fq, score));
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
/// intersect on a non-empty interval).
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MinorAxis {
    /// Minor axis runs horizontally — used for vertical (`Up`/`Down`) navigation.
    Horizontal,
    /// Minor axis runs vertically — used for horizontal (`Left`/`Right`) navigation.
    Vertical,
}

/// Compute the minor-axis distance between two rects.
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
    use crate::types::{
        FullyQualifiedMoniker, LayerName, Pixels, Rect, SegmentMoniker, WindowLabel,
    };
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
    /// focused FQM (semantic "stay put" at the layer root).
    #[test]
    fn beam_returns_focused_fq_for_known_start_with_no_neighbors() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(FocusLayer {
            fq: FullyQualifiedMoniker::from_string("/L"),
            segment: SegmentMoniker::from_string("L"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string("main"),
            last_focused: None,
        });
        reg.register_scope(FocusScope {
            fq: FullyQualifiedMoniker::from_string("/L/k"),
            segment: SegmentMoniker::from_string("k"),
            rect: rect_zero(),
            layer_fq: FullyQualifiedMoniker::from_string("/L"),
            parent_zone: None,
            overrides: HashMap::new(),
        });

        let strategy = BeamNavStrategy::new();
        let focused_fq = FullyQualifiedMoniker::from_string("/L/k");
        let focused_segment = SegmentMoniker::from_string("k");
        let result = strategy.next(&reg, &focused_fq, &focused_segment, Direction::Right);
        assert_eq!(result, focused_fq);
    }

    /// Unknown starting FQM echoes the input — torn state is surfaced
    /// to logs, not as `None`.
    #[test]
    fn beam_returns_focused_fq_for_unknown_start() {
        let reg = SpatialRegistry::new();
        let strategy = BeamNavStrategy::new();
        let focused_fq = FullyQualifiedMoniker::from_string("/ghost");
        let focused_segment = SegmentMoniker::from_string("ghost");
        let result = strategy.next(&reg, &focused_fq, &focused_segment, Direction::Up);
        assert_eq!(result, focused_fq);
    }
}
