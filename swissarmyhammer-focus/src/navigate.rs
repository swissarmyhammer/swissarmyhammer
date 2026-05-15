//! Snapshot-driven Android-style beam-search navigation.
//!
//! [`pick_target`] is the single entry point: given a layer-scoped
//! [`IndexedSnapshot`], the focused [`FullyQualifiedMoniker`] paired
//! with its [`SegmentMoniker`], and a [`Direction`], it returns the
//! FQM of the next focus target.
//!
//! # No-silent-dropout contract
//!
//! [`pick_target`] always returns a [`FullyQualifiedMoniker`]. "No
//! motion possible" is communicated by returning the focused entry's
//! own FQM — the React side detects "stay put" by comparing the
//! returned FQM to the previous focused FQM. Torn state (focused FQM
//! missing from the snapshot) emits `tracing::error!` and echoes the
//! input FQM. There is no `Option` or `Result` on this surface;
//! silence is impossible.
//!
//! Two principles distinguish the two non-motion paths:
//!
//! - **No motion → return focused FQM (no trace).** A semantic
//!   "stay put" — wall override, focused at the visual edge of the
//!   layer with an empty Direction-D half-plane, leaf with no children.
//! - **Torn state → trace error AND echo input.** A genuine error —
//!   focused FQM missing from the snapshot. The kernel emits
//!   `tracing::error!` with the operation, the relevant FQM(s), and
//!   the FQM being echoed back. User-observable behavior is identical
//!   to the "no motion" case (focus stays put), but ops / devs can
//!   chase the error in logs.
//!
//! # Cardinal navigation — geometric pick (keyboard-as-mouse)
//!
//! Cardinal nav for [`Direction::Up`], [`Direction::Down`],
//! [`Direction::Left`], and [`Direction::Right`] is **purely
//! geometric**. Pressing an arrow key picks the snapshot scope whose
//! rect minimises the Android beam score (`13 * major² + minor²`)
//! across ALL scopes in the snapshot that:
//!
//! 1. Pass the **strict half-plane test** for D — the candidate's
//!    leading edge in the reverse direction is past the focused entry's
//!    leading edge in D. For `Down`: `cand.top >= from.bottom`.
//! 2. Pass the **in-beam test** for D — the candidate overlaps `from`
//!    on the cross axis (horizontal overlap for `Up`/`Down`, vertical
//!    overlap for `Left`/`Right`).
//! 3. Are not the focused entry itself.
//!
//! No structural filtering — `parent_zone` and "has children" are
//! tie-breakers and observability only.
//!
//! ## Tie-break: leaves over containers
//!
//! When two candidates have equal beam scores, **leaves win over
//! containers** (a leaf is a scope no other snapshot entry names as
//! `parent_zone`; a container is a scope at least one other entry
//! names as `parent_zone`). This ensures that when the geometric pick
//! would land equally on a `showFocus=false` container and an inner
//! leaf, the user sees the focus indicator paint on the leaf.
//!
//! ## When the half-plane is empty
//!
//! If no candidate passes the strict half-plane and in-beam tests, the
//! focused entry is at the visual edge of the layer in direction D.
//! The kernel returns the focused FQM (stay-put), per the
//! no-silent-dropout invariant.
//!
//! # First / Last
//!
//! [`Direction::First`] and [`Direction::Last`] focus the
//! **focused scope's children**, not its siblings. The deprecated
//! `Direction::RowStart` / `Direction::RowEnd` aliases route through
//! the same path.
//!
//! - **First child** = the child whose rect is topmost; ties broken by
//!   leftmost.
//! - **Last child** = the child whose rect is bottommost; ties broken
//!   by rightmost.
//! - **Children** = snapshot entries whose `parent_zone` is the focused
//!   scope's FQM.
//!
//! On a focused leaf (no children) both ops return the focused FQM
//! (semantic no-op, no log noise) per the no-silent-dropout contract.

use std::collections::HashSet;

use crate::registry::SpatialRegistry;
use crate::snapshot::{IndexedSnapshot, SnapshotScope};
use crate::types::{pixels_cmp, Direction, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker};

/// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
/// *into* the scope at `fq`.
///
/// Resolution order:
///
/// - **Scope with a live `last_focused_by_fq` entry** — returns that
///   descendant's FQM when the recorded target is still present in
///   `view`, restoring the user's last position inside the scope across
///   drill-out / drill-in cycles.
/// - **Scope with stale or absent `last_focused_by_fq`** — falls back
///   to the first child by rect top-left ordering (topmost wins; ties
///   broken by leftmost). Matches `Direction::First` ordering so the
///   keyboard model stays consistent.
/// - **Scope with no children (a leaf)** — returns `focused_fq`. The
///   caller compares the result against the focused FQM; equal means
///   "no descent happened, fall through to edit / no-op".
/// - **Unknown `fq`** — emits `tracing::error!` and returns
///   `focused_fq`.
pub fn drill_in(
    view: &IndexedSnapshot<'_>,
    registry: &SpatialRegistry,
    fq: FullyQualifiedMoniker,
    focused_fq: &FullyQualifiedMoniker,
) -> FullyQualifiedMoniker {
    if view.get(&fq).is_none() {
        tracing::error!(
            op = "drill_in",
            focused_fq = %fq,
            focused = %focused_fq,
            "unknown FQM passed to drill_in"
        );
        return focused_fq.clone();
    };

    if let Some(remembered) = registry.last_focused_by_fq.get(&fq) {
        if view.get(remembered).is_some() {
            return remembered.clone();
        }
    }

    view.scopes()
        .iter()
        .filter(|s| s.parent_zone.as_ref() == Some(&fq))
        .min_by(|a, b| {
            pixels_cmp(a.rect.top(), b.rect.top()).then(pixels_cmp(a.rect.left(), b.rect.left()))
        })
        .map(|s| s.fq.clone())
        .unwrap_or_else(|| focused_fq.clone())
}

/// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
/// *out of* the scope at `fq`.
///
/// Returns the FQM of the scope's `parent_zone`. Returns `focused_fq`
/// when the scope has no `parent_zone` (sits directly under the layer
/// root) — the React side falls through to `app.dismiss` on the
/// FQM-equality check. Torn state (unknown `fq`, parent_zone names a
/// missing FQM) emits `tracing::error!` and returns `focused_fq`.
pub fn drill_out(
    view: &IndexedSnapshot<'_>,
    fq: FullyQualifiedMoniker,
    focused_fq: &FullyQualifiedMoniker,
) -> FullyQualifiedMoniker {
    let Some(entry) = view.get(&fq) else {
        tracing::error!(
            op = "drill_out",
            focused_fq = %fq,
            focused = %focused_fq,
            "unknown FQM passed to drill_out"
        );
        return focused_fq.clone();
    };
    let Some(parent_zone_fq) = &entry.parent_zone else {
        return focused_fq.clone();
    };
    if view.get(parent_zone_fq).is_none() {
        tracing::error!(
            op = "drill_out",
            focused_fq = %fq,
            focused = %focused_fq,
            parent_zone_fq = %parent_zone_fq,
            "parent_zone references unregistered scope"
        );
        return focused_fq.clone();
    }
    parent_zone_fq.clone()
}

/// Pick the next focus target for `focused_fq` in `direction`.
///
/// Reads the focused entry, candidates, override target, and
/// First/Last children entirely from `view`. When motion is not
/// possible (visual edge of the layout, override wall, layer root,
/// torn-state errors), the function returns `focused_fq` itself —
/// never `None`.
pub fn pick_target(
    view: &IndexedSnapshot<'_>,
    focused_fq: &FullyQualifiedMoniker,
    focused_segment: &SegmentMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    let Some(focused) = view.get(focused_fq) else {
        tracing::error!(
            op = "nav",
            focused_fq = %focused_fq,
            focused_segment = %focused_segment,
            ?direction,
            "unknown focused FQM passed to pick_target"
        );
        return focused_fq.clone();
    };

    match check_override(view, focused, direction) {
        Some(Some(target)) => return target,
        Some(None) => return focused_fq.clone(),
        None => {}
    }

    #[allow(deprecated)]
    match direction {
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
            geometric_pick(view, focused, focused_fq, direction)
        }
        Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
            edge_command(view, focused, focused_fq, direction)
        }
    }
}

/// First/Last edge-command pick.
///
/// Children of the focused scope are entries whose
/// `parent_zone == focused.fq`. When the focused scope has no
/// children, the function returns `focused_fq` (semantic no-op).
#[allow(deprecated)]
fn edge_command(
    view: &IndexedSnapshot<'_>,
    focused: &SnapshotScope,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    let pick: Option<FullyQualifiedMoniker> = match direction {
        Direction::First | Direction::RowStart => view
            .scopes()
            .iter()
            .filter(|s| s.parent_zone.as_ref() == Some(&focused.fq))
            .min_by(|a, b| {
                pixels_cmp(a.rect.top(), b.rect.top())
                    .then(pixels_cmp(a.rect.left(), b.rect.left()))
            })
            .map(|s| s.fq.clone()),
        Direction::Last | Direction::RowEnd => view
            .scopes()
            .iter()
            .filter(|s| s.parent_zone.as_ref() == Some(&focused.fq))
            .max_by(|a, b| {
                pixels_cmp(a.rect.bottom(), b.rect.bottom())
                    .then(pixels_cmp(a.rect.right(), b.rect.right()))
            })
            .map(|s| s.fq.clone()),
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => None,
    };
    pick.unwrap_or_else(|| focused_fq.clone())
}

