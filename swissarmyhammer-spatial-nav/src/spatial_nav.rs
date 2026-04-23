//! Spatial navigation algorithm: beam test, scoring, and candidate selection.
//!
//! This is a pure function module — no state, no side effects. The
//! [`find_target`] function takes a source rect and candidate entries,
//! applies the Android FocusFinder beam test and scoring algorithm,
//! and returns the best navigation target.
//!
//! The [`container_first_search`] wrapper implements scope-aware navigation:
//! siblings in the same `parent_scope` are searched first, falling back to
//! the full candidate set when no sibling matches.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::spatial_state::{Rect, SpatialEntry};

/// Navigation direction for spatial focus movement.
///
/// Cardinal directions use beam test + scoring. Edge commands use
/// positional sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// Move focus upward (decreasing y).
    Up,
    /// Move focus downward (increasing y).
    Down,
    /// Move focus leftward (decreasing x).
    Left,
    /// Move focus rightward (increasing x).
    Right,
    /// Jump to the topmost-leftmost entry.
    First,
    /// Jump to the bottommost-rightmost entry.
    Last,
    /// Jump to the leftmost entry in the same row.
    RowStart,
    /// Jump to the rightmost entry in the same row.
    RowEnd,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Up => write!(f, "Up"),
            Direction::Down => write!(f, "Down"),
            Direction::Left => write!(f, "Left"),
            Direction::Right => write!(f, "Right"),
            Direction::First => write!(f, "First"),
            Direction::Last => write!(f, "Last"),
            Direction::RowStart => write!(f, "RowStart"),
            Direction::RowEnd => write!(f, "RowEnd"),
        }
    }
}

/// Error returned when parsing an invalid direction string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDirectionError(pub String);

impl fmt::Display for ParseDirectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown direction: {}", self.0)
    }
}

impl std::error::Error for ParseDirectionError {}

impl FromStr for Direction {
    type Err = ParseDirectionError;

    /// Parse a direction string (case-insensitive).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "up" => Ok(Direction::Up),
            "down" => Ok(Direction::Down),
            "left" => Ok(Direction::Left),
            "right" => Ok(Direction::Right),
            "first" => Ok(Direction::First),
            "last" => Ok(Direction::Last),
            "rowstart" => Ok(Direction::RowStart),
            "rowend" => Ok(Direction::RowEnd),
            _ => Err(ParseDirectionError(s.to_string())),
        }
    }
}

/// Center point of a rect along a single axis.
fn center_x(r: &Rect) -> f64 {
    r.x + r.width / 2.0
}

/// Center point of a rect along the vertical axis.
fn center_y(r: &Rect) -> f64 {
    r.y + r.height / 2.0
}

/// Test whether a candidate is in the correct direction from the source.
///
/// Returns `true` if the candidate's edge is strictly beyond the source's
/// edge in the navigation direction.
fn is_in_direction(source: &Rect, candidate: &Rect, direction: Direction) -> bool {
    match direction {
        Direction::Right => candidate.x >= source.right(),
        Direction::Left => candidate.right() <= source.x,
        Direction::Down => candidate.y >= source.bottom(),
        Direction::Up => candidate.bottom() <= source.y,
        _ => false,
    }
}

/// Test whether a candidate falls within the perpendicular beam of the source.
///
/// For horizontal navigation (Left/Right), the beam is the vertical band
/// from `source.y` to `source.bottom()`. For vertical navigation (Up/Down),
/// the beam is the horizontal band from `source.x` to `source.right()`.
fn is_in_beam(source: &Rect, candidate: &Rect, direction: Direction) -> bool {
    match direction {
        Direction::Right | Direction::Left => {
            candidate.y < source.bottom() && candidate.bottom() > source.y
        }
        Direction::Up | Direction::Down => {
            candidate.x < source.right() && candidate.right() > source.x
        }
        _ => false,
    }
}

