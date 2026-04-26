//! Pluggable navigation strategy and the default Android-style beam
//! search.
//!
//! [`NavStrategy`] abstracts the algorithm that picks the next focus
//! target given the current registry state, the currently focused
//! [`SpatialKey`], and the requested [`Direction`]. Consumers that
//! want the default behavior use [`BeamNavStrategy`]; tests and
//! specialised layouts can swap in a custom impl without touching
//! [`SpatialState`].
//!
//! The trait returns a [`Moniker`] rather than a [`SpatialKey`] so
//! consumers that key off entity identity (the same reason
//! [`crate::state::FocusChangedEvent::next_moniker`] exists) can act on
//! the result without an extra reverse-lookup through the registry.
//!
//! # Algorithm — three-rule cascade plus zone-level nav
//!
//! Within a single layer (the focused entry's `layer_key` is the hard
//! boundary — nav never crosses it), beam search runs in priority
//! order:
//!
//! 1. **Within-zone beam** — candidates restricted to siblings whose
//!    `parent_zone` matches the focused leaf's `parent_zone`. Beam
//!    test + Android scoring (`13 * major² + minor²`).
//! 2. **Cross-zone leaf fallback** — when rule 1 finds nothing, every
//!    [`Focusable`] in the same layer becomes a candidate. Same beam
//!    test + scoring.
//! 3. **No-op** — when both fail, return `None`.
//!
//! When the focused entry is itself a [`FocusZone`] (the user drilled
//! out), the candidate set is **sibling zones only** — leaves are
//! invisible at this level. Same beam test + scoring against the
//! restricted set.
//!
//! Edge commands ([`Direction::First`], [`Direction::Last`],
//! [`Direction::RowStart`], [`Direction::RowEnd`]) bypass beam search
//! and instead pick the boundary candidate from a level-aware set.
//!
//! # Scoring rationale
//!
//! Android's FocusFinder weights `major² * 13 + minor²` so a perfectly
//! aligned candidate (zero minor) beats a closer-but-diagonal one. The
//! beam test (a candidate's rect must overlap the focused rect's
//! cross-axis projection) acts as the primary filter; in-beam
//! candidates always beat out-of-beam candidates regardless of raw
//! distance.

use crate::registry::SpatialRegistry;
use crate::scope::{FocusScope, FocusZone, Focusable};
use crate::types::{pixels_cmp, Direction, LayerKey, Moniker, Pixels, Rect, SpatialKey};

/// Pluggable navigation algorithm.
///
/// Given the current registry state, the focused [`SpatialKey`], and a
/// [`Direction`], return the [`Moniker`] of the next focus target —
/// or `None` when no candidate exists in that direction (visual edge
/// of the layout, or the strategy declines to navigate).
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
    /// - `direction` — the direction the user pressed.
    ///
    /// # Returns
    /// - `Some(Moniker)` when the strategy has a target.
    /// - `None` when no candidate exists or the strategy declines.
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused: &SpatialKey,
        direction: Direction,
    ) -> Option<Moniker>;
}

/// Default Android-beam-search navigation strategy.
///
/// Implements the three-rule cascade described in the module docs:
/// within-zone beam first, cross-zone leaf fallback second, no-op
/// third. Zone-level nav (focused entry is a `FocusZone`) restricts
/// candidates to sibling zones in the same layer.
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
    /// three-rule beam-search cascade fires (within-zone, then cross-
    /// zone leaf fallback for leaves; sibling-zone beam for zones).
    /// Layer is the absolute boundary throughout — every candidate set
    /// is filtered by `candidate.layer_key == focused.layer_key` before
    /// any scoring runs.
    fn next(
        &self,
        registry: &SpatialRegistry,
        focused: &SpatialKey,
        direction: Direction,
    ) -> Option<Moniker> {
        let scope = registry.scope(focused)?;
        let layer = scope.layer_key();

        // Rule 0: per-direction override on the focused scope.
        //
        // The outer `Option` distinguishes "did the override apply?":
        //   - `Some(_)` → override fired; its inner value (target or
        //     `None` wall) is the answer; beam search does **not** run.
        //   - `None`    → override did not apply; fall through to the
        //     beam-search cascade below.
        if let Some(result) = check_override(registry, scope, direction) {
            return result;
        }

        match scope {
            FocusScope::Focusable(f) => navigate_leaf(registry, f, direction, layer),
            FocusScope::Zone(z) => navigate_zone(registry, z, direction, layer),
        }
    }
}