/// Resolve the per-direction override on `focused`, if any.
///
/// The outer [`Option`] of the return value encodes "did an override
/// apply?", and the inner [`Option<FullyQualifiedMoniker>`] encodes
/// the answer when it did:
///
/// - **`None`** — no entry for `direction` on the focused scope (or the
///   entry names a target that does not resolve in `view`). The
///   override didn't apply; the caller falls through to the geometric
///   pick.
/// - **`Some(None)`** — explicit "wall": the override map maps
///   `direction → None`. Navigation is blocked; the caller returns
///   the focused FQM without consulting beam search.
/// - **`Some(Some(target_fq))`** — redirect: the override map maps
///   `direction → Some(target)` *and* `target` is present in `view`.
///   The caller returns `target_fq`; beam search does not run.
///
/// Layer scoping is enforced by the snapshot itself — the snapshot is
/// already layer-scoped, so a target that names an FQM in a different
/// layer is treated as "unresolved" (it's not in `view`) and the
/// override falls through.
fn check_override(
    view: &IndexedSnapshot<'_>,
    focused: &SnapshotScope,
    direction: Direction,
) -> Option<Option<FullyQualifiedMoniker>> {
    let entry = focused.nav_override.get(&direction)?;
    match entry {
        None => Some(None),
        Some(target) => {
            if view.get(target).is_some() {
                Some(Some(target.clone()))
            } else {
                None
            }
        }
    }
}

/// Run the geometric pick from `focused` in `direction`.
///
/// Iterates every entry in `view`, filters out the focused entry
/// itself, scores via [`score_candidate`], and returns the candidate
/// with the lowest beam score that passes the strict half-plane and
/// in-beam tests. Ties are broken by preferring leaves (scopes that
/// no other entry names as `parent_zone`) over containers.
///
/// When no candidate satisfies both tests, the focused entry is at
/// the visual edge of the layer in `direction`; the function returns
/// `focused_fq` (stay-put).
fn geometric_pick(
    view: &IndexedSnapshot<'_>,
    focused: &SnapshotScope,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    // Build the "has children" predicate once per pick. Looking up
    // `has_children(fq)` on every iteration is O(N), so calling it
    // inside the candidate loop made the pick O(N²). Collecting the
    // set of FQMs that appear as some scope's `parent_zone` (anywhere
    // in the layer view) restores O(N).
    let parent_fqs: HashSet<FullyQualifiedMoniker> = view
        .scopes()
        .iter()
        .filter_map(|s| s.parent_zone.clone())
        .collect();
    let from_rect = focused.rect;
    let mut best: Option<BestCandidate> = None;
    for cand in view.scopes() {
        if cand.fq == focused.fq {
            continue;
        }
        if !in_strict_half_plane(&from_rect, &cand.rect, direction) {
            continue;
        }
        let Some((in_beam, score)) = score_candidate(&from_rect, &cand.rect, direction) else {
            continue;
        };
        if !in_beam {
            continue;
        }
        let cand_summary = BestCandidate {
            fq: cand.fq.clone(),
            score,
            has_children: parent_fqs.contains(&cand.fq),
        };
        if better_candidate(&best, &cand_summary) {
            best = Some(cand_summary);
        }
    }
    match best {
        Some(b) => b.fq,
        None => focused_fq.clone(),
    }
}

/// Running best candidate inside [`geometric_pick`]: FQM, beam score, and
/// the has-children flag used for the leaves-over-containers tie-break.
#[derive(Clone)]
struct BestCandidate {
    fq: FullyQualifiedMoniker,
    score: f64,
    has_children: bool,
}

/// `true` when `cand` should replace the current best candidate.
///
/// Primary order is the Android beam score: lower is better. When two
/// candidates have equal scores, the leaf wins over the container.
fn better_candidate(current: &Option<BestCandidate>, cand: &BestCandidate) -> bool {
    match current {
        None => true,
        Some(cur) => {
            if cand.score < cur.score {
                true
            } else if cand.score > cur.score {
                false
            } else {
                !cand.has_children && cur.has_children
            }
        }
    }
}

/// `true` if `cand` lies strictly in the half-plane of `direction`
/// from `from`.
#[allow(deprecated)]
fn in_strict_half_plane(from: &Rect, cand: &Rect, direction: Direction) -> bool {
    match direction {
        Direction::Down => cand.top().value() >= from.bottom().value(),
        Direction::Up => cand.bottom().value() <= from.top().value(),
        Direction::Right => cand.left().value() >= from.right().value(),
        Direction::Left => cand.right().value() <= from.left().value(),
        Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => false,
    }
}

