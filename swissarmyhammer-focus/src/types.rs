//! Newtypes for the spatial-navigation kernel.
//!
//! Every signature in the spatial-nav surface uses one of the newtypes
//! defined here — never a bare `String` or `f64`. This is enforced by the
//! task spec: the frontend mirrors these as TypeScript branded types so
//! mixing up a [`WindowLabel`] and a [`FullyQualifiedMoniker`] is a
//! compile error on both sides of the Tauri boundary.
//!
//! All string-valued newtypes are produced by the canonical
//! [`swissarmyhammer_common::define_id!`] macro, matching the pattern used
//! throughout the workspace (`TaskId`, `ColumnId`, `TagId`, …). This buys
//! us `#[serde(transparent)]`, `Display`, `AsRef<str>`, `From<&str>`,
//! `From<String>`, `Deref<str>`, `FromStr`, and `PartialEq<str>` for free,
//! plus `new()` (fresh ULID) and `from_string()` constructors.
//!
//! # Path-monikers identifier model
//!
//! The kernel uses **one** identifier shape per primitive — the
//! [`FullyQualifiedMoniker`]. The path through the focus hierarchy
//! IS the spatial key. Consumers declare a relative [`SegmentMoniker`]
//! when constructing a `<FocusLayer>` / `<FocusZone>` / `<FocusScope>`
//! and the FQM is composed by parent/child nesting on the consumer side
//! before being passed to the kernel.
//!
//! There is no UUID-based `SpatialKey` and no flat `Moniker`. Path is
//! the key, the key is exact-match. See the parent path-monikers card
//! (`01KQD6064G1C1RAXDFPJVT1F46`) for the structural-bug rationale: with
//! flat `Moniker`s the inspector's `field:T1.title` zone collided with
//! the board's `field:T1.title` zone in the registry's lookup table,
//! and `find_by_moniker` resolved non-deterministically. FQMs eliminate
//! the collision by construction.
//!
//! [`Pixels`] is the only numeric newtype on the spatial-nav surface, so
//! it is hand-rolled with arithmetic ops (`+`, `-`, `*`, `/`) that keep
//! beam-search and rect math in newtype-land. `Rect` composes four
//! `Pixels` values plus the standard edge accessors (`top`, `left`,
//! `bottom`, `right`).
//!
//! The cardinal `Direction` enum lives here too because it is the only
//! non-string identity-like value on the spatial-nav signatures.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

use swissarmyhammer_common::define_id;

define_id!(
    WindowLabel,
    "Tauri window label — which window a scope/layer lives in"
);
define_id!(
    SegmentMoniker,
    "Relative path segment declared by a consumer, e.g. \"field:T1.title\", \"card:T1\", \"inspector\". Composed with a parent FQM via `FullyQualifiedMoniker::compose` to form a canonical key."
);
define_id!(
    FullyQualifiedMoniker,
    "Canonical path through the focus hierarchy, e.g. \"/window/inspector/field:T1.title\". The FQM IS the spatial key — used as the registry key, the focus identity, and the wire-format identifier on every spatial-nav IPC."
);
define_id!(
    LayerName,
    "Layer role: \"window\", \"inspector\", \"dialog\", \"palette\""
);

/// Path separator used by [`FullyQualifiedMoniker::compose`] and
/// [`FullyQualifiedMoniker::root`]. Mirrors the `/window/...` shape the
/// React side composes via `FullyQualifiedMonikerContext`. Separator
/// choice is a wire-format detail; both sides agree on `'/'`.
const FQ_SEPARATOR: char = '/';

impl FullyQualifiedMoniker {
    /// Construct the FQM for a **layer root** — the topmost segment in
    /// a window's spatial hierarchy. The result is `"/<segment>"`,
    /// e.g. `FullyQualifiedMoniker::root(&seg("window"))` produces
    /// `/window`.
    ///
    /// Layer roots are the only primitives constructed without a
    /// parent FQM — every other zone or scope composes against a
    /// parent via [`Self::compose`].
    pub fn root(segment: &SegmentMoniker) -> Self {
        Self(format!("{FQ_SEPARATOR}{}", segment.as_str()))
    }