/// Android FocusFinder scoring: `13 * major² + minor²`.
///
/// Major axis is the distance along the navigation direction; minor axis
/// is the perpendicular distance between centers. The 13:1 squared ratio
/// strongly prefers aligned candidates over closer diagonal ones.
fn score(source: &Rect, candidate: &Rect, direction: Direction) -> f64 {
    let (major, minor) = match direction {
        Direction::Right => (
            candidate.x - source.right(),
            (center_y(candidate) - center_y(source)).abs(),
        ),
        Direction::Left => (
            source.x - candidate.right(),
            (center_y(candidate) - center_y(source)).abs(),
        ),
        Direction::Down => (
            candidate.y - source.bottom(),
            (center_x(candidate) - center_x(source)).abs(),
        ),
        Direction::Up => (
            source.y - candidate.bottom(),
            (center_x(candidate) - center_x(source)).abs(),
        ),
        _ => (0.0, 0.0),
    };
    13.0 * major * major + minor * minor
}

/// Test whether a candidate overlaps the source's vertical range.
///
/// Used by RowStart/RowEnd to filter to "same row" candidates.
fn overlaps_y_range(source: &Rect, candidate: &Rect) -> bool {
    candidate.y < source.bottom() && candidate.bottom() > source.y
}

/// Find the best navigation target from a set of candidates.
///
/// For cardinal directions (Up/Down/Left/Right), applies the two-phase
/// beam test: in-beam candidates are preferred, falling back to all
/// directional candidates. Within each group, the lowest-scoring
/// candidate wins (Android FocusFinder algorithm).
///
/// For edge commands (First/Last/RowStart/RowEnd), uses positional sorting.
///
/// **Caller must exclude the source entry from `candidates`.** If the
/// source appears in candidates, edge commands (RowStart/RowEnd) may
/// return it as the winner since it overlaps its own y-range.
///
/// Returns `None` if no valid candidate exists.
pub fn find_target(
    source: &SpatialEntry,
    candidates: &[&SpatialEntry],
    direction: Direction,
) -> Option<String> {
    match direction {
        Direction::First => find_edge_first(candidates),
        Direction::Last => find_edge_last(candidates),
        Direction::RowStart => find_row_start(source, candidates),
        Direction::RowEnd => find_row_end(source, candidates),
        _ => find_cardinal(source, candidates, direction),
    }
}

/// Major-axis distance from source to candidate's **near** edge along the
/// travel direction. Always non-negative for directional candidates.
fn major_axis_distance(source: &Rect, candidate: &Rect, direction: Direction) -> f64 {
    match direction {
        Direction::Right => candidate.x - source.right(),
        Direction::Left => source.x - candidate.right(),
        Direction::Down => candidate.y - source.bottom(),
        Direction::Up => source.y - candidate.bottom(),
        _ => 0.0,
    }
}

/// Major-axis distance from source to candidate's **far** edge along the
/// travel direction. Always greater than or equal to
/// [`major_axis_distance`] for the same candidate.
fn major_axis_distance_to_far_edge(source: &Rect, candidate: &Rect, direction: Direction) -> f64 {
    match direction {
        Direction::Right => candidate.right() - source.right(),
        Direction::Left => source.x - candidate.x,
        Direction::Down => candidate.bottom() - source.bottom(),
        Direction::Up => source.y - candidate.y,
        _ => 0.0,
    }
}

/// Android FocusFinder "beam beats" rule: when `in_beam` is in the
/// perpendicular beam of `source` and `out_beam` is not, in-beam wins
/// **unless** `out_beam` is dramatically closer — specifically, unless
/// its far edge is nearer than `in_beam`'s near edge along the travel
/// direction.
///
/// This is the fix for the cross-region starvation bug: when the
/// nearest in-beam candidate is very far (e.g. from a LeftNav button
/// the only in-beam hit is a toolbar pill on the far right), a close
/// out-of-beam candidate (e.g. a row selector just below the LeftNav)
/// gets to win instead of being silently culled by the pool-split.
fn in_beam_dominates(source: &Rect, in_beam: &Rect, out_beam: &Rect, direction: Direction) -> bool {
    major_axis_distance(source, in_beam, direction)
        < major_axis_distance_to_far_edge(source, out_beam, direction)
}