/// Score one candidate against the focused rect for a cardinal
/// direction.
///
/// Returns:
/// - `None` if the candidate is on the wrong side of `from` (e.g.
///   not "below" when navigating `Down`) or its rect collapses into
///   `from` along the major axis.
/// - `Some((in_beam, score))` otherwise. Lower `score` is better.
#[allow(deprecated)]
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

/// `true` if two rects overlap on the x axis.
fn horizontal_overlap(a: &Rect, b: &Rect) -> bool {
    a.left().value() < b.right().value() && b.left().value() < a.right().value()
}

/// `true` if two rects overlap on the y axis.
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
    Horizontal,
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
    use crate::snapshot::{NavSnapshot, SnapshotScope};
    use crate::types::{FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker};
    use std::collections::HashMap;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x: Pixels::new(x),
            y: Pixels::new(y),
            width: Pixels::new(w),
            height: Pixels::new(h),
        }
    }

    fn rect_zero() -> Rect {
        rect(0.0, 0.0, 10.0, 10.0)
    }

    fn scope(fq: &str, parent_zone: Option<&str>, r: Rect) -> SnapshotScope {
        SnapshotScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            rect: r,
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            nav_override: HashMap::new(),
        }
    }

    fn snapshot(scopes: Vec<SnapshotScope>) -> NavSnapshot {
        NavSnapshot {
            layer_fq: FullyQualifiedMoniker::from_string("/L"),
            scopes,
        }
    }

    fn pick(snap: &NavSnapshot, focused_fq: &str, direction: Direction) -> FullyQualifiedMoniker {
        let view = IndexedSnapshot::new(snap);
        let fq = FullyQualifiedMoniker::from_string(focused_fq);
        let segment = SegmentMoniker::from_string("seg");
        pick_target(&view, &fq, &segment, direction)
    }

    /// Lonely scope — nothing else to navigate to. Returns the
    /// focused FQM (semantic "stay put" — empty Direction-D
    /// half-plane).
    #[test]
    fn lonely_scope_returns_focused_fq() {
        let snap = snapshot(vec![scope("/L/k", None, rect_zero())]);
        for d in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let result = pick(&snap, "/L/k", d);
            assert_eq!(
                result,
                FullyQualifiedMoniker::from_string("/L/k"),
                "lonely scope must echo focused FQM for {d:?}"
            );
        }
    }

    /// One neighbor in direction wins.
    #[test]
    fn one_neighbor_in_direction_wins() {
        let snap = snapshot(vec![
            scope("/L/src", None, rect(0.0, 0.0, 10.0, 10.0)),
            scope("/L/neighbor", None, rect(20.0, 0.0, 10.0, 10.0)),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/neighbor")
        );
    }

    /// Two neighbors at different distances — closer wins.
    #[test]
    fn closer_neighbor_wins() {
        let snap = snapshot(vec![
            scope("/L/src", None, rect(0.0, 0.0, 10.0, 10.0)),
            scope("/L/near", None, rect(20.0, 0.0, 10.0, 10.0)),
            scope("/L/far", None, rect(100.0, 0.0, 10.0, 10.0)),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/near"),
            "closer in-beam neighbor must win"
        );
    }

    /// Tied geometry — leaf wins over container.
    #[test]
    fn tied_distances_leaf_wins_over_container() {
        let snap = snapshot(vec![
            scope("/L/src", None, rect(0.0, 0.0, 10.0, 10.0)),
            scope("/L/container-cand", None, rect(20.0, 0.0, 10.0, 10.0)),
            scope(
                "/L/container-cand/child",
                Some("/L/container-cand"),
                rect(20.0, 0.0, 5.0, 5.0),
            ),
            scope("/L/leaf-cand", None, rect(20.0, 0.0, 10.0, 10.0)),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/leaf-cand"),
            "geometric tie must resolve to the leaf, not the container"
        );
    }

    /// Cross-`parent_zone` candidate wins when geometrically nearer.
    #[test]
    fn cross_parent_zone_candidate_wins_when_geometrically_nearer() {
        let snap = snapshot(vec![
            scope("/L/zone-left", None, rect(0.0, 0.0, 100.0, 50.0)),
            scope("/L/zone-right", None, rect(200.0, 100.0, 100.0, 50.0)),
            scope("/L/src", Some("/L/zone-left"), rect(80.0, 10.0, 10.0, 10.0)),
            scope(
                "/L/in-zone-below",
                Some("/L/zone-left"),
                rect(80.0, 30.0, 10.0, 10.0),
            ),
            scope(
                "/L/cross-zone-near",
                Some("/L/zone-right"),
                rect(205.0, 10.0, 10.0, 10.0),
            ),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/cross-zone-near"),
        );
    }

    /// Unknown starting FQM echoes the input — torn state is surfaced
    /// to logs, not as `None`.
    #[test]
    fn beam_returns_focused_fq_for_unknown_start() {
        let snap = snapshot(vec![]);
        let result = pick(&snap, "/ghost", Direction::Up);
        assert_eq!(result, FullyQualifiedMoniker::from_string("/ghost"));
    }

    /// Focused leaf has no children — both `First` and `Last` echo the
    /// focused FQM (semantic no-op, no log noise).
    #[test]
    fn first_last_on_leaf_returns_focused_self() {
        let snap = snapshot(vec![scope("/L/leaf", None, rect_zero())]);
        for d in [Direction::First, Direction::Last] {
            assert_eq!(
                pick(&snap, "/L/leaf", d),
                FullyQualifiedMoniker::from_string("/L/leaf"),
                "leaf has no children — {d:?} must echo focused FQM"
            );
        }
    }

    /// Focused scope with exactly one child — both `First` and `Last`
    /// return that child's FQM.
    #[test]
    fn first_last_on_zone_with_one_child_returns_that_child() {
        let snap = snapshot(vec![
            scope("/L/parent", None, rect(0.0, 0.0, 100.0, 100.0)),
            scope(
                "/L/parent/only",
                Some("/L/parent"),
                rect(10.0, 10.0, 50.0, 50.0),
            ),
        ]);
        assert_eq!(
            pick(&snap, "/L/parent", Direction::First),
            FullyQualifiedMoniker::from_string("/L/parent/only")
        );
        assert_eq!(
            pick(&snap, "/L/parent", Direction::Last),
            FullyQualifiedMoniker::from_string("/L/parent/only")
        );
    }

    /// Focused scope whose three children sit in a horizontal row —
    /// `First` picks the leftmost; `Last` picks the rightmost.
    #[test]
    fn first_last_on_zone_with_row_of_children() {
        let snap = snapshot(vec![
            scope("/L/row", None, rect(0.0, 0.0, 300.0, 50.0)),
            scope("/L/row/left", Some("/L/row"), rect(0.0, 10.0, 50.0, 30.0)),
            scope(
                "/L/row/middle",
                Some("/L/row"),
                rect(100.0, 10.0, 50.0, 30.0),
            ),
            scope(
                "/L/row/right",
                Some("/L/row"),
                rect(200.0, 10.0, 50.0, 30.0),
            ),
        ]);
        assert_eq!(
            pick(&snap, "/L/row", Direction::First),
            FullyQualifiedMoniker::from_string("/L/row/left"),
        );
        assert_eq!(
            pick(&snap, "/L/row", Direction::Last),
            FullyQualifiedMoniker::from_string("/L/row/right"),
        );
    }

    /// Focused scope whose three children sit in a vertical column —
    /// `First` picks the topmost; `Last` picks the bottommost.
    #[test]
    fn first_last_on_zone_with_column_of_children() {
        let snap = snapshot(vec![
            scope("/L/col", None, rect(0.0, 0.0, 50.0, 300.0)),
            scope("/L/col/top", Some("/L/col"), rect(0.0, 0.0, 50.0, 30.0)),
            scope(
                "/L/col/middle",
                Some("/L/col"),
                rect(0.0, 100.0, 50.0, 30.0),
            ),
            scope(
                "/L/col/bottom",
                Some("/L/col"),
                rect(0.0, 200.0, 50.0, 30.0),
            ),
        ]);
        assert_eq!(
            pick(&snap, "/L/col", Direction::First),
            FullyQualifiedMoniker::from_string("/L/col/top"),
        );
        assert_eq!(
            pick(&snap, "/L/col", Direction::Last),
            FullyQualifiedMoniker::from_string("/L/col/bottom"),
        );
    }

    /// The deprecated `RowStart` / `RowEnd` aliases must keep
    /// returning the same target as `First` / `Last`.
    #[allow(deprecated)]
    #[test]
    fn deprecated_row_start_end_still_alias_first_last() {
        let snap = snapshot(vec![
            scope("/L/row", None, rect(0.0, 0.0, 300.0, 50.0)),
            scope("/L/row/left", Some("/L/row"), rect(0.0, 10.0, 50.0, 30.0)),
            scope(
                "/L/row/right",
                Some("/L/row"),
                rect(200.0, 10.0, 50.0, 30.0),
            ),
        ]);
        assert_eq!(
            pick(&snap, "/L/row", Direction::RowStart),
            pick(&snap, "/L/row", Direction::First),
        );
        assert_eq!(
            pick(&snap, "/L/row", Direction::RowEnd),
            pick(&snap, "/L/row", Direction::Last),
        );
    }

    /// Override redirect honored — beam search is bypassed.
    #[test]
    fn nav_override_redirect_honored() {
        let mut src = scope("/L/src", None, rect(0.0, 0.0, 10.0, 10.0));
        src.nav_override.insert(
            Direction::Right,
            Some(FullyQualifiedMoniker::from_string("/L/target")),
        );
        let snap = snapshot(vec![
            src,
            scope("/L/target", None, rect(100.0, 0.0, 10.0, 10.0)),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/target"),
        );
    }

    /// Override block honored — explicit "wall" returns focused FQM
    /// even when a geometric candidate exists.
    #[test]
    fn nav_override_block_honored() {
        let mut src = scope("/L/src", None, rect(0.0, 0.0, 10.0, 10.0));
        src.nav_override.insert(Direction::Right, None);
        let snap = snapshot(vec![
            src,
            scope("/L/neighbor", None, rect(20.0, 0.0, 10.0, 10.0)),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/src"),
            "explicit wall must take precedence over geometric pick",
        );
    }

    /// `parent_zone` cycle does not freeze pathfinding — the
    /// has-children precomputation reads `parent_zone` only as a
    /// flat-set membership check.
    #[test]
    fn parent_zone_cycle_does_not_hang() {
        let snap = snapshot(vec![
            scope("/L/src", None, rect(0.0, 0.0, 10.0, 10.0)),
            scope("/L/a", Some("/L/b"), rect(20.0, 0.0, 10.0, 10.0)),
            scope("/L/b", Some("/L/a"), rect(50.0, 0.0, 10.0, 10.0)),
        ]);
        assert_eq!(
            pick(&snap, "/L/src", Direction::Right),
            FullyQualifiedMoniker::from_string("/L/a"),
        );
    }

    // -----------------------------------------------------------------
    // drill_in / drill_out coverage
    //
    // These tests pin the user-symptom contract: pressing Enter on a
    // focused card must walk into the card's first field child via
    // `drill_in`'s `parent_zone == Some(card_fq)` filter, and pressing
    // Escape inside a field must climb back up via `drill_out`'s
    // `parent_zone` lookup. Both helpers are layer-scoped and rely on
    // the snapshot for all geometry — the registry only contributes the
    // `last_focused_by_fq` warm-path memory consulted by `drill_in`.
    // -----------------------------------------------------------------

    /// Drive `drill_in` against an in-test snapshot. Mirrors `pick` but
    /// returns the descended target rather than the cardinal pick, and
    /// borrows a [`SpatialRegistry`] so callers can preload
    /// `last_focused_by_fq` to exercise the warm path.
    fn drill_in_at(
        snap: &NavSnapshot,
        registry: &SpatialRegistry,
        fq: &str,
        focused_fq: &str,
    ) -> FullyQualifiedMoniker {
        let view = IndexedSnapshot::new(snap);
        let target = FullyQualifiedMoniker::from_string(fq);
        let focused = FullyQualifiedMoniker::from_string(focused_fq);
        drill_in(&view, registry, target, &focused)
    }

    /// Drive `drill_out` against an in-test snapshot. `drill_out` does
    /// not consult the registry, so the caller does not need to build
    /// one.
    fn drill_out_at(snap: &NavSnapshot, fq: &str, focused_fq: &str) -> FullyQualifiedMoniker {
        let view = IndexedSnapshot::new(snap);
        let target = FullyQualifiedMoniker::from_string(fq);
        let focused = FullyQualifiedMoniker::from_string(focused_fq);
        drill_out(&view, target, &focused)
    }

    /// Card with three field children, no `last_focused_by_fq` entry.
    /// `drill_in` must fall back to the topmost-leftmost child. The
    /// fields are deliberately registered in a non-sorted order so the
    /// test can't pass on insertion order alone.
    #[test]
    fn drill_in_card_with_field_children_picks_topmost_leftmost() {
        let snap = snapshot(vec![
            scope("/L/card", None, rect(0.0, 0.0, 200.0, 100.0)),
            // Bottom row, left.
            scope(
                "/L/card/f-bottom",
                Some("/L/card"),
                rect(0.0, 60.0, 50.0, 30.0),
            ),
            // Top row, right — same y as the topmost-left field.
            scope(
                "/L/card/f-top-right",
                Some("/L/card"),
                rect(100.0, 10.0, 50.0, 30.0),
            ),
            // Top row, left — should win on `(top, left)` ordering.
            scope(
                "/L/card/f-top-left",
                Some("/L/card"),
                rect(0.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let registry = SpatialRegistry::new();
        let result = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/card/f-top-left"),
            "with no last_focused, drill_in must pick the topmost-leftmost field child"
        );
    }

    /// Card with field children AND a recorded `last_focused_by_fq`
    /// entry. The warm path wins over the topmost-leftmost fallback.
    #[test]
    fn drill_in_returns_remembered_last_focused_when_present() {
        let snap = snapshot(vec![
            scope("/L/card", None, rect(0.0, 0.0, 200.0, 100.0)),
            scope(
                "/L/card/f-top-left",
                Some("/L/card"),
                rect(0.0, 10.0, 50.0, 30.0),
            ),
            scope(
                "/L/card/f-second",
                Some("/L/card"),
                rect(60.0, 10.0, 50.0, 30.0),
            ),
            scope(
                "/L/card/f-third",
                Some("/L/card"),
                rect(120.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let mut registry = SpatialRegistry::new();
        registry.last_focused_by_fq.insert(
            FullyQualifiedMoniker::from_string("/L/card"),
            FullyQualifiedMoniker::from_string("/L/card/f-second"),
        );
        let result = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/card/f-second"),
            "warm path: a recorded last_focused must override the geometric first-child fallback"
        );
    }

    /// Card with no field children (a leaf). Per the no-silent-dropout
    /// contract, `drill_in` echoes `focused_fq`.
    #[test]
    fn drill_in_card_with_no_children_returns_focused_fq() {
        let snap = snapshot(vec![scope("/L/card", None, rect_zero())]);
        let registry = SpatialRegistry::new();
        let result = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/card"),
            "leaf card must echo focused_fq (semantic 'stay put')"
        );
    }

    /// Card with exactly one field child — trivial pick.
    #[test]
    fn drill_in_card_with_one_field_child_returns_that_child() {
        let snap = snapshot(vec![
            scope("/L/card", None, rect(0.0, 0.0, 100.0, 50.0)),
            scope(
                "/L/card/only-field",
                Some("/L/card"),
                rect(10.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let registry = SpatialRegistry::new();
        let result = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/card/only-field"),
            "single-child case must descend to that child"
        );
    }

    /// Two field children at the same rect — the kernel must pick one
    /// deterministically using the chained `(top, left)` comparison.
    /// `min_by` is stable on equal keys: it returns the first element
    /// when no later element compares strictly less, so the snapshot
    /// insertion order pins the tie-break.
    #[test]
    fn drill_in_with_tied_field_positions_is_deterministic() {
        let snap = snapshot(vec![
            scope("/L/card", None, rect(0.0, 0.0, 100.0, 50.0)),
            scope(
                "/L/card/f-first",
                Some("/L/card"),
                rect(10.0, 10.0, 50.0, 30.0),
            ),
            scope(
                "/L/card/f-second",
                Some("/L/card"),
                rect(10.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let registry = SpatialRegistry::new();
        let first_run = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        // Multiple runs must agree — there is no nondeterministic
        // tie-break (no hashing or RNG on this path).
        for _ in 0..5 {
            let again = drill_in_at(&snap, &registry, "/L/card", "/L/card");
            assert_eq!(
                again, first_run,
                "drill_in must return the same FQM on every call for a tied snapshot"
            );
        }
        assert_eq!(
            first_run,
            FullyQualifiedMoniker::from_string("/L/card/f-first"),
            "tie-break must yield the first scope in insertion order"
        );
    }

    /// `parent_zone` cycle: card claims its own field as its parent.
    /// `drill_in` walks `parent_zone` only as a flat-set predicate
    /// (`s.parent_zone.as_ref() == Some(&fq)`) so a self-reference does
    /// not cause infinite recursion. This also pins the result against
    /// regression — even on a malformed snapshot the pick is
    /// deterministic.
    #[test]
    fn drill_in_with_parent_zone_cycle_does_not_hang() {
        let snap = snapshot(vec![
            // Card names the field as its own parent — torn input.
            scope(
                "/L/card",
                Some("/L/card/field"),
                rect(0.0, 0.0, 100.0, 50.0),
            ),
            scope(
                "/L/card/field",
                Some("/L/card"),
                rect(10.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let registry = SpatialRegistry::new();
        let result = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/card/field"),
            "even with a parent_zone cycle, drill_in must terminate with a deterministic pick"
        );
    }

    /// `record_focus` writes the focused FQM into every ancestor scope's
    /// `last_focused_by_fq` slot, so a subsequent `drill_in(card)`
    /// re-lands on the same field. Pins the warm-path round-trip
    /// described in the registry's `record_focus` doc-comment.
    #[test]
    fn record_focus_then_drill_in_round_trips_to_same_field() {
        use crate::layer::FocusLayer;
        use crate::types::{LayerName, WindowLabel};

        let snap = snapshot(vec![
            scope("/L/card", None, rect(0.0, 0.0, 100.0, 50.0)),
            scope(
                "/L/card/f-top-left",
                Some("/L/card"),
                rect(0.0, 10.0, 50.0, 30.0),
            ),
            scope(
                "/L/card/f-second",
                Some("/L/card"),
                rect(60.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let mut registry = SpatialRegistry::new();
        // The layer must be registered for `record_focus` to walk the
        // layer-ancestor chain — otherwise that phase is a no-op.
        registry.push_layer(FocusLayer {
            fq: FullyQualifiedMoniker::from_string("/L"),
            segment: SegmentMoniker::from_string("L"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string("main"),
            last_focused: None,
        });

        let view = IndexedSnapshot::new(&snap);
        let second_field = FullyQualifiedMoniker::from_string("/L/card/f-second");
        registry.record_focus(&second_field, &view);

        // The card's `last_focused_by_fq` slot now holds f-second.
        assert_eq!(
            registry
                .last_focused_by_fq
                .get(&FullyQualifiedMoniker::from_string("/L/card")),
            Some(&second_field),
            "record_focus must write the ancestor's last_focused_by_fq slot",
        );

        // Drill back into the card — the warm path returns f-second
        // even though f-top-left would win on the cold-path topmost-
        // leftmost ordering.
        let result = drill_in_at(&snap, &registry, "/L/card", "/L/card");
        assert_eq!(
            result, second_field,
            "drill_in after record_focus must return the recorded descendant",
        );
    }

    /// `drill_in` on an unknown FQM emits `tracing::error!` and echoes
    /// `focused_fq`. The registry's `last_focused_by_fq` is consulted
    /// only after the existence check, so a stale entry does not mask
    /// the torn-state error path.
    #[test]
    fn drill_in_unknown_fq_returns_focused_fq() {
        let snap = snapshot(vec![scope("/L/known", None, rect_zero())]);
        let registry = SpatialRegistry::new();
        let result = drill_in_at(&snap, &registry, "/L/ghost", "/L/known");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/known"),
            "torn state must echo focused_fq, not the unknown target"
        );
    }

    /// `drill_out` on a field with a registered parent card returns the
    /// card's FQM. Mirrors the user-symptom contract for Escape inside
    /// a field.
    #[test]
    fn drill_out_field_returns_parent_card() {
        let snap = snapshot(vec![
            scope("/L/card", None, rect(0.0, 0.0, 100.0, 50.0)),
            scope(
                "/L/card/field",
                Some("/L/card"),
                rect(10.0, 10.0, 50.0, 30.0),
            ),
        ]);
        let result = drill_out_at(&snap, "/L/card/field", "/L/card/field");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/card"),
            "drill_out from a field must climb to its parent card"
        );
    }

    /// `drill_out` on a scope with no parent_zone (registered directly
    /// under the layer root) returns `focused_fq`. The React side
    /// detects this via FQM-equality and falls through to `app.dismiss`.
    #[test]
    fn drill_out_top_level_scope_returns_focused_fq() {
        let snap = snapshot(vec![scope("/L/top", None, rect_zero())]);
        let result = drill_out_at(&snap, "/L/top", "/L/top");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/top"),
            "scope with no parent_zone must echo focused_fq"
        );
    }

    /// `drill_out` on an unknown FQM emits `tracing::error!` and echoes
    /// `focused_fq`. Torn-state path.
    #[test]
    fn drill_out_unknown_fq_returns_focused_fq() {
        let snap = snapshot(vec![scope("/L/known", None, rect_zero())]);
        let result = drill_out_at(&snap, "/L/ghost", "/L/known");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/known"),
            "drill_out on unknown FQM must echo focused_fq"
        );
    }

    /// `drill_out` where the scope's `parent_zone` names an FQM that
    /// is not present in the snapshot — torn-state recovery. Returns
    /// `focused_fq`.
    #[test]
    fn drill_out_dangling_parent_zone_returns_focused_fq() {
        let snap = snapshot(vec![scope("/L/orphan", Some("/L/missing"), rect_zero())]);
        let result = drill_out_at(&snap, "/L/orphan", "/L/orphan");
        assert_eq!(
            result,
            FullyQualifiedMoniker::from_string("/L/orphan"),
            "dangling parent_zone must echo focused_fq, not propagate the missing FQM"
        );
    }
}