    /// Compose a child FQM by appending `segment` to `parent` with the
    /// path separator. The result is `"<parent>/<segment>"`.
    ///
    /// This is the only way to construct a non-root FQM in well-formed
    /// kernel code. The React adapter performs the same composition via
    /// `FullyQualifiedMonikerContext` so the kernel and React agree on
    /// the canonical path string for every primitive.
    ///
    /// # Examples
    ///
    /// ```
    /// # use swissarmyhammer_focus::{FullyQualifiedMoniker, SegmentMoniker};
    /// let window = FullyQualifiedMoniker::root(&SegmentMoniker::from_string("window"));
    /// let inspector = FullyQualifiedMoniker::compose(
    ///     &window,
    ///     &SegmentMoniker::from_string("inspector"),
    /// );
    /// assert_eq!(inspector.as_str(), "/window/inspector");
    /// ```
    pub fn compose(parent: &FullyQualifiedMoniker, segment: &SegmentMoniker) -> Self {
        Self(format!(
            "{}{FQ_SEPARATOR}{}",
            parent.as_str(),
            segment.as_str()
        ))
    }
}

/// Navigation direction passed to `spatial_navigate`.
///
/// Includes the four cardinal arrows plus four "edge" commands that
/// jump to the boundaries of the active scope:
///
/// - [`Direction::First`] / [`Direction::Last`] jump to the topmost-leftmost
///   / bottommost-rightmost candidate at the focused level (in-zone for
///   leaves, sibling-zone for zones). Wired to Home / End style keymap
///   entries on the React side.
/// - [`Direction::RowStart`] / [`Direction::RowEnd`] jump to the leftmost
///   / rightmost candidate whose vertical extent overlaps the focused
///   rect — i.e. the start / end of the focused row. Wired to
///   Cmd-Left / Cmd-Right style keymap entries.
///
/// Drill-in / drill-out are **separate commands** (see the corresponding
/// task card), not directions.
///
/// Serializes to lower-case identifiers (`"up"`, `"down"`, `"left"`,
/// `"right"`, `"first"`, `"last"`, `"rowstart"`, `"rowend"`) so the
/// TypeScript side can mirror the variants as a string-literal union
/// without bridging glue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Move toward decreasing screen-y (visually upward).
    Up,
    /// Move toward increasing screen-y (visually downward).
    Down,
    /// Move toward decreasing screen-x (visually leftward).
    Left,
    /// Move toward increasing screen-x (visually rightward).
    Right,
    /// Jump to the topmost-leftmost candidate at the focused level.
    First,
    /// Jump to the bottommost-rightmost candidate at the focused level.
    Last,
    /// Jump to the leftmost candidate whose vertical extent overlaps
    /// the focused rect.
    RowStart,
    /// Jump to the rightmost candidate whose vertical extent overlaps
    /// the focused rect.
    RowEnd,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => f.write_str("up"),
            Self::Down => f.write_str("down"),
            Self::Left => f.write_str("left"),
            Self::Right => f.write_str("right"),
            Self::First => f.write_str("first"),
            Self::Last => f.write_str("last"),
            Self::RowStart => f.write_str("rowstart"),
            Self::RowEnd => f.write_str("rowend"),
        }
    }
}

/// A measurement in CSS pixels.
///
/// Hand-rolled rather than derived from `define_id!` because the underlying
/// representation is `f64`, not a string. Carries the arithmetic operators
/// the beam-search / rect-math callers need so a stray `.0` cannot drop
/// the type-safety the rest of the spatial-nav surface enforces.
///
/// `#[serde(transparent)]` mirrors the wire shape used by the string
/// newtypes — `Pixels::new(13.5)` serializes as the bare number `13.5`,
/// not `{"0": 13.5}`. The frontend mirrors this as a branded `number`
/// type.
///
/// The inner `f64` is **private**: every read goes through [`Pixels::value`]
/// or one of the arithmetic ops below, so `.0` cannot leak the
/// type-safety we paid for with the newtype. Construction is via
/// [`Pixels::new`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pixels(f64);

impl Pixels {
    /// Construct a `Pixels` value from a raw `f64`. The only entry point
    /// from raw numeric literals into newtype-land — every other surface
    /// returns or operates on an existing `Pixels`, so a single grep for
    /// `Pixels::new(` finds every place numerics enter the spatial-nav
    /// surface.
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// Read the inner `f64`. Provided for the rare cases where escaping
    /// the newtype is genuinely needed (e.g. computing distances against
    /// a non-`Pixels` constant). Inside this crate, prefer the
    /// arithmetic ops below so type-safety stays end-to-end.
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Total-ordering comparator for [`Pixels`] values.
///
/// `Pixels` wraps an `f64`, which only implements `PartialOrd` because
/// of `NaN`. Spatial-nav rects come from CSS layout and are always
/// finite, so `NaN` is unreachable in well-formed registries — but
/// `unwrap_or(Equal)` keeps the kernel panic-free if one ever sneaks
/// in (e.g. from a malformed test fixture).
///
/// Used by the navigator's edge-command ordering and by the registry's
/// drill-in first-child fallback. Both call sites need a chainable
/// `Ordering` (so they can break ties on a secondary axis), which
/// rules out `Pixels: Ord` via a wrapper type — a free function keeps
/// the call sites idiomatic.
pub(crate) fn pixels_cmp(a: Pixels, b: Pixels) -> std::cmp::Ordering {
    a.value()
        .partial_cmp(&b.value())
        .unwrap_or(std::cmp::Ordering::Equal)
}

impl fmt::Display for Pixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}px", self.0)
    }
}

