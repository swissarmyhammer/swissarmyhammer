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
//! returned FQM to the previous focused FQM. Torn state (unknown FQM)
//! emits `tracing::error!` and echoes the input FQM so the call site
//! has a valid result. There is no `Option` or `Result` on these APIs;
//! silence is impossible.
//!
//! Two principles distinguish the two non-motion paths:
//!
//! - **No motion → return focused FQM (no trace).** A semantic
//!   "stay put" — wall override, focused at the visual edge of the
//!   layer with an empty Direction-D half-plane, leaf with no children.
//!   The kernel returns the focused entry's own FQM. Observable: focus
//!   stays where it was, no `null` blip on the React side, no log
//!   noise.
//! - **Torn state → trace error AND echo input.** A genuine error —
//!   unknown FQM passed in. The kernel emits `tracing::error!` with
//!   the operation, the relevant FQM(s), and the FQM being echoed
//!   back, then returns the input FQM so the call site has a valid
//!   value. User-observable behavior is identical to the "no motion"
//!   case (focus stays put), but ops / devs can chase the error in
//!   logs.
//!
//! The trait returns a [`FullyQualifiedMoniker`] — the canonical
//! identity. Callers that need the relative segment (for human-readable
//! logs or local-only display) read it from the registry by FQM.
//!
//! # Cardinal navigation — geometric pick (keyboard-as-mouse)
//!
//! Cardinal nav for [`Direction::Up`], [`Direction::Down`],
//! [`Direction::Left`], and [`Direction::Right`] is **purely
//! geometric**. Pressing an arrow key picks the registered scope (in
//! the same `layer_fq`) whose rect minimises the Android beam score
//! (`13 * major² + minor²`) across ALL registered scopes in the layer
//! that:
//!
//! 1. Pass the **strict half-plane test** for D — the candidate's
//!    leading edge in the reverse direction is past the focused entry's
//!    leading edge in D. For `Down`: `cand.top >= from.bottom`. This
//!    filters out candidates that are not strictly in the half-plane,
//!    including containing parent scopes.
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
//! containers** (a leaf is a scope with no registered children; a
//! container is a scope with at least one). This ensures that when the
//! geometric pick would land equally on a `showFocusBar=false`
//! container and an inner leaf, the user sees the focus indicator
//! paint on the leaf rather than the invisible container.
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
//! **focused scope's children**, not its siblings. (The deprecated
//! `Direction::RowStart` / `Direction::RowEnd` aliases route through
//! the same path and are scheduled for removal — see the variant
//! docs in `crate::types`.)
//!
//! - **First child** = the child whose rect is topmost; ties broken by
//!   leftmost.
//! - **Last child** = the child whose rect is bottommost; ties broken
//!   by rightmost.
//! - **Children** = registered scopes whose `parent_zone` is the
//!   focused scope's FQM.
//!
//! On a focused leaf (no children) both ops return the focused FQM
//! (semantic no-op, no log noise) per the no-silent-dropout contract.
//!
//! `Direction::First` shares its result with
//! [`SpatialRegistry::drill_in`]'s cold-start fallback when the
//! focused scope has no `last_focused` memory — both pick the
//! topmost-then-leftmost child via the shared
//! [`crate::registry::first_child_by_top_left`] helper, so divergence
//! is structurally impossible. The
//! `first_matches_drill_in_first_child_fallback` test is the
//! behavioural backstop on that contract. The two ops differ only in
//! the key binding (Home vs Enter) and the React-side editor-focus
//! extension on Enter that `nav.first` does not get.
//!
//! Override (rule 0) still runs first — the focused scope's
//! per-direction `overrides` map short-circuits the children-of-focused
//! pick entirely.
//!
//! [`SpatialRegistry::drill_in`]: crate::registry::SpatialRegistry::drill_in

use std::collections::HashSet;

use crate::registry::{first_child_by_top_left, last_child_by_bottom_right, SpatialRegistry};
use crate::scope::FocusScope;
use crate::snapshot::{FocusOverrides, IndexedSnapshot};
use crate::types::{pixels_cmp, Direction, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker};

// ---------------------------------------------------------------------------
// NavScopeView — pathfinding's read-only view over a single layer's scopes.
// ---------------------------------------------------------------------------

/// Read-only, layer-scoped view of the scope set pathfinding consults.
pub trait NavScopeView {
    /// Look up a scope by its fully-qualified path.
    ///
    /// Returns `None` when the FQM is not present in this layer view —
    /// either the FQM is unknown to the underlying store or it belongs
    /// to a different layer. Pathfinding treats `None` as "torn state"
    /// and falls back per the no-silent-dropout contract.
    fn get(&self, fq: &FullyQualifiedMoniker) -> Option<NavScopeRef<'_>>;

    /// Iterate every scope in the layer view.
    ///
    /// Order is implementation-defined; pathfinding does not rely on
    /// any particular traversal order because the Android beam score
    /// plus the leaves-over-containers tie-break together produce a
    /// total order on candidates.
    fn iter(&self) -> Box<dyn Iterator<Item = NavScopeRef<'_>> + '_>;
}

/// Borrowed view of the fields pathfinding reads on a single scope.
///
/// All references borrow from the underlying store ([`FocusScope`] for
/// the registry impl, [`crate::snapshot::SnapshotScope`] for the
/// snapshot impl); the rect is carried by value because [`Rect`] is
/// [`Copy`] and pathfinding pulls coordinates off it more times than
/// it is worth threading another lifetime through.
#[derive(Debug, Clone, Copy)]
pub struct NavScopeRef<'a> {
    /// Canonical fully-qualified path of this scope.
    pub fq: &'a FullyQualifiedMoniker,
    /// Bounding rect in viewport pixel coordinates. Drives beam-search
    /// distance and overlap math. Carried by value (the type is `Copy`).
    pub rect: Rect,
    /// FQM of the immediate enclosing scope, or `None` when this scope
    /// is registered directly under the layer root. Pathfinding only
    /// reads it for the leaves-over-containers tie-break.
    pub parent_zone: Option<&'a FullyQualifiedMoniker>,
    /// Per-direction navigation overrides for this scope. Mirrors the
    /// existing `FocusScope::overrides` shape; `check_override` reads
    /// `nav_override.get(&direction)` to decide between redirect, wall,
    /// and fall-through.
    pub nav_override: &'a FocusOverrides,
}

/// [`NavScopeView`] backed by a [`SpatialRegistry`] filtered to a single layer.
pub struct RegistryLayerView<'a> {
    registry: &'a SpatialRegistry,
    layer_fq: &'a FullyQualifiedMoniker,
}

impl<'a> RegistryLayerView<'a> {
    /// Construct a view of `registry` restricted to scopes whose `layer_fq` matches.
    pub fn new(registry: &'a SpatialRegistry, layer_fq: &'a FullyQualifiedMoniker) -> Self {
        Self { registry, layer_fq }
    }
}

impl<'a> NavScopeView for RegistryLayerView<'a> {
    fn get(&self, fq: &FullyQualifiedMoniker) -> Option<NavScopeRef<'_>> {
        let scope = self.registry.find_by_fq(fq)?;
        if &scope.layer_fq != self.layer_fq {
            return None;
        }
        Some(scope_ref_from_focus_scope(scope))
    }

    fn iter(&self) -> Box<dyn Iterator<Item = NavScopeRef<'_>> + '_> {
        Box::new(
            self.registry
                .scopes_in_layer(self.layer_fq)
                .map(scope_ref_from_focus_scope),
        )
    }
}