// ---------------------------------------------------------------------------
// Rule 0: per-direction override on the focused scope.
// ---------------------------------------------------------------------------

/// Resolve the per-direction override on `focused`, if any.
///
/// Each [`FocusScope`] carries a `HashMap<Direction, Option<Moniker>>`
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
/// layer teleportation is never allowed, even via override — see the
/// rationale on [`SpatialRegistry::scopes_in_layer`].
fn check_override(
    registry: &SpatialRegistry,
    focused: &FocusScope,
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
                .scopes_in_layer(focused.layer_key())
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
// Leaf-level navigation: three-rule cascade.
// ---------------------------------------------------------------------------

/// Run leaf-level navigation from `from` in `direction`.
///
/// Cardinal directions execute the three-rule cascade (within-zone
/// beam, then cross-zone leaf fallback, then `None`). Edge commands
/// pick the boundary candidate from the leaf's `parent_zone` siblings.
fn navigate_leaf(
    reg: &SpatialRegistry,
    from: &Focusable,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    match direction {
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
            if let Some(m) = beam_in_zone(reg, from, direction, layer) {
                return Some(m);
            }
            beam_all_leaves_in_layer(reg, from, direction, layer)
        }
        Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
            edge_command_for_leaf(reg, from, direction, layer)
        }
    }
}

/// Rule 1: within-zone beam search.
///
/// Candidates are [`Focusable`] entries with matching `layer_key` and
/// matching `parent_zone` (same enclosing zone as the focused leaf).
/// `FocusZone` entries are not candidates at leaf level.
fn beam_in_zone(
    reg: &SpatialRegistry,
    from: &Focusable,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    pick_best_candidate(
        &from.rect,
        direction,
        reg.scopes_in_layer(layer).filter_map(|s| match s {
            FocusScope::Focusable(f) if f.parent_zone == from.parent_zone && f.key != from.key => {
                Some((&f.moniker, f.rect))
            }
            _ => None,
        }),
    )
}

/// Rule 2: cross-zone leaf fallback.
///
/// Candidates are every [`Focusable`] in the same layer (regardless of
/// `parent_zone`). Used when rule 1 finds nothing — makes
/// `nav.right` across columns work naturally.
fn beam_all_leaves_in_layer(
    reg: &SpatialRegistry,
    from: &Focusable,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    pick_best_candidate(
        &from.rect,
        direction,
        reg.scopes_in_layer(layer).filter_map(|s| match s {
            FocusScope::Focusable(f) if f.key != from.key => Some((&f.moniker, f.rect)),
            _ => None,
        }),
    )
}

// ---------------------------------------------------------------------------
// Zone-level navigation: sibling zones only.
// ---------------------------------------------------------------------------

/// Run zone-level navigation from `from` in `direction`.
///
/// Candidates are restricted to **sibling zones** — `FocusZone` entries
/// in the same layer with matching `parent_zone`. Leaves are invisible
/// at this level. Edge commands operate on the same restricted set.
fn navigate_zone(
    reg: &SpatialRegistry,
    from: &FocusZone,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    match direction {
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
            beam_sibling_zones(reg, from, direction, layer)
        }
        Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
            edge_command_for_zone(reg, from, direction, layer)
        }
    }
}

/// Beam search restricted to sibling zones — same `layer_key`, same
/// `parent_zone` as the focused zone.
fn beam_sibling_zones(
    reg: &SpatialRegistry,
    from: &FocusZone,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    pick_best_candidate(
        &from.rect,
        direction,
        reg.scopes_in_layer(layer).filter_map(|s| match s {
            FocusScope::Zone(z) if z.parent_zone == from.parent_zone && z.key != from.key => {
                Some((&z.moniker, z.rect))
            }
            _ => None,
        }),
    )
}

// ---------------------------------------------------------------------------
// Edge commands: First / Last / RowStart / RowEnd.
// ---------------------------------------------------------------------------