impl Add for Pixels {
    type Output = Pixels;

    /// Add two `Pixels` values. Used by `Rect::bottom`, `Rect::right`, and
    /// every beam-search candidate-distance computation.
    fn add(self, rhs: Self) -> Self::Output {
        Pixels::new(self.0 + rhs.0)
    }
}

impl Sub for Pixels {
    type Output = Pixels;

    /// Subtract one `Pixels` value from another. Used in beam search to
    /// compute the gap between two rect edges.
    fn sub(self, rhs: Self) -> Self::Output {
        Pixels::new(self.0 - rhs.0)
    }
}

impl Mul<f64> for Pixels {
    type Output = Pixels;

    /// Scale a `Pixels` value by a unitless `f64` multiplier. Used by
    /// beam-search beam-width computations (e.g. a 0.5×rect-height beam).
    fn mul(self, rhs: f64) -> Self::Output {
        Pixels::new(self.0 * rhs)
    }
}

impl Div<f64> for Pixels {
    type Output = Pixels;

    /// Divide a `Pixels` value by a unitless `f64` divisor. Used to find
    /// midpoints (`width / 2.0`) and similar geometric helpers.
    fn div(self, rhs: f64) -> Self::Output {
        Pixels::new(self.0 / rhs)
    }
}

/// Axis-aligned rectangle in CSS pixel coordinates, with the origin at the
/// top-left of the surrounding window viewport.
///
/// `x` / `y` mark the rectangle's top-left corner; `width` / `height` are
/// non-negative extents. The four edge accessors (`top`, `left`, `bottom`,
/// `right`) derive the opposite edges so beam search and overlap math do
/// not have to spell out `x + width` everywhere.
///
/// `Copy` because `Pixels` is `Copy`; rects are passed by value through
/// the spatial-nav signatures the same way `Rect` is in the browser DOM.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Distance from the viewport's left edge to the rect's left edge.
    pub x: Pixels,
    /// Distance from the viewport's top edge to the rect's top edge.
    pub y: Pixels,
    /// Horizontal extent of the rect; non-negative.
    pub width: Pixels,
    /// Vertical extent of the rect; non-negative.
    pub height: Pixels,
}

impl Rect {
    /// Top edge of the rect, equal to `y`.
    pub fn top(&self) -> Pixels {
        self.y
    }

    /// Left edge of the rect, equal to `x`.
    pub fn left(&self) -> Pixels {
        self.x
    }

    /// Bottom edge of the rect, computed as `y + height`.
    pub fn bottom(&self) -> Pixels {
        self.y + self.height
    }