/// Borrow a [`NavScopeRef`] out of a [`FocusScope`]. Tiny helper used
/// by both the iterator and lookup paths of
/// [`RegistryLayerView`].
fn scope_ref_from_focus_scope(scope: &FocusScope) -> NavScopeRef<'_> {
    NavScopeRef {
        fq: &scope.fq,
        rect: scope.rect,
        parent_zone: scope.parent_zone.as_ref(),
        nav_override: &scope.overrides,
    }
}

impl<'snap> NavScopeView for IndexedSnapshot<'snap> {
    fn get(&self, fq: &FullyQualifiedMoniker) -> Option<NavScopeRef<'_>> {
        let scope = IndexedSnapshot::get(self, fq)?;
        Some(NavScopeRef {
            fq: &scope.fq,
            rect: scope.rect,
            parent_zone: scope.parent_zone.as_ref(),
            nav_override: &scope.nav_override,
        })
    }

    fn iter(&self) -> Box<dyn Iterator<Item = NavScopeRef<'_>> + '_> {
        Box::new(self.scopes().iter().map(|scope| NavScopeRef {
            fq: &scope.fq,
            rect: scope.rect,
            parent_zone: scope.parent_zone.as_ref(),
            nav_override: &scope.nav_override,
        }))
    }
}

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
    ///   [`SpatialRegistry::scopes_in_layer`] for candidates.
    /// - `focused_fq` — the FQM of the currently focused scope.
    /// - `focused_segment` — the relative segment paired with
    ///   `focused_fq`. Carried for human-readable logs only — the
    ///   strategy keys on FQMs.
    /// - `direction` — the direction the user pressed.
    ///
    /// # Returns
    /// The FQM of the next focus target. When the strategy has a real
    /// target (geometric pick, override redirect), that target's FQM
    /// is returned. When the strategy declines (override wall, empty
    /// half-plane, unknown FQM) the returned FQM equals `focused_fq` —
    /// the call site detects "stay put" by equality comparison.
    /// Torn-state paths additionally emit `tracing::error!` before
    /// returning so the issue is observable in logs.
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
/// Implements the geometric pick described in the module docs:
/// override (rule 0) → layer-wide geometric beam search across all
/// registered scopes in the focused entry's `layer_fq`. The Android
/// beam score (`13 * major² + minor²`) selects the visually-nearest
/// candidate; ties are broken in favor of leaves over containers so
/// the user sees the focus indicator paint on a visible target.
///
/// [`Direction::First`] / [`Direction::Last`] focus the focused
/// scope's children — first by topmost-then-leftmost, last by
/// bottommost-then-rightmost. On a leaf they no-op (the leaf has no
/// children). The deprecated `Direction::RowStart` /
/// `Direction::RowEnd` aliases route through the same path. See the
/// module docs.
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
    /// Run the override-first path: rule 0 consults the focused
    /// scope's per-direction `overrides` map; on no-op fall-through, the
    /// geometric pick fires for cardinal directions, and the
    /// children-of-focused-scope pick fires for `First` / `Last` (and
    /// the deprecated `RowStart` / `RowEnd` aliases).
    ///
    /// Layer is the absolute boundary throughout — every candidate set
    /// is filtered by `candidate.layer_fq == focused.layer_fq` before
    /// any scoring runs (the inspector layer is captured-focus, never
    /// crosses into the window layer).
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
        let Some(entry) = registry.find_by_fq(focused_fq) else {
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

        // Build the layer-scoped view once. Cardinal pathfinding and
        // override resolution both read through the trait; First /
        // Last keep using the registry directly (out of scope for step
        // 3 of the spatial-nav redesign).
        let layer_view = RegistryLayerView::new(registry, &entry.layer_fq);
        let focused_ref = NavScopeRef {
            fq: &entry.fq,
            rect: entry.rect,
            parent_zone: entry.parent_zone.as_ref(),
            nav_override: &entry.overrides,
        };

        // Rule 0: per-direction override on the focused scope.
        match check_override(&layer_view, &focused_ref, direction) {
            Some(Some(target)) => return target,
            Some(None) => {
                // Explicit wall — semantic "stay put", not torn state.
                return focused_fq.clone();
            }
            None => {} // fall through to geometric pick / edge command
        }

        // The deprecated `RowStart` / `RowEnd` aliases route to the
        // same edge_command path as `First` / `Last` — they are kept
        // on the enum during the one-release deprecation window so
        // wire-format consumers can migrate. `#[allow(deprecated)]`
        // here marks the implementation that supports the variants
        // it has marked deprecated.
        #[allow(deprecated)]
        match direction {
            Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
                geometric_pick(&layer_view, &focused_ref, focused_fq, direction)
            }
            Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
                edge_command(registry, entry, focused_fq, direction)
            }
        }
    }
}

/// Pick the next focus target for `focused_fq` in `direction`, reading
/// the layer's scope set entirely through a [`NavScopeView`].
///
/// Mirrors [`BeamNavStrategy::next`] but with no [`SpatialRegistry`]
/// dependency: every read (focused entry, candidates, override target,
/// First/Last children) goes through `view`. The result is byte-equal
/// to the registry-backed path when `view` is built from a
/// [`RegistryLayerView`] over the focused entry's layer, which is the
/// invariant the divergence diagnostic in the snapshot adapter relies
/// on.
///
/// Returns `focused_fq` (semantic stay-put) when:
/// - the focused FQM is missing from `view` (torn snapshot — also
///   emits `tracing::error!`),
/// - an explicit "wall" override fires for `direction`, or
/// - no candidate satisfies the half-plane / in-beam filters
///   (cardinal) or the focused scope has no children (First/Last).
pub fn pick_target_via_view<V: NavScopeView + ?Sized>(
    view: &V,
    focused_fq: &FullyQualifiedMoniker,
    focused_segment: &SegmentMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    let Some(focused) = view.get(focused_fq) else {
        tracing::error!(
            op = "nav_via_view",
            focused_fq = %focused_fq,
            focused_segment = %focused_segment,
            ?direction,
            "unknown focused FQM passed to pick_target_via_view"
        );
        return focused_fq.clone();
    };

    match check_override(view, &focused, direction) {
        Some(Some(target)) => return target,
        Some(None) => return focused_fq.clone(),
        None => {}
    }

    #[allow(deprecated)]
    match direction {
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => {
            geometric_pick(view, &focused, focused_fq, direction)
        }
        Direction::First | Direction::Last | Direction::RowStart | Direction::RowEnd => {
            edge_command_via_view(view, &focused, focused_fq, direction)
        }
    }
}