/// Cardinal direction navigation with beam test and scoring.
///
/// Candidates split into two pools by the perpendicular-beam test. The
/// best candidate within each pool is chosen by Android FocusFinder's
/// `13 * major² + minor²` score. The two pool-winners are then arbitrated
/// by [`in_beam_dominates`]: the in-beam winner is preferred unless the
/// out-of-beam winner is dramatically closer along the travel axis.
///
/// This unified ranking prevents the failure mode where a far-away
/// in-beam candidate silently starves a close, obviously-correct
/// out-of-beam candidate — the bug that broke cross-region moves such
/// as LeftNav → grid row selector.
fn find_cardinal(
    source: &SpatialEntry,
    candidates: &[&SpatialEntry],
    direction: Direction,
) -> Option<String> {
    let mut best_in_beam: Option<(&SpatialEntry, f64)> = None;
    let mut best_out_beam: Option<(&SpatialEntry, f64)> = None;

    for &c in candidates {
        if !is_in_direction(&source.rect, &c.rect, direction) {
            continue;
        }
        let s = score(&source.rect, &c.rect, direction);
        let bucket = if is_in_beam(&source.rect, &c.rect, direction) {
            &mut best_in_beam
        } else {
            &mut best_out_beam
        };
        match bucket {
            Some((_, bs)) if *bs <= s => {}
            _ => *bucket = Some((c, s)),
        }
    }

    match (best_in_beam, best_out_beam) {
        (Some((in_entry, _)), Some((out_entry, _))) => {
            if in_beam_dominates(&source.rect, &in_entry.rect, &out_entry.rect, direction) {
                Some(in_entry.key.clone())
            } else {
                Some(out_entry.key.clone())
            }
        }
        (Some((entry, _)), None) | (None, Some((entry, _))) => Some(entry.key.clone()),
        (None, None) => None,
    }
}