    /// Right edge of the rect, computed as `x + width`.
    pub fn right(&self) -> Pixels {
        self.x + self.width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each newtype JSON-round-trips as a bare primitive — the
    /// `#[serde(transparent)]` attribute from `define_id!` means the wire
    /// shape is just the inner string, not `{"0": "..."}`.
    #[test]
    fn newtypes_serialize_as_bare_strings() {
        let label = WindowLabel::from_string("main");
        let segment = SegmentMoniker::from_string("field:T1.title");
        let fq = FullyQualifiedMoniker::from_string("/window/inspector/field:T1.title");
        let name = LayerName::from_string("inspector");

        assert_eq!(serde_json::to_string(&label).unwrap(), "\"main\"");
        assert_eq!(
            serde_json::to_string(&segment).unwrap(),
            "\"field:T1.title\""
        );
        assert_eq!(
            serde_json::to_string(&fq).unwrap(),
            "\"/window/inspector/field:T1.title\""
        );
        assert_eq!(serde_json::to_string(&name).unwrap(), "\"inspector\"");
    }

    /// `FullyQualifiedMoniker::root` produces a leading-slash path with
    /// the supplied segment. Every layer root in the kernel is built
    /// this way so the path shape is uniform across the registry.
    #[test]
    fn fq_root_prefixes_separator() {
        let window = FullyQualifiedMoniker::root(&SegmentMoniker::from_string("window"));
        assert_eq!(window.as_str(), "/window");
    }

    /// `FullyQualifiedMoniker::compose` appends a segment to a parent
    /// with a single separator between them. Composition is the only
    /// path through which non-root FQMs are minted in well-formed
    /// kernel code.
    #[test]
    fn fq_compose_appends_segment() {
        let window = FullyQualifiedMoniker::root(&SegmentMoniker::from_string("window"));
        let inspector =
            FullyQualifiedMoniker::compose(&window, &SegmentMoniker::from_string("inspector"));
        assert_eq!(inspector.as_str(), "/window/inspector");

        let field = FullyQualifiedMoniker::compose(
            &inspector,
            &SegmentMoniker::from_string("field:T1.title"),
        );
        assert_eq!(field.as_str(), "/window/inspector/field:T1.title");
    }

    /// `SegmentMoniker` and `FullyQualifiedMoniker` are distinct types
    /// — the type system rejects passing one where the other is
    /// expected. This is the safety net the path-monikers refactor
    /// relies on; an accidental `String` alias would silently let
    /// segment values reach `find_by_fq` callsites.
    #[test]
    fn segment_and_fq_are_distinct_types() {
        use std::any::TypeId;

        assert_ne!(
            TypeId::of::<SegmentMoniker>(),
            TypeId::of::<FullyQualifiedMoniker>(),
        );
    }

    /// Direction serializes to lower-case so React can mirror it as a
    /// `"up" | "down" | "left" | "right"` string-literal union.
    #[test]
    fn direction_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Direction::Up).unwrap(), "\"up\"");
        assert_eq!(serde_json::to_string(&Direction::Down).unwrap(), "\"down\"");
        assert_eq!(serde_json::to_string(&Direction::Left).unwrap(), "\"left\"");
        assert_eq!(
            serde_json::to_string(&Direction::Right).unwrap(),
            "\"right\""
        );
    }

    /// The `define_id!` macro provides `from_string` and `as_str` — sanity-
    /// check that the spatial newtypes use the same surface as the rest of
    /// the workspace's IDs.
    #[test]
    fn newtypes_use_define_id_surface() {
        let fq = FullyQualifiedMoniker::from_string("/window/inspector");
        assert_eq!(fq.as_str(), "/window/inspector");
        assert_eq!(format!("{}", fq), "/window/inspector");

        let segment = SegmentMoniker::from_string("inspector");
        assert_eq!(segment.as_str(), "inspector");
        assert_eq!(format!("{}", segment), "inspector");
    }

    /// `Pixels` arithmetic returns `Pixels`, never `f64`. The unit tests
    /// here mirror the integration coverage in `tests/focus_registry.rs`
    /// so the contract is asserted twice — once at the inner crate
    /// before linking, once across the public surface.
    #[test]
    fn pixels_arithmetic_round_trip() {
        let a = Pixels::new(8.0);
        let b = Pixels::new(2.0);

        let sum: Pixels = a + b;
        let diff: Pixels = a - b;
        let scaled: Pixels = a * 1.5;
        let halved: Pixels = a / 2.0;

        assert_eq!(sum, Pixels::new(10.0));
        assert_eq!(diff, Pixels::new(6.0));
        assert_eq!(scaled, Pixels::new(12.0));
        assert_eq!(halved, Pixels::new(4.0));
    }

    /// `Rect::bottom` and `Rect::right` derive from origin + size without
    /// touching `.0` at the call site.
    #[test]
    fn rect_edges() {
        let r = Rect {
            x: Pixels::new(1.0),
            y: Pixels::new(2.0),
            width: Pixels::new(10.0),
            height: Pixels::new(20.0),
        };
        assert_eq!(r.left(), Pixels::new(1.0));
        assert_eq!(r.top(), Pixels::new(2.0));
        assert_eq!(r.right(), Pixels::new(11.0));
        assert_eq!(r.bottom(), Pixels::new(22.0));
    }

    /// `Pixels::new` and `value` are inverses — the round-trip exists so
    /// any future representation change has a single concrete contract to
    /// preserve.
    #[test]
    fn pixels_new_and_value_round_trip() {
        let p = Pixels::new(13.5);
        assert_eq!(p.value(), 13.5);
    }
}