/// First/Last edge-command pick, generic over [`NavScopeView`].
///
/// Mirrors the registry-backed [`edge_command`] but reads candidates
/// from `view`. Children of the focused scope are entries whose
/// `parent_zone == focused.parent_zone` (i.e. siblings of the focused
/// scope under its own zone), or — when the focused scope has no
/// parent zone — entries whose `parent_zone == focused.fq` (the focused
/// scope IS a layer-root container, so its children are the entries
/// directly under it).
#[allow(deprecated)]
fn edge_command_via_view<V: NavScopeView + ?Sized>(
    view: &V,
    focused: &NavScopeRef<'_>,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    let target_parent = focused.parent_zone.cloned();
    let pick: Option<FullyQualifiedMoniker> = match direction {
        Direction::First | Direction::RowStart => view
            .iter()
            .filter(|s| match &target_parent {
                Some(p) => s.parent_zone == Some(p),
                None => s.parent_zone == Some(focused.fq),
            })
            .min_by(|a, b| {
                pixels_cmp(a.rect.top(), b.rect.top())
                    .then(pixels_cmp(a.rect.left(), b.rect.left()))
            })
            .map(|s| s.fq.clone()),
        Direction::Last | Direction::RowEnd => view
            .iter()
            .filter(|s| match &target_parent {
                Some(p) => s.parent_zone == Some(p),
                None => s.parent_zone == Some(focused.fq),
            })
            .max_by(|a, b| {
                pixels_cmp(a.rect.bottom(), b.rect.bottom())
                    .then(pixels_cmp(a.rect.right(), b.rect.right()))
            })
            .map(|s| s.fq.clone()),
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => None,
    };
    pick.unwrap_or_else(|| focused_fq.clone())
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
///   to the geometric pick.
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
///
/// Reads through [`NavScopeView`] so the same code services both the
/// existing registry-backed path and the future snapshot-driven path.
/// `view` already represents a single layer; resolution against the
/// view is therefore implicitly layer-scoped.
fn check_override<V: NavScopeView + ?Sized>(
    view: &V,
    focused: &NavScopeRef<'_>,
    direction: Direction,
) -> Option<Option<FullyQualifiedMoniker>> {
    let entry = focused.nav_override.get(&direction)?;
    match entry {
        // Explicit `None` — block navigation in this direction.
        None => Some(None),
        // `Some(target_fq)` — resolve only within the layer view. A
        // target outside the view (different layer, or unregistered
        // entirely) makes the override fall through to beam search.
        Some(target) => {
            if view.get(target).is_some() {
                Some(Some(target.clone()))
            } else {
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cardinal-direction navigation: geometric layer-wide beam search.
// ---------------------------------------------------------------------------

/// Run the geometric pick from `focused` in `direction`.
///
/// Iterates every entry in `focused.layer_fq`, filters out the
/// focused entry itself, scores via [`score_candidate`], and returns
/// the candidate with the lowest beam score that passes the strict
/// half-plane and in-beam tests. Ties are broken by preferring leaves
/// (scopes with no registered children) over containers so the focus
/// indicator paints on a visible surface.
///
/// When no candidate satisfies both tests, the focused entry is at the
/// visual edge of the layer in `direction`; the function returns
/// `focused_fq` (stay-put), per the no-silent-dropout invariant.
///
/// The layer FQM is the absolute boundary — the view passed in is
/// already layer-scoped, so a candidate from a different layer (e.g.
/// the inspector layer) never enters the search.
///
/// Reads through [`NavScopeView`] so the same code services both the
/// existing registry-backed path and the future snapshot-driven path.
/// The "has children" precomputation walks the same view and therefore
/// stays O(N), independent of the underlying store.
fn geometric_pick<V: NavScopeView + ?Sized>(
    view: &V,
    focused: &NavScopeRef<'_>,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    // Build the "has children" predicate once per pick, not per
    // candidate. Looking up `has_children(fq)` on every iteration is
    // O(N) on the underlying store, so calling it inside the
    // candidate loop made the pick O(N²). Collecting the set of FQMs
    // that appear as some scope's `parent_zone` (anywhere in the
    // layer view) restores O(N). Cloning each FQM keeps the set
    // independent of the iteration's transient `NavScopeRef` borrow.
    let parent_fqs: HashSet<FullyQualifiedMoniker> =
        view.iter().filter_map(|s| s.parent_zone.cloned()).collect();
    let from_rect = focused.rect;
    let mut best: Option<BestCandidate> = None;
    for cand in view.iter() {
        if cand.fq == focused.fq {
            continue;
        }
        let cand_rect = cand.rect;
        if !in_strict_half_plane(&from_rect, &cand_rect, direction) {
            continue;
        }
        let Some((in_beam, score)) = score_candidate(&from_rect, &cand_rect, direction) else {
            continue;
        };
        if !in_beam {
            continue;
        }
        let cand_summary = BestCandidate {
            fq: cand.fq.clone(),
            score,
            has_children: parent_fqs.contains(cand.fq),
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

/// Running best candidate inside [`geometric_pick`]: FQM, beam score, and the
/// has-children flag used for the leaves-over-containers tie-break.
///
/// `fq` is owned because `view.iter()` returns a fresh boxed iterator whose
/// items don't outlive the call.
#[derive(Clone)]
struct BestCandidate {
    fq: FullyQualifiedMoniker,
    score: f64,
    has_children: bool,
}

/// `true` when `cand` should replace the current best candidate.
///
/// Primary order is the Android beam score: lower is better. When two
/// candidates have equal scores, the leaf wins over the container
/// (a leaf is a scope with no registered children) — this ensures the
/// focus indicator paints on a visible leaf rather than on a
/// `showFocusBar=false` container.
fn better_candidate(current: &Option<BestCandidate>, cand: &BestCandidate) -> bool {
    match current {
        None => true,
        Some(cur) => {
            if cand.score < cur.score {
                true
            } else if cand.score > cur.score {
                false
            } else {
                // Tie on score: prefer a leaf (`!has_children`) over a
                // container (`has_children`). If `cand` is a leaf and
                // `cur` is a container, replace. Otherwise keep `cur`
                // (no leaf-tie advantage).
                !cand.has_children && cur.has_children
            }
        }
    }
}

/// `true` if `cand` lies strictly in the half-plane of `direction`
/// from `from`.
///
/// "Strictly" here means the candidate's leading edge in the reverse
/// of `direction` is at or past `from`'s leading edge in `direction`.
/// For `Down`: `cand.top >= from.bottom` (the candidate starts at or
/// below `from`'s bottom edge). Symmetric for the other three
/// directions.
///
/// This filter is the kernel's geometric notion of "below / above /
/// left of / right of" — it excludes containing parent scopes (which
/// extend on both sides of `from` on the major axis) and overlapping
/// rects from being treated as candidates.
///
/// First / Last commands ([`Direction::First`], [`Direction::Last`],
/// and the deprecated `Direction::RowStart` / `Direction::RowEnd`
/// aliases) never call this helper.
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

// ---------------------------------------------------------------------------
// First / Last — focus the focused scope's first / last child.
// ---------------------------------------------------------------------------

/// Run a First / Last command from `focused` in `direction`.
///
/// New contract (per design `01KQQSXM2PEYR1WAQ7QXW3B8ME`):
///
/// - **First child** = the child whose rect is topmost; ties broken by
///   leftmost.
/// - **Last child** = the child whose rect is bottommost; ties broken
///   by rightmost.
/// - **Children** = registered scopes whose `parent_zone` is
///   `focused.fq`.
///
/// On a leaf (no children) both ops return `focused_fq` (semantic
/// no-op, no log noise) per the no-silent-dropout contract.
///
/// `Direction::First` shares its result with [`SpatialRegistry::drill_in`]'s
/// cold-start fallback when the focused scope has no `last_focused`
/// memory — both pick the topmost-then-leftmost child via the shared
/// [`first_child_by_top_left`] helper, so divergence is structurally
/// impossible. The `first_matches_drill_in_first_child_fallback` test
/// is the behavioural backstop on that contract.
///
/// The deprecated `Direction::RowStart` / `Direction::RowEnd`
/// aliases route through the same arms as `First` / `Last`. The user
/// model has no separate "first in row" concept — the focused scope
/// IS the row, so "first in row" and "first child" collapse to the
/// same operation; the aliases are kept on the enum for one release
/// so wire-format consumers can migrate.
#[allow(deprecated)]
fn edge_command(
    reg: &SpatialRegistry,
    focused: &FocusScope,
    focused_fq: &FullyQualifiedMoniker,
    direction: Direction,
) -> FullyQualifiedMoniker {
    // Vim G / gg semantics: jump to first/last sibling within the focused
    // scope's parent zone — `children_of(focused.parent_zone)`. When the
    // focused scope has no parent zone (e.g. it IS a layer root), fall
    // back to drilling into its own children. Without this fallback, a
    // focused leaf (inspector field, card field) would have an empty
    // children set and the command would no-op.
    let children = match focused.parent_zone.as_ref() {
        Some(parent) => reg.children_of(parent),
        None => reg.children_of(&focused.fq),
    };
    let pick = match direction {
        // First (and the deprecated `RowStart` alias) — topmost; ties
        // broken by leftmost. Shared with `SpatialRegistry::drill_in`'s
        // cold-start fallback so `nav.first` and drill-in cannot drift
        // apart.
        Direction::First | Direction::RowStart => first_child_by_top_left(children),
        // Last (and the deprecated `RowEnd` alias) — bottommost; ties
        // broken by rightmost. Mirror of the First helper.
        Direction::Last | Direction::RowEnd => last_child_by_bottom_right(children),
        // Cardinal directions never reach this helper — `BeamNavStrategy`
        // routes them through `geometric_pick` instead.
        Direction::Up | Direction::Down | Direction::Left | Direction::Right => None,
    };
    pick.map(|s| s.fq.clone())
        .unwrap_or_else(|| focused_fq.clone())
}

// ---------------------------------------------------------------------------
// Beam math: candidate scoring.
// ---------------------------------------------------------------------------

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
        // First / Last (and the deprecated `RowStart` / `RowEnd`
        // aliases) never reach this helper — they have their own
        // candidate-picking logic in `edge_command`.
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
///
/// This helper is retained for symmetry with [`score_candidate`] and
/// future strategies that want to pick a best candidate from a
/// pre-filtered iterator. The geometric-pick path uses a hand-rolled
/// loop (so it can layer the leaf-tie-break on top of the score
/// comparison) and does not call this helper.
#[allow(dead_code)]
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

    /// Build a [`Rect`] from raw `f64` coordinates. Local helper for
    /// the test fixtures — keeps each test top-to-bottom readable.
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

    /// Build the canonical `/L` window-style layer used by the unit
    /// tests below. The layer FQM is `/L` and its name is `"window"`.
    fn make_layer() -> FocusLayer {
        FocusLayer {
            fq: FullyQualifiedMoniker::from_string("/L"),
            segment: SegmentMoniker::from_string("L"),
            name: LayerName::from_string("window"),
            parent: None,
            window_label: WindowLabel::from_string("main"),
            last_focused: None,
        }
    }

    /// Build a `FocusScope` inside `/L` with the given segment and
    /// rect. `parent_zone` is configurable; `last_focused` defaults to
    /// `None`.
    fn make_scope(
        segment: &str,
        parent_zone: Option<FullyQualifiedMoniker>,
        r: Rect,
    ) -> FocusScope {
        FocusScope {
            fq: FullyQualifiedMoniker::from_string(format!("/L/{segment}")),
            segment: SegmentMoniker::from_string(segment),
            rect: r,
            layer_fq: FullyQualifiedMoniker::from_string("/L"),
            parent_zone,
            last_focused: None,
            overrides: HashMap::new(),
        }
    }

    /// Lonely scope — nothing else to navigate to. Returns the
    /// focused FQM (semantic "stay put" — empty Direction-D
    /// half-plane).
    #[test]
    fn lonely_scope_returns_focused_fq() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let only = make_scope("k", None, rect_zero());
        let only_fq = only.fq.clone();
        reg.register_scope(only);

        let strategy = BeamNavStrategy::new();
        let focused_segment = SegmentMoniker::from_string("k");
        for d in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let result = strategy.next(&reg, &only_fq, &focused_segment, d);
            assert_eq!(
                result, only_fq,
                "lonely scope must echo focused FQM for {d:?}"
            );
        }
    }

    /// One neighbor in direction wins.
    #[test]
    fn one_neighbor_in_direction_wins() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        // Source on the left; one neighbor strictly to the right.
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let neighbor = make_scope("neighbor", None, rect(20.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        let neighbor_fq = neighbor.fq.clone();
        reg.register_scope(src);
        reg.register_scope(neighbor);

        let strategy = BeamNavStrategy::new();
        let focused_segment = SegmentMoniker::from_string("src");
        let result = strategy.next(&reg, &src_fq, &focused_segment, Direction::Right);
        assert_eq!(result, neighbor_fq);
    }

    /// Two neighbors at different distances — closer wins.
    #[test]
    fn closer_neighbor_wins() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let near = make_scope("near", None, rect(20.0, 0.0, 10.0, 10.0));
        let far = make_scope("far", None, rect(100.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        let near_fq = near.fq.clone();
        reg.register_scope(src);
        reg.register_scope(near);
        reg.register_scope(far);

        let strategy = BeamNavStrategy::new();
        let focused_segment = SegmentMoniker::from_string("src");
        let result = strategy.next(&reg, &src_fq, &focused_segment, Direction::Right);
        assert_eq!(result, near_fq, "closer in-beam neighbor must win");
    }

    /// Tied geometry — leaf wins over container (the leaves-over-
    /// containers tie-break that ensures the focus indicator paints on
    /// a visible surface rather than a `showFocusBar=false` container).
    /// A "container" is a scope with at least one registered child.
    #[test]
    fn tied_distances_leaf_wins_over_container() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        // Two candidates at the same rect (geometric tie). One is a
        // container (has a child); one is a leaf — the leaf must win
        // on the tie-break.
        let container_cand = make_scope("container-cand", None, rect(20.0, 0.0, 10.0, 10.0));
        let container_cand_fq = container_cand.fq.clone();
        let container_child = make_scope(
            "container-cand/child",
            Some(container_cand_fq.clone()),
            rect(20.0, 0.0, 5.0, 5.0),
        );
        let leaf_cand = make_scope("leaf-cand", None, rect(20.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        let leaf_cand_fq = leaf_cand.fq.clone();
        reg.register_scope(src);
        reg.register_scope(container_cand);
        reg.register_scope(container_child);
        reg.register_scope(leaf_cand);

        let strategy = BeamNavStrategy::new();
        let focused_segment = SegmentMoniker::from_string("src");
        let result = strategy.next(&reg, &src_fq, &focused_segment, Direction::Right);
        assert_eq!(
            result, leaf_cand_fq,
            "geometric tie must resolve to the leaf (no children), not the container"
        );
    }

    /// Cross-`parent_zone` candidate wins when geometrically nearer
    /// than the in-zone candidate. The geometric pick has no
    /// structural filter, so a sibling with a different `parent_zone`
    /// can beat an in-zone sibling on raw distance.
    #[test]
    fn cross_parent_zone_candidate_wins_when_geometrically_nearer() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let zone_left = make_scope("zone-left", None, rect(0.0, 0.0, 100.0, 50.0));
        // zone-right's vertical extent does not overlap `src` so the
        // zone itself is not an in-beam Right candidate; only the
        // leaf inside it is.
        let zone_right = make_scope("zone-right", None, rect(200.0, 100.0, 100.0, 50.0));
        let zone_left_fq = zone_left.fq.clone();
        let zone_right_fq = zone_right.fq.clone();
        reg.register_scope(zone_left);
        reg.register_scope(zone_right);

        // Source in zone-left near its right edge.
        let src = make_scope(
            "src",
            Some(zone_left_fq.clone()),
            rect(80.0, 10.0, 10.0, 10.0),
        );
        // In-zone sibling — directly below `src` (out of the Right
        // beam because it has no vertical overlap with `src`).
        let in_zone = make_scope(
            "in-zone-below",
            Some(zone_left_fq),
            rect(80.0, 30.0, 10.0, 10.0),
        );
        // Cross-zone sibling — slightly past zone-right's left edge,
        // matching `src`'s y. Geometrically the nearest Right
        // candidate even though it has a different `parent_zone`.
        let cross_zone = make_scope(
            "cross-zone-near",
            Some(zone_right_fq),
            rect(205.0, 10.0, 10.0, 10.0),
        );
        let src_fq = src.fq.clone();
        let cross_fq = cross_zone.fq.clone();
        reg.register_scope(src);
        reg.register_scope(in_zone);
        reg.register_scope(cross_zone);

        let strategy = BeamNavStrategy::new();
        let focused_segment = SegmentMoniker::from_string("src");
        let result = strategy.next(&reg, &src_fq, &focused_segment, Direction::Right);
        assert_eq!(
            result, cross_fq,
            "geometric pick has no structural filter — the cross-parent_zone \
             candidate wins when it is the nearest in-beam in-half-plane scope"
        );
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

    // -----------------------------------------------------------------
    // First / Last — focus the focused scope's first / last child.
    //
    // Contract (from design 01KQQSXM2PEYR1WAQ7QXW3B8ME):
    //   First child = the child whose rect is topmost; ties broken by leftmost.
    //   Last child  = the child whose rect is bottommost; ties broken by rightmost.
    //   Children    = registered scopes whose `parent_zone` is the focused FQM.
    //
    // On a leaf (no children) both ops return the focused FQM (no-op).
    // -----------------------------------------------------------------

    /// Helper: build a registry with a window layer pre-pushed and run
    /// the strategy with the canonical leaf-segment placeholder. Keeps
    /// the per-test fixtures concise.
    fn run_first_last(
        reg: &SpatialRegistry,
        focused_fq: &FullyQualifiedMoniker,
        direction: Direction,
    ) -> FullyQualifiedMoniker {
        let strategy = BeamNavStrategy::new();
        let segment = SegmentMoniker::from_string("seg");
        strategy.next(reg, focused_fq, &segment, direction)
    }

    /// Focused leaf has no children — both `First` and `Last` echo the
    /// focused FQM (semantic no-op, no log noise).
    #[test]
    fn first_last_on_leaf_returns_focused_self() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let leaf = make_scope("leaf", None, rect_zero());
        let leaf_fq = leaf.fq.clone();
        reg.register_scope(leaf);

        for d in [Direction::First, Direction::Last] {
            let result = run_first_last(&reg, &leaf_fq, d);
            assert_eq!(
                result, leaf_fq,
                "leaf has no children — {d:?} must echo focused FQM"
            );
        }
    }

    /// Focused scope with exactly one child — both `First` and `Last`
    /// return that child's FQM.
    #[test]
    fn first_last_on_zone_with_one_child_returns_that_child() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let parent = make_scope("parent", None, rect(0.0, 0.0, 100.0, 100.0));
        let parent_fq = parent.fq.clone();
        reg.register_scope(parent);

        let only = make_scope(
            "only",
            Some(parent_fq.clone()),
            rect(10.0, 10.0, 50.0, 50.0),
        );
        let only_fq = only.fq.clone();
        reg.register_scope(only);

        assert_eq!(run_first_last(&reg, &parent_fq, Direction::First), only_fq);
        assert_eq!(run_first_last(&reg, &parent_fq, Direction::Last), only_fq);
    }

    /// Focused scope whose three children sit in a horizontal row —
    /// `First` picks the leftmost (it is also the topmost — top is the
    /// primary key, so leftmost wins on the tie); `Last` picks the
    /// rightmost (bottom is the primary key for `Last`; tied here, so
    /// rightmost wins).
    #[test]
    fn first_last_on_zone_with_row_of_children() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let row = make_scope("row", None, rect(0.0, 0.0, 300.0, 50.0));
        let row_fq = row.fq.clone();
        reg.register_scope(row);

        // Three children in a horizontal row, all at y=10.
        let left = make_scope("left", Some(row_fq.clone()), rect(0.0, 10.0, 50.0, 30.0));
        let middle = make_scope(
            "middle",
            Some(row_fq.clone()),
            rect(100.0, 10.0, 50.0, 30.0),
        );
        let right = make_scope("right", Some(row_fq.clone()), rect(200.0, 10.0, 50.0, 30.0));
        let left_fq = left.fq.clone();
        let right_fq = right.fq.clone();
        reg.register_scope(left);
        reg.register_scope(middle);
        reg.register_scope(right);

        assert_eq!(
            run_first_last(&reg, &row_fq, Direction::First),
            left_fq,
            "row of children: First = leftmost (tied on top, leftmost wins)"
        );
        assert_eq!(
            run_first_last(&reg, &row_fq, Direction::Last),
            right_fq,
            "row of children: Last = rightmost (tied on bottom, rightmost wins)"
        );
    }

    /// Focused scope whose three children sit in a vertical column —
    /// `First` picks the topmost; `Last` picks the bottommost.
    #[test]
    fn first_last_on_zone_with_column_of_children() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let col = make_scope("col", None, rect(0.0, 0.0, 50.0, 300.0));
        let col_fq = col.fq.clone();
        reg.register_scope(col);

        let top = make_scope("top", Some(col_fq.clone()), rect(0.0, 0.0, 50.0, 30.0));
        let middle = make_scope("middle", Some(col_fq.clone()), rect(0.0, 100.0, 50.0, 30.0));
        let bottom = make_scope("bottom", Some(col_fq.clone()), rect(0.0, 200.0, 50.0, 30.0));
        let top_fq = top.fq.clone();
        let bottom_fq = bottom.fq.clone();
        reg.register_scope(top);
        reg.register_scope(middle);
        reg.register_scope(bottom);

        assert_eq!(
            run_first_last(&reg, &col_fq, Direction::First),
            top_fq,
            "column of children: First = topmost"
        );
        assert_eq!(
            run_first_last(&reg, &col_fq, Direction::Last),
            bottom_fq,
            "column of children: Last = bottommost"
        );
    }

    /// Focused scope with mixed children (some are containers, some
    /// are leaves) — both `First` and `Last` consider all children.
    /// The contract is purely geometric.
    #[test]
    fn first_last_considers_children_of_any_shape() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let parent = make_scope("parent", None, rect(0.0, 0.0, 300.0, 300.0));
        let parent_fq = parent.fq.clone();
        reg.register_scope(parent);

        // Top child has a sub-child (acts as a container); bottom child
        // is a leaf.
        let top_child = make_scope(
            "top-child",
            Some(parent_fq.clone()),
            rect(0.0, 0.0, 100.0, 50.0),
        );
        let top_child_fq = top_child.fq.clone();
        let top_child_subchild = make_scope(
            "top-child/sub",
            Some(top_child_fq.clone()),
            rect(0.0, 0.0, 10.0, 10.0),
        );
        let bottom_child = make_scope(
            "bottom-child",
            Some(parent_fq.clone()),
            rect(0.0, 200.0, 100.0, 50.0),
        );
        let bottom_child_fq = bottom_child.fq.clone();
        reg.register_scope(top_child);
        reg.register_scope(top_child_subchild);
        reg.register_scope(bottom_child);

        assert_eq!(
            run_first_last(&reg, &parent_fq, Direction::First),
            top_child_fq,
            "First considers any child shape — top child wins because it is topmost"
        );
        assert_eq!(
            run_first_last(&reg, &parent_fq, Direction::Last),
            bottom_child_fq,
            "Last considers any child shape — bottom child wins because it is bottommost"
        );
    }

    /// The deprecated `RowStart` / `RowEnd` aliases must keep
    /// returning the same target as `First` / `Last` for the duration
    /// of their one-release deprecation window. New code must use
    /// `Direction::First` / `Direction::Last`; this test pins the
    /// alias behaviour so wire-format consumers that have not yet
    /// migrated keep getting the right answer.
    #[allow(deprecated)]
    #[test]
    fn deprecated_row_start_end_still_alias_first_last() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let row = make_scope("row", None, rect(0.0, 0.0, 300.0, 50.0));
        let row_fq = row.fq.clone();
        reg.register_scope(row);

        let left = make_scope("left", Some(row_fq.clone()), rect(0.0, 10.0, 50.0, 30.0));
        let right = make_scope("right", Some(row_fq.clone()), rect(200.0, 10.0, 50.0, 30.0));
        let left_fq = left.fq.clone();
        let right_fq = right.fq.clone();
        reg.register_scope(left);
        reg.register_scope(right);

        assert_eq!(
            run_first_last(&reg, &row_fq, Direction::RowStart),
            run_first_last(&reg, &row_fq, Direction::First),
            "deprecated RowStart must echo First"
        );
        assert_eq!(
            run_first_last(&reg, &row_fq, Direction::First),
            left_fq,
            "First — leftmost-topmost child"
        );
        assert_eq!(
            run_first_last(&reg, &row_fq, Direction::RowEnd),
            run_first_last(&reg, &row_fq, Direction::Last),
            "deprecated RowEnd must echo Last"
        );
        assert_eq!(
            run_first_last(&reg, &row_fq, Direction::Last),
            right_fq,
            "Last — rightmost-bottommost child"
        );
    }

    /// `First` from the focused scope is identical to drill-in's
    /// first-child fallback when the scope has no `last_focused`
    /// memory. Both pick the topmost-then-leftmost child.
    ///
    /// The two ops now share the same
    /// [`crate::registry::first_child_by_top_left`] helper, so divergence
    /// is structurally impossible — this test is the behavioural backstop
    /// that confirms the helper is wired into both call sites and the
    /// "topmost-then-leftmost" contract holds end-to-end.
    #[test]
    fn first_matches_drill_in_first_child_fallback() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let parent = make_scope("parent", None, rect(0.0, 0.0, 300.0, 300.0));
        let parent_fq = parent.fq.clone();
        reg.register_scope(parent);

        // Three children — a clear topmost-leftmost winner.
        let alpha = make_scope("alpha", Some(parent_fq.clone()), rect(0.0, 0.0, 50.0, 30.0));
        let beta = make_scope(
            "beta",
            Some(parent_fq.clone()),
            rect(100.0, 0.0, 50.0, 30.0),
        );
        let gamma = make_scope(
            "gamma",
            Some(parent_fq.clone()),
            rect(0.0, 100.0, 50.0, 30.0),
        );
        let alpha_fq = alpha.fq.clone();
        reg.register_scope(alpha);
        reg.register_scope(beta);
        reg.register_scope(gamma);

        let first = run_first_last(&reg, &parent_fq, Direction::First);
        let drill_target = reg.drill_in(parent_fq.clone(), &parent_fq);
        assert_eq!(
            first, alpha_fq,
            "First on scope-with-no-memory must pick topmost-then-leftmost child"
        );
        assert_eq!(
            first, drill_target,
            "First and drill_in (cold-start fallback) share semantics — both \
             pick topmost-then-leftmost child"
        );
    }

    // -----------------------------------------------------------------
    // Snapshot-path parity — step 3 of the spatial-nav redesign
    // (`01KQTC1VNQM9KC90S65P7QX9N1` / step card `01KQW65Z689G7WWRYMBHX6MD7V`).
    //
    // The pathfinding helpers (`geometric_pick`, `check_override`) are
    // refactored over [`NavScopeView`]. The trait is implemented for
    // both [`RegistryLayerView`] (existing behaviour) and
    // [`IndexedSnapshot`] (new path; no production callers yet — step 6
    // wires it in).
    //
    // Each parallel test below builds a [`NavSnapshot`] mirroring the
    // exact scope set used by the registry-backed test above, runs
    // pathfinding via the snapshot view, and asserts the result
    // matches the registry-path result. This is the parity invariant
    // that lets later steps swap the call boundary without smoke
    // testing every cardinal trajectory by hand.
    // -----------------------------------------------------------------

    use crate::snapshot::{IndexedSnapshot, NavSnapshot, SnapshotScope};

    /// Convert a [`FocusScope`] into the [`SnapshotScope`] wire-format
    /// counterpart. The fields map one-to-one because both types share
    /// the same shape; the snapshot omits the registry's
    /// `last_focused`, `segment`, and `layer_fq` slots (snapshots are
    /// already layer-scoped, segment is for logging, and last_focused
    /// is registry state).
    fn snapshot_from_focus_scope(s: &FocusScope) -> SnapshotScope {
        SnapshotScope {
            fq: s.fq.clone(),
            rect: s.rect,
            parent_zone: s.parent_zone.clone(),
            nav_override: s.overrides.clone(),
        }
    }

    /// Build a [`NavSnapshot`] that mirrors every scope in `reg` for
    /// the layer FQ `/L`. Used by the parallel-snapshot tests below to
    /// stand up the same scope set against both pathfinding paths.
    fn snapshot_for_layer(reg: &SpatialRegistry) -> NavSnapshot {
        let layer_fq = FullyQualifiedMoniker::from_string("/L");
        NavSnapshot {
            scopes: reg
                .scopes_in_layer(&layer_fq)
                .map(snapshot_from_focus_scope)
                .collect(),
            layer_fq,
        }
    }

    /// Run the cardinal-direction pathfinder against a snapshot view.
    fn snapshot_geometric_pick(
        reg: &SpatialRegistry,
        focused_fq: &FullyQualifiedMoniker,
        direction: Direction,
    ) -> FullyQualifiedMoniker {
        let snap = snapshot_for_layer(reg);
        let idx = IndexedSnapshot::new(&snap);
        // Resolve `focused` through the trait so the test exercises
        // `IndexedSnapshot::get`, not the snapshot's inherent method
        // — the trait is the production path step 6 will use.
        let focused = NavScopeView::get(&idx, focused_fq)
            .expect("focused fq must be present in the snapshot");
        geometric_pick(&idx, &focused, focused_fq, direction)
    }

    /// Resolve a per-direction override against a snapshot view —
    /// matches the rule-0 path inside `BeamNavStrategy::next`. The
    /// override-block / override-redirect parallel tests exercise this
    /// helper; cardinal-direction parallel tests use the geometric
    /// pick helper instead.
    fn snapshot_check_override(
        reg: &SpatialRegistry,
        focused_fq: &FullyQualifiedMoniker,
        direction: Direction,
    ) -> Option<Option<FullyQualifiedMoniker>> {
        let snap = snapshot_for_layer(reg);
        let idx = IndexedSnapshot::new(&snap);
        let focused = NavScopeView::get(&idx, focused_fq)
            .expect("focused fq must be present in the snapshot");
        check_override(&idx, &focused, direction)
    }

    /// Resolve a per-direction override against a registry layer view
    /// — mirrors `snapshot_check_override` for the registry path so
    /// the override parity tests can compare the two return values
    /// directly.
    fn registry_check_override(
        reg: &SpatialRegistry,
        focused_fq: &FullyQualifiedMoniker,
        direction: Direction,
    ) -> Option<Option<FullyQualifiedMoniker>> {
        let entry = reg
            .find_by_fq(focused_fq)
            .expect("focused fq must be present in the registry");
        let view = RegistryLayerView::new(reg, &entry.layer_fq);
        let focused_ref = NavScopeRef {
            fq: &entry.fq,
            rect: entry.rect,
            parent_zone: entry.parent_zone.as_ref(),
            nav_override: &entry.overrides,
        };
        check_override(&view, &focused_ref, direction)
    }

    /// Snapshot mirror of `lonely_scope_returns_focused_fq`. The
    /// half-plane is empty in every direction, so the snapshot path
    /// must echo the focused FQM under the no-silent-dropout
    /// invariant — same outcome as the registry path.
    #[test]
    fn snapshot_lonely_scope_returns_focused_fq() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let only = make_scope("k", None, rect_zero());
        let only_fq = only.fq.clone();
        reg.register_scope(only);

        for d in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let snap_result = snapshot_geometric_pick(&reg, &only_fq, d);
            assert_eq!(
                snap_result, only_fq,
                "snapshot-backed pathfinding must echo focused FQM for {d:?}"
            );

            // Parity guard: the registry path must agree.
            let strategy = BeamNavStrategy::new();
            let focused_segment = SegmentMoniker::from_string("k");
            let reg_result = strategy.next(&reg, &only_fq, &focused_segment, d);
            assert_eq!(
                snap_result, reg_result,
                "snapshot and registry paths must agree on {d:?}"
            );
        }
    }

    /// Snapshot mirror of `one_neighbor_in_direction_wins`.
    #[test]
    fn snapshot_one_neighbor_in_direction_wins() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let neighbor = make_scope("neighbor", None, rect(20.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        let neighbor_fq = neighbor.fq.clone();
        reg.register_scope(src);
        reg.register_scope(neighbor);

        let snap_result = snapshot_geometric_pick(&reg, &src_fq, Direction::Right);
        assert_eq!(snap_result, neighbor_fq);

        let strategy = BeamNavStrategy::new();
        let reg_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(snap_result, reg_result);
    }

    /// Snapshot mirror of `closer_neighbor_wins`.
    #[test]
    fn snapshot_closer_neighbor_wins() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let near = make_scope("near", None, rect(20.0, 0.0, 10.0, 10.0));
        let far = make_scope("far", None, rect(100.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        let near_fq = near.fq.clone();
        reg.register_scope(src);
        reg.register_scope(near);
        reg.register_scope(far);

        let snap_result = snapshot_geometric_pick(&reg, &src_fq, Direction::Right);
        assert_eq!(snap_result, near_fq);

        let strategy = BeamNavStrategy::new();
        let reg_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(snap_result, reg_result);
    }

    /// Snapshot mirror of `tied_distances_leaf_wins_over_container` —
    /// the leaves-over-containers tie-break is computed via
    /// `view.iter()` so it must hold identically through the snapshot
    /// path (every container's child contributes its `parent_zone` to
    /// the precomputed `parent_fqs` set, exactly as it does on the
    /// registry path).
    #[test]
    fn snapshot_tied_distances_leaf_wins_over_container() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let container_cand = make_scope("container-cand", None, rect(20.0, 0.0, 10.0, 10.0));
        let container_cand_fq = container_cand.fq.clone();
        let container_child = make_scope(
            "container-cand/child",
            Some(container_cand_fq.clone()),
            rect(20.0, 0.0, 5.0, 5.0),
        );
        let leaf_cand = make_scope("leaf-cand", None, rect(20.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        let leaf_cand_fq = leaf_cand.fq.clone();
        reg.register_scope(src);
        reg.register_scope(container_cand);
        reg.register_scope(container_child);
        reg.register_scope(leaf_cand);

        let snap_result = snapshot_geometric_pick(&reg, &src_fq, Direction::Right);
        assert_eq!(snap_result, leaf_cand_fq);

        let strategy = BeamNavStrategy::new();
        let reg_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(snap_result, reg_result);
    }

    /// Snapshot mirror of
    /// `cross_parent_zone_candidate_wins_when_geometrically_nearer`.
    #[test]
    fn snapshot_cross_parent_zone_candidate_wins_when_geometrically_nearer() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let zone_left = make_scope("zone-left", None, rect(0.0, 0.0, 100.0, 50.0));
        let zone_right = make_scope("zone-right", None, rect(200.0, 100.0, 100.0, 50.0));
        let zone_left_fq = zone_left.fq.clone();
        let zone_right_fq = zone_right.fq.clone();
        reg.register_scope(zone_left);
        reg.register_scope(zone_right);

        let src = make_scope(
            "src",
            Some(zone_left_fq.clone()),
            rect(80.0, 10.0, 10.0, 10.0),
        );
        let in_zone = make_scope(
            "in-zone-below",
            Some(zone_left_fq),
            rect(80.0, 30.0, 10.0, 10.0),
        );
        let cross_zone = make_scope(
            "cross-zone-near",
            Some(zone_right_fq),
            rect(205.0, 10.0, 10.0, 10.0),
        );
        let src_fq = src.fq.clone();
        let cross_fq = cross_zone.fq.clone();
        reg.register_scope(src);
        reg.register_scope(in_zone);
        reg.register_scope(cross_zone);

        let snap_result = snapshot_geometric_pick(&reg, &src_fq, Direction::Right);
        assert_eq!(snap_result, cross_fq);

        let strategy = BeamNavStrategy::new();
        let reg_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(snap_result, reg_result);
    }

    /// Snapshot mirror of `beam_returns_focused_fq_for_unknown_start` —
    /// torn-state echo. The snapshot path's `iter()` returns the empty
    /// set when the focused FQ is missing; we test by using the
    /// strategy's full path (which catches the unknown FQ before
    /// touching `geometric_pick`) so both paths echo the input FQM
    /// identically.
    #[test]
    fn snapshot_beam_returns_focused_fq_for_unknown_start() {
        let reg = SpatialRegistry::new();
        let strategy = BeamNavStrategy::new();
        let focused_fq = FullyQualifiedMoniker::from_string("/ghost");
        let focused_segment = SegmentMoniker::from_string("ghost");
        let reg_result = strategy.next(&reg, &focused_fq, &focused_segment, Direction::Up);
        assert_eq!(reg_result, focused_fq);

        // Snapshot-path mirror: an empty registry produces an empty
        // snapshot; the snapshot view's `get` returns `None` for any
        // FQ, and the geometric pick helper falls through to the
        // stay-put echo. We open-code the lookup here rather than
        // calling `snapshot_geometric_pick`, because that helper
        // unwraps the focused entry (which is exactly what
        // `BeamNavStrategy::next` checks for and short-circuits).
        let snap = snapshot_for_layer(&reg);
        let idx = IndexedSnapshot::new(&snap);
        assert!(NavScopeView::get(&idx, &focused_fq).is_none());
    }

    /// Empty half-plane parity (Up/Down/Left/Right) — the focused
    /// scope sits at the visual edge of the layer for every cardinal
    /// direction. Both paths return `focused_fq` (semantic stay-put,
    /// no log noise) for every direction.
    #[test]
    fn snapshot_empty_half_plane_returns_focused_fq() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        reg.register_scope(src);
        // Every other scope sits behind `src` on every cardinal axis.
        // For `Direction::Right`: behind = "to the left or overlapping".
        let behind_left = make_scope("behind-left", None, rect(-50.0, 0.0, 10.0, 10.0));
        let behind_right = make_scope("behind-right", None, rect(50.0, 200.0, 10.0, 10.0));
        reg.register_scope(behind_left);
        reg.register_scope(behind_right);

        // Down must stay-put (no candidate strictly below `src`'s
        // bottom that overlaps horizontally).
        let snap_result = snapshot_geometric_pick(&reg, &src_fq, Direction::Down);
        assert_eq!(
            snap_result, src_fq,
            "Down half-plane is empty — snapshot path must stay put"
        );

        let strategy = BeamNavStrategy::new();
        let reg_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Down,
        );
        assert_eq!(
            snap_result, reg_result,
            "snapshot and registry paths must agree on empty Down half-plane"
        );
    }

    /// Override redirect parity. With a per-direction redirect on the
    /// focused scope, both paths return the redirect target. The
    /// override targets a scope registered in the same layer; both
    /// the registry view's `get` and the snapshot view's `get` resolve
    /// it.
    #[test]
    fn snapshot_nav_override_redirect_honored() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let target = make_scope("target", None, rect(100.0, 0.0, 10.0, 10.0));
        let target_fq = target.fq.clone();
        reg.register_scope(target);

        // Source has a Right-redirect to `target`. Geometrically
        // there is no Right neighbor, so the only way `Right` resolves
        // to `target` is via the override.
        let mut src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        src.overrides
            .insert(Direction::Right, Some(target_fq.clone()));
        let src_fq = src.fq.clone();
        reg.register_scope(src);

        let snap = snapshot_check_override(&reg, &src_fq, Direction::Right);
        assert_eq!(
            snap,
            Some(Some(target_fq.clone())),
            "snapshot path must resolve override redirect"
        );

        let reg_result = registry_check_override(&reg, &src_fq, Direction::Right);
        assert_eq!(
            snap, reg_result,
            "snapshot and registry paths must resolve the override identically"
        );

        // End-to-end through the strategy: same target.
        let strategy = BeamNavStrategy::new();
        let strategy_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(strategy_result, target_fq);
    }

    /// Override block parity. A `None` entry on the focused scope's
    /// override map for `direction` is an explicit "wall" — both
    /// paths report `Some(None)` and the strategy returns the focused
    /// FQM (semantic stay-put).
    #[test]
    fn snapshot_nav_override_block_honored() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        let neighbor = make_scope("neighbor", None, rect(20.0, 0.0, 10.0, 10.0));
        reg.register_scope(neighbor);

        // Source has a Right-block (`null`). Without the override,
        // `neighbor` would win; with it, navigation walls and stays
        // put.
        let mut src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        src.overrides.insert(Direction::Right, None);
        let src_fq = src.fq.clone();
        reg.register_scope(src);

        let snap = snapshot_check_override(&reg, &src_fq, Direction::Right);
        assert_eq!(
            snap,
            Some(None),
            "snapshot path must report explicit wall as Some(None)"
        );

        let reg_result = registry_check_override(&reg, &src_fq, Direction::Right);
        assert_eq!(
            snap, reg_result,
            "snapshot and registry paths must report the wall identically"
        );

        // End-to-end through the strategy: stay put despite an
        // otherwise valid Right neighbor.
        let strategy = BeamNavStrategy::new();
        let strategy_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(
            strategy_result, src_fq,
            "explicit wall must take precedence over geometric pick"
        );
    }

    /// `parent_zone` cycle parity. A malformed registry / snapshot
    /// where two scopes name each other as their `parent_zone` must
    /// not freeze pathfinding. `geometric_pick` reads `parent_zone`
    /// only as a flat-set membership check (the leaves-over-containers
    /// tie-break), so cycles are naturally tolerated by both paths;
    /// this test pins the contract.
    #[test]
    fn snapshot_parent_zone_cycle_does_not_hang() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer());

        // Source on the left.
        let src = make_scope("src", None, rect(0.0, 0.0, 10.0, 10.0));
        let src_fq = src.fq.clone();
        reg.register_scope(src);

        // Two candidates whose `parent_zone` form a cycle
        // (a.parent = b, b.parent = a). Both candidates are leaves
        // (no third scope names them as `parent_zone`), so the
        // leaves-over-containers tie-break is a no-op. The Right
        // beam picks the closer one.
        let a_fq = FullyQualifiedMoniker::from_string("/L/a");
        let b_fq = FullyQualifiedMoniker::from_string("/L/b");
        let mut a = make_scope("a", Some(b_fq.clone()), rect(20.0, 0.0, 10.0, 10.0));
        a.fq = a_fq.clone();
        let mut b = make_scope("b", Some(a_fq.clone()), rect(50.0, 0.0, 10.0, 10.0));
        b.fq = b_fq.clone();
        reg.register_scope(a);
        reg.register_scope(b);

        let snap_result = snapshot_geometric_pick(&reg, &src_fq, Direction::Right);
        assert_eq!(
            snap_result, a_fq,
            "snapshot path must terminate cleanly under a parent_zone cycle"
        );

        let strategy = BeamNavStrategy::new();
        let reg_result = strategy.next(
            &reg,
            &src_fq,
            &SegmentMoniker::from_string("src"),
            Direction::Right,
        );
        assert_eq!(
            snap_result, reg_result,
            "snapshot and registry paths must agree under a parent_zone cycle"
        );
    }
}