/// First: topmost-leftmost candidate, sorted by (y, x).
fn find_edge_first(candidates: &[&SpatialEntry]) -> Option<String> {
    candidates
        .iter()
        .min_by(|a, b| {
            a.rect
                .y
                .partial_cmp(&b.rect.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.rect
                        .x
                        .partial_cmp(&b.rect.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .map(|e| e.key.clone())
}

/// Last: bottommost-rightmost candidate, sorted by (y desc, x desc).
fn find_edge_last(candidates: &[&SpatialEntry]) -> Option<String> {
    candidates
        .iter()
        .max_by(|a, b| {
            a.rect
                .bottom()
                .partial_cmp(&b.rect.bottom())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.rect
                        .right()
                        .partial_cmp(&b.rect.right())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .map(|e| e.key.clone())
}

/// RowStart: leftmost candidate overlapping the source's y-range.
fn find_row_start(source: &SpatialEntry, candidates: &[&SpatialEntry]) -> Option<String> {
    candidates
        .iter()
        .filter(|c| overlaps_y_range(&source.rect, &c.rect))
        .min_by(|a, b| {
            a.rect
                .x
                .partial_cmp(&b.rect.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|e| e.key.clone())
}

/// RowEnd: rightmost candidate overlapping the source's y-range.
fn find_row_end(source: &SpatialEntry, candidates: &[&SpatialEntry]) -> Option<String> {
    candidates
        .iter()
        .filter(|c| overlaps_y_range(&source.rect, &c.rect))
        .max_by(|a, b| {
            a.rect
                .right()
                .partial_cmp(&b.rect.right())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|e| e.key.clone())
}

/// Container-first navigation: search siblings in the same parent scope
/// first, then fall back to the full candidate set.
///
/// If the source has no `parent_scope`, skips the scoped search and goes
/// directly to the full candidate set.
///
/// **Caller must exclude the source entry from `candidates`** — same
/// contract as [`find_target`].
pub fn container_first_search(
    source: &SpatialEntry,
    candidates: &[&SpatialEntry],
    direction: Direction,
) -> Option<String> {
    if let Some(ref scope) = source.parent_scope {
        let scoped: Vec<&SpatialEntry> = candidates
            .iter()
            .filter(|c| c.parent_scope.as_deref() == Some(scope))
            .copied()
            .collect();
        if let Some(key) = find_target(source, &scoped, direction) {
            return Some(key);
        }
    }
    find_target(source, candidates, direction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial_state::Rect;

    /// Helper to create a `SpatialEntry` with minimal fields for navigation tests.
    fn entry(key: &str, x: f64, y: f64, w: f64, h: f64) -> SpatialEntry {
        SpatialEntry {
            key: key.to_string(),
            moniker: key.to_string(),
            rect: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            layer_key: "layer-1".to_string(),
            parent_scope: None,
            overrides: std::collections::HashMap::new(),
        }
    }

    /// Helper to create a `SpatialEntry` with a parent scope.
    fn entry_scoped(key: &str, x: f64, y: f64, w: f64, h: f64, scope: &str) -> SpatialEntry {
        SpatialEntry {
            key: key.to_string(),
            moniker: key.to_string(),
            rect: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            layer_key: "layer-1".to_string(),
            parent_scope: Some(scope.to_string()),
            overrides: std::collections::HashMap::new(),
        }
    }

    // --- Beam test ---

    #[test]
    fn beam_candidate_preferred_over_closer_out_of_beam() {
        let source = entry("source", 0.0, 100.0, 100.0, 50.0);
        // A: in beam (y overlap with source), 50px right of source.right()
        let a = entry("A", 150.0, 120.0, 50.0, 30.0);
        // B: out of beam (y=10, above source), only 10px right of source.right()
        let b = entry("B", 110.0, 10.0, 50.0, 30.0);

        let candidates = vec![&a, &b];
        let result = find_target(&source, &candidates, Direction::Right);
        assert_eq!(
            result.as_deref(),
            Some("A"),
            "in-beam candidate should win over closer out-of-beam"
        );
    }

    // --- Scoring ---

    #[test]
    fn aligned_candidate_beats_closer_diagonal() {
        let source = entry("source", 0.0, 0.0, 100.0, 50.0);
        // A: aligned, 100px right — in beam (y overlap)
        let a = entry("A", 200.0, 0.0, 100.0, 50.0);
        // B: diagonal, 10px right but y=200 — out of beam (y=200 > source.bottom=50)
        let b = entry("B", 110.0, 200.0, 100.0, 50.0);

        let candidates = vec![&a, &b];
        let result = find_target(&source, &candidates, Direction::Right);
        assert_eq!(
            result.as_deref(),
            Some("A"),
            "A is in beam, B is out of beam — A wins by beam"
        );
    }

    #[test]
    fn scoring_tiebreak_among_in_beam_candidates() {
        let source = entry("source", 0.0, 0.0, 100.0, 100.0);
        // A: aligned, 100px right, center offset = 0
        //   score = 13 * 100² + 0² = 130_000
        let a = entry("A", 200.0, 0.0, 100.0, 100.0);
        // B: 50px right, center offset = 20
        //   score = 13 * 50² + 20² = 32_500 + 400 = 32_900
        let b = entry("B", 150.0, 20.0, 100.0, 100.0);

        let candidates = vec![&a, &b];
        let result = find_target(&source, &candidates, Direction::Right);
        assert_eq!(
            result.as_deref(),
            Some("B"),
            "B has lower score (32_900 < 130_000)"
        );
    }

    // --- Board layout: 3 columns × varying cards ---

    #[test]
    fn board_layout_3_columns() {
        // Column 0: header + 5 cards
        let c0_header = entry("c0-header", 0.0, 0.0, 200.0, 40.0);
        let c0_card0 = entry("c0-card0", 0.0, 50.0, 200.0, 60.0);
        let c0_card1 = entry("c0-card1", 0.0, 120.0, 200.0, 60.0);
        let c0_card2 = entry("c0-card2", 0.0, 190.0, 200.0, 60.0);
        let c0_card3 = entry("c0-card3", 0.0, 260.0, 200.0, 60.0);
        let c0_card4 = entry("c0-card4", 0.0, 330.0, 200.0, 60.0);

        // Column 1: header + 5 cards
        let c1_header = entry("c1-header", 210.0, 0.0, 200.0, 40.0);
        let c1_card0 = entry("c1-card0", 210.0, 50.0, 200.0, 60.0);
        let c1_card1 = entry("c1-card1", 210.0, 120.0, 200.0, 60.0);
        let c1_card2 = entry("c1-card2", 210.0, 190.0, 200.0, 60.0);
        let c1_card3 = entry("c1-card3", 210.0, 260.0, 200.0, 60.0);
        let c1_card4 = entry("c1-card4", 210.0, 330.0, 200.0, 60.0);

        // Column 2: header + 3 cards only
        let c2_header = entry("c2-header", 420.0, 0.0, 200.0, 40.0);
        let c2_card0 = entry("c2-card0", 420.0, 50.0, 200.0, 60.0);
        let c2_card1 = entry("c2-card1", 420.0, 120.0, 200.0, 60.0);
        let c2_card2 = entry("c2-card2", 420.0, 190.0, 200.0, 60.0);

        let all: Vec<&SpatialEntry> = vec![
            &c0_header, &c0_card0, &c0_card1, &c0_card2, &c0_card3, &c0_card4, &c1_header,
            &c1_card0, &c1_card1, &c1_card2, &c1_card3, &c1_card4, &c2_header, &c2_card0,
            &c2_card1, &c2_card2,
        ];

        // Helper: candidates = all minus source
        let without = |source_key: &str| -> Vec<&SpatialEntry> {
            all.iter()
                .filter(|e| e.key != source_key)
                .copied()
                .collect()
        };

        // 1. nav.down from col0.card[0] → col0.card[1]
        let cands = without("c0-card0");
        assert_eq!(
            find_target(&c0_card0, &cands, Direction::Down).as_deref(),
            Some("c0-card1"),
        );

        // 2. nav.right from col0.card[0] → col1.card[0] (beam-aligned)
        assert_eq!(
            find_target(&c0_card0, &cands, Direction::Right).as_deref(),
            Some("c1-card0"),
        );

        // 3. nav.right from col0.card[4] → nearest in col2 by beam
        //    col0.card4 is at y=330..390. col2 only has cards up to y=190..250.
        //    col1.card4 at y=330..390 is beam-aligned — should win.
        //    After col1, col2 has no card at that y, so nav.right from col1.card4
        //    would look at col2. But the task says from col0.card4 → col2 clamped.
        //    Actually col1.card4 is in the way. Let me re-read: the test says
        //    "col2 only has 3 cards, nearest is last". Let me just test from
        //    col1.card4 → col2.card2 (last card in col2, out of beam fallback).
        let cands_c1c4 = without("c1-card4");
        assert_eq!(
            find_target(&c1_card4, &cands_c1c4, Direction::Right).as_deref(),
            Some("c2-card2"),
            "col1.card4 right → col2.card2 (nearest col2 card, out-of-beam fallback)"
        );

        // 4. nav.up from col0.card[0] → col0.header
        assert_eq!(
            find_target(&c0_card0, &cands, Direction::Up).as_deref(),
            Some("c0-header"),
        );

        // 5. nav.right from col2.card[2] → None (no column to the right)
        let cands_c2c2 = without("c2-card2");
        assert_eq!(find_target(&c2_card2, &cands_c2c2, Direction::Right), None,);

        // 6. nav.first → col0.header (top-left-most)
        assert_eq!(
            find_target(&c0_card0, &cands, Direction::First).as_deref(),
            Some("c0-header"),
        );

        // 7. nav.last → c1-card4 (bottom-right-most by y=330..390, then x=210..410)
        assert_eq!(
            find_target(&c0_card0, &cands, Direction::Last).as_deref(),
            Some("c1-card4"),
        );
    }

    #[test]
    fn empty_column_nav_right_lands_on_header() {
        // Column 0 with header + 5 cards
        let c0_header = entry("c0-header", 0.0, 0.0, 200.0, 40.0);
        let c0_card0 = entry("c0-card0", 0.0, 50.0, 200.0, 60.0);
        let c0_card1 = entry("c0-card1", 0.0, 120.0, 200.0, 60.0);
        let c0_card2 = entry("c0-card2", 0.0, 190.0, 200.0, 60.0);
        let c0_card3 = entry("c0-card3", 0.0, 260.0, 200.0, 60.0);
        let c0_card4 = entry("c0-card4", 0.0, 330.0, 200.0, 60.0);

        // Column 1 with header only (no cards)
        let c1_header = entry("c1-header", 210.0, 0.0, 200.0, 40.0);

        let all: Vec<&SpatialEntry> = vec![
            &c0_header, &c0_card0, &c0_card1, &c0_card2, &c0_card3, &c0_card4, &c1_header,
        ];
        let cands: Vec<&SpatialEntry> = all
            .iter()
            .filter(|e| e.key != "c0-card2")
            .copied()
            .collect();

        assert_eq!(
            find_target(&c0_card2, &cands, Direction::Right).as_deref(),
            Some("c1-header"),
            "only element in col1's x-range is the header"
        );
    }

    // --- Inspector layout: stacked field rows ---

    #[test]
    fn inspector_8_stacked_fields() {
        let fields: Vec<SpatialEntry> = (0..8)
            .map(|i| entry(&format!("field-{i}"), 0.0, (i as f64) * 40.0, 300.0, 35.0))
            .collect();
        let refs: Vec<&SpatialEntry> = fields.iter().collect();

        // nav.down from field[0] → field[1]
        let cands: Vec<&SpatialEntry> = refs
            .iter()
            .filter(|e| e.key != "field-0")
            .copied()
            .collect();
        assert_eq!(
            find_target(&fields[0], &cands, Direction::Down).as_deref(),
            Some("field-1")
        );

        // nav.up from field[7] → field[6]
        let cands: Vec<&SpatialEntry> = refs
            .iter()
            .filter(|e| e.key != "field-7")
            .copied()
            .collect();
        assert_eq!(
            find_target(&fields[7], &cands, Direction::Up).as_deref(),
            Some("field-6")
        );

        // nav.first → field[0]
        assert_eq!(
            find_target(&fields[3], &cands, Direction::First).as_deref(),
            Some("field-0")
        );

        // nav.last → field[7] — but field-7 is excluded from cands above, use full set
        let cands_full: Vec<&SpatialEntry> = refs
            .iter()
            .filter(|e| e.key != "field-3")
            .copied()
            .collect();
        assert_eq!(
            find_target(&fields[3], &cands_full, Direction::Last).as_deref(),
            Some("field-7")
        );

        // nav.left from field[3] → None (nothing to the left — all same x)
        assert_eq!(find_target(&fields[3], &cands_full, Direction::Left), None);
    }

    // --- Pill horizontal navigation ---

    #[test]
    fn pill_horizontal_navigation() {
        let label = entry("label", 0.0, 0.0, 100.0, 30.0);
        let pill_a = entry("pill-A", 110.0, 5.0, 60.0, 20.0);
        let pill_b = entry("pill-B", 180.0, 5.0, 60.0, 20.0);
        let pill_c = entry("pill-C", 250.0, 5.0, 60.0, 20.0);

        let all: Vec<&SpatialEntry> = vec![&label, &pill_a, &pill_b, &pill_c];

        // nav.right from label → Pill A
        let cands: Vec<&SpatialEntry> = all.iter().filter(|e| e.key != "label").copied().collect();
        assert_eq!(
            find_target(&label, &cands, Direction::Right).as_deref(),
            Some("pill-A")
        );

        // nav.right from Pill A → Pill B
        let cands: Vec<&SpatialEntry> = all.iter().filter(|e| e.key != "pill-A").copied().collect();
        assert_eq!(
            find_target(&pill_a, &cands, Direction::Right).as_deref(),
            Some("pill-B")
        );

        // nav.left from Pill A → label
        let cands: Vec<&SpatialEntry> = all.iter().filter(|e| e.key != "pill-A").copied().collect();
        assert_eq!(
            find_target(&pill_a, &cands, Direction::Left).as_deref(),
            Some("label")
        );

        // nav.left from Pill C → Pill B
        let cands: Vec<&SpatialEntry> = all.iter().filter(|e| e.key != "pill-C").copied().collect();
        assert_eq!(
            find_target(&pill_c, &cands, Direction::Left).as_deref(),
            Some("pill-B")
        );
    }

    // --- Container-first search ---

    #[test]
    fn container_first_stays_in_parent_scope() {
        let btn_a = entry_scoped("btn-A", 0.0, 0.0, 80.0, 30.0, "toolbar");
        let btn_b = entry_scoped("btn-B", 90.0, 0.0, 80.0, 30.0, "toolbar");
        let card = entry_scoped("card", 0.0, 40.0, 200.0, 60.0, "column:todo");

        let candidates: Vec<&SpatialEntry> = vec![&btn_b, &card];
        let result = container_first_search(&btn_a, &candidates, Direction::Right);
        assert_eq!(
            result.as_deref(),
            Some("btn-B"),
            "same parent scope sibling wins"
        );
    }

    #[test]
    fn container_fallback_when_no_sibling() {
        let btn = entry_scoped("btn", 0.0, 0.0, 80.0, 30.0, "toolbar");
        let card = entry_scoped("card", 0.0, 40.0, 200.0, 60.0, "column:todo");

        // No other toolbar entries → fall through to full layer
        let candidates: Vec<&SpatialEntry> = vec![&card];
        let result = container_first_search(&btn, &candidates, Direction::Down);
        assert_eq!(
            result.as_deref(),
            Some("card"),
            "falls through to full layer when no sibling"
        );
    }

    // --- RowStart / RowEnd ---

    #[test]
    fn navigate_rowstart_rowend() {
        // 3x3 grid
        let entries: Vec<SpatialEntry> = (0..3)
            .flat_map(|row| {
                (0..3).map(move |col| {
                    entry(
                        &format!("cell-{col}-{row}"),
                        (col as f64) * 110.0,
                        (row as f64) * 50.0,
                        100.0,
                        40.0,
                    )
                })
            })
            .collect();
        let refs: Vec<&SpatialEntry> = entries.iter().collect();

        // Focused on center (1,1)
        let center = &entries[4]; // cell-1-1
        let cands: Vec<&SpatialEntry> = refs
            .iter()
            .filter(|e| e.key != "cell-1-1")
            .copied()
            .collect();

        // RowStart → leftmost in row 1 = cell-0-1
        assert_eq!(
            find_target(center, &cands, Direction::RowStart).as_deref(),
            Some("cell-0-1"),
        );

        // RowEnd → rightmost in row 1 = cell-2-1
        assert_eq!(
            find_target(center, &cands, Direction::RowEnd).as_deref(),
            Some("cell-2-1"),
        );
    }

    // --- Cross-region navigation (beam-starvation regression) ---
    //
    // These tests exercise the scenario described in the kanban task: a
    // source sits in one screen region (LeftNav, perspective tab bar),
    // the "natural" next focus target sits in another region with no
    // perpendicular-axis overlap, and the pool-split culling incorrectly
    // kept focus pinned to a far-away in-beam candidate. The fix widens
    // the choice so dramatically closer out-of-beam candidates can win.

    #[test]
    fn right_from_leftnav_reaches_grid_row_selector_when_no_in_beam_candidate_closer() {
        // LeftNav sits in the top-left corner, 40px tall near the top.
        let leftnav_btn = entry("leftnav-board", 0.0, 100.0, 40.0, 40.0);
        // A far-off in-beam candidate that is vertically overlapping the
        // LeftNav button (shares a sliver of y-range) but is pushed all
        // the way to the right — a noisy toolbar icon, say.
        //
        // Its x starts at 900, well beyond the row selector at x=60.
        // Previously this "wins" over the closer out-of-beam row
        // selector because `in_beam` is non-empty → pool-split culls the
        // row selector. With the fix it must lose: the row selector's
        // *far edge* (x=80) is closer than this button's *near edge*
        // (x=900), so the out-of-beam candidate dominates.
        let far_in_beam = entry("far-toolbar", 900.0, 110.0, 40.0, 20.0);
        // Row selector: the westernmost grid-body cell, sitting below
        // the LeftNav button with zero y-overlap.
        let row_selector = entry("row-selector-0", 60.0, 200.0, 20.0, 36.0);

        let cands = vec![&far_in_beam, &row_selector];
        assert_eq!(
            find_target(&leftnav_btn, &cands, Direction::Right).as_deref(),
            Some("row-selector-0"),
            "far in-beam candidate must not starve close out-of-beam row selector"
        );
    }

    #[test]
    fn right_from_leftnav_prefers_in_beam_candidate_when_available() {
        // Same LeftNav source as above, but this time the in-beam
        // candidate is a reasonable neighbour (close to the right, same
        // vertical band). It must still win over a dramatically more
        // distant out-of-beam candidate.
        let leftnav_btn = entry("leftnav-board", 0.0, 100.0, 40.0, 40.0);
        // Close in-beam candidate — 50 px to the right, overlapping y.
        let close_in_beam = entry("close-in-beam", 90.0, 110.0, 40.0, 20.0);
        // Far out-of-beam candidate — below and farther right.
        let far_out_beam = entry("far-out-beam", 500.0, 400.0, 20.0, 20.0);

        let cands = vec![&close_in_beam, &far_out_beam];
        assert_eq!(
            find_target(&leftnav_btn, &cands, Direction::Right).as_deref(),
            Some("close-in-beam"),
            "in-beam candidate must win when it is a reasonable neighbour"
        );
    }

    #[test]
    fn left_from_grid_row_selector_reaches_leftnav() {
        // Symmetry: starting from the row selector (wedge in the grid
        // body), Left must reach back onto the LeftNav button even
        // though the LeftNav button has no vertical overlap.
        let row_selector = entry("row-selector-0", 60.0, 200.0, 20.0, 36.0);
        let leftnav_btn = entry("leftnav-board", 0.0, 100.0, 40.0, 40.0);
        // A noisy in-beam distractor to the left (e.g. a sidebar cell
        // that happens to overlap y but is very far horizontally).
        let far_in_beam_left = entry("far-distractor", -900.0, 210.0, 40.0, 20.0);

        let cands = vec![&leftnav_btn, &far_in_beam_left];
        assert_eq!(
            find_target(&row_selector, &cands, Direction::Left).as_deref(),
            Some("leftnav-board"),
            "close out-of-beam LeftNav button must beat far in-beam distractor"
        );
    }

    #[test]
    fn down_from_perspective_bar_reaches_grid_header_when_no_horizontal_overlap() {
        // Perspective tab bar sits at the top, away from the first
        // content scope which begins further down and shifted right —
        // no horizontal overlap with the tab bar button.
        let tab_btn = entry("perspective-tab-active", 0.0, 40.0, 120.0, 30.0);
        // Far in-beam candidate (horizontally overlaps tab_btn, but
        // way down the screen).
        let far_in_beam_down = entry("far-footer", 50.0, 2000.0, 40.0, 20.0);
        // Grid header sits just below the tab bar, shifted to the
        // right (no horizontal overlap with tab_btn's x=[0,120]).
        let grid_header = entry("grid-header", 300.0, 80.0, 200.0, 30.0);

        let cands = vec![&far_in_beam_down, &grid_header];
        assert_eq!(
            find_target(&tab_btn, &cands, Direction::Down).as_deref(),
            Some("grid-header"),
            "close out-of-beam grid header must beat far in-beam footer"
        );
    }

    // --- Direction parsing ---

    #[test]
    fn direction_from_str() {
        assert_eq!("up".parse::<Direction>().unwrap(), Direction::Up);
        assert_eq!("Down".parse::<Direction>().unwrap(), Direction::Down);
        assert_eq!("LEFT".parse::<Direction>().unwrap(), Direction::Left);
        assert_eq!(
            "RowStart".parse::<Direction>().unwrap(),
            Direction::RowStart
        );
        assert!("diagonal".parse::<Direction>().is_err());
    }
}