/// Edge command for a leaf. Candidates are in-zone siblings (same
/// `parent_zone`) **including the focused leaf itself**; the chosen
/// candidate is the boundary one per `direction`.
///
/// Including `from` in the candidate set makes "already at boundary" a
/// no-op: when the focused leaf is itself the topmost-leftmost in its
/// zone, `Direction::First` picks it, and the resolver in
/// [`crate::state::SpatialState::navigate_with`] short-circuits via the
/// "already focused → no event" check in [`crate::state::SpatialState::focus`].
fn edge_command_for_leaf(
    reg: &SpatialRegistry,
    from: &Focusable,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    let candidates = reg.scopes_in_layer(layer).filter_map(|s| match s {
        FocusScope::Focusable(f) if f.parent_zone == from.parent_zone => Some((&f.moniker, f.rect)),
        _ => None,
    });
    edge_command_from_candidates(&from.rect, direction, candidates)
}

/// Edge command for a zone. Candidates are sibling zones (same
/// `parent_zone`) **including the focused zone itself**; the chosen
/// candidate is the boundary one per `direction`.
///
/// Including `from` in the candidate set makes "already at boundary" a
/// no-op: see [`edge_command_for_leaf`] for the same rationale.
fn edge_command_for_zone(
    reg: &SpatialRegistry,
    from: &FocusZone,
    direction: Direction,
    layer: &LayerKey,
) -> Option<Moniker> {
    let candidates = reg.scopes_in_layer(layer).filter_map(|s| match s {
        FocusScope::Zone(z) if z.parent_zone == from.parent_zone => Some((&z.moniker, z.rect)),
        _ => None,
    });
    edge_command_from_candidates(&from.rect, direction, candidates)
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
/// The beam test (`in_beam`) is a tie-break **strictly above** raw
/// score: every in-beam candidate beats every out-of-beam candidate
/// regardless of distance — see the docs on
/// [`pick_best_candidate`].
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
/// Two-tier comparison: in-beam candidates always beat out-of-beam
/// candidates regardless of raw score, then within each tier the
/// lower-scored candidate wins. This implements Android's beam-test
/// preference — an aligned candidate beats a closer-but-diagonal one.
///
/// Candidates carry borrowed [`Moniker`] references so the helper does
/// not allocate per-candidate; only the winning moniker is cloned.
fn pick_best_candidate<'a>(
    from_rect: &Rect,
    direction: Direction,
    candidates: impl Iterator<Item = (&'a Moniker, Rect)>,
) -> Option<Moniker> {
    let mut best: Option<(&Moniker, bool, f64)> = None;
    for (moniker, rect) in candidates {
        let Some((in_beam, score)) = score_candidate(from_rect, &rect, direction) else {
            continue;
        };
        match best.as_ref() {
            None => best = Some((moniker, in_beam, score)),
            Some((_, best_in_beam, best_score)) => {
                // Tier 1: in-beam beats out-of-beam.
                if in_beam && !best_in_beam {
                    best = Some((moniker, in_beam, score));
                } else if in_beam == *best_in_beam && score < *best_score {
                    // Tier 2: within the same beam tier, lower score wins.
                    best = Some((moniker, in_beam, score));
                }
            }
        }
    }
    best.map(|(m, _, _)| m.clone())
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
    use crate::scope::Focusable;
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

    /// Lonely leaf — nothing else to navigate to.
    #[test]
    fn beam_returns_none_for_known_start_with_no_neighbors() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(FocusLayer {
            key: LayerKey::from_string("L"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string("main"),
            last_focused: None,
        });
        reg.register_focusable(Focusable {
            key: SpatialKey::from_string("k"),
            moniker: Moniker::from_string("ui:k"),
            rect: rect_zero(),
            layer_key: LayerKey::from_string("L"),
            parent_zone: None,
            overrides: HashMap::new(),
        });

        let strategy = BeamNavStrategy::new();
        assert!(strategy
            .next(&reg, &SpatialKey::from_string("k"), Direction::Right)
            .is_none());
    }

    /// Unknown starting key yields `None` — same contract as
    /// `SpatialState::navigate` for stale keys.
    #[test]
    fn beam_returns_none_for_unknown_start() {
        let reg = SpatialRegistry::new();
        let strategy = BeamNavStrategy::new();
        assert!(strategy
            .next(&reg, &SpatialKey::from_string("ghost"), Direction::Up)
            .is_none());
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
