//! `SpatialRegistry` — the headless store for spatial scopes and layers.
//!
//! The registry holds two flat maps:
//!
//! - `scopes: HashMap<FullyQualifiedMoniker, FocusScope>` — every
//!   registered scope, keyed by its canonical FQM. Whether a scope is
//!   a leaf or a navigable container is determined at runtime by
//!   whether anything else is registered under it
//!   (see [`SpatialRegistry::children_of`]).
//! - `layers: HashMap<FullyQualifiedMoniker, FocusLayer>` — every
//!   registered layer node, keyed by its FQM.
//!
//! Tree / forest structure is **derived**, not stored: scope hierarchy
//! comes from each scope's `parent_zone`, layer hierarchy from each
//! layer's `parent`. This keeps mutation simple (one map insert per mount)
//! and makes the structural queries (`children_of`, `ancestor_zones`,
//! `children_of_layer`, `ancestors_of_layer`) the source of truth for
//! "what's inside what".
//!
//! ## Path-monikers identifier model
//!
//! The kernel uses **one** identifier shape per primitive: the
//! [`FullyQualifiedMoniker`]. The path through the focus hierarchy IS
//! the spatial key. A consumer constructing a `<FocusScope>` declares
//! a relative [`SegmentMoniker`]; the React adapter composes the FQM
//! through `FullyQualifiedMonikerContext` and ships it through IPC.
//! There is no UUID-based `SpatialKey` and no flat `Moniker`.
//!
//! Path-as-key eliminates the structural bug where a board card field
//! and an inspector panel field share a `SegmentMoniker` (e.g.
//! `field:T1.title`) and end up registered under the same flat key —
//! the FQMs `/window/board/.../card:T1/field:T1.title` and
//! `/window/inspector/field:T1.title` are distinct by construction.
//!
//! ## Two peers, not three
//!
//! The kernel exposes two peer types: [`super::layer::FocusLayer`] and
//! [`FocusScope`]. There is no separate "zone" type — a scope that has
//! children acts as a navigable container, a scope with no children
//! acts as a leaf. Consumers iterate the registry via
//! [`scopes_iter`](SpatialRegistry::scopes_iter) and
//! [`scopes_in_layer`](SpatialRegistry::scopes_in_layer).
//!
//! ## Threading model
//!
//! `SpatialRegistry` is plain data — not `Sync` on its own. Callers wrap
//! it in a `Mutex`/`RwLock` when they need shared mutable access. The
//! kanban-app `AppState` already serializes spatial commands behind a
//! `tokio::sync::Mutex`, so no additional locking lives here.
//!
//! ## Relationship to `SpatialState`
//!
//! [`super::state::SpatialState`] tracks per-window focus (the
//! `focus_by_window` map) and emits [`super::state::FocusChangedEvent`]s.
//! `SpatialRegistry` tracks the geometry / layer / scope structure that
//! the navigator reads to compute the next focus target. The two are
//! intentionally separate: focus state mutates frequently (every
//! keystroke), structural data mutates only on mount / unmount / resize.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::layer::FocusLayer;
use super::scope::FocusScope;
use super::snapshot::IndexedSnapshot;
use super::types::{
    pixels_cmp, Direction, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker, WindowLabel,
};

/// Emit `tracing::error!` only when re-registering at an already-occupied
/// FQM with a *structurally different* entry — i.e. a mismatch in any
/// of the structural fields (`segment`, `layer_fq`, `parent_zone`,
/// `overrides`).
///
/// Same-shape re-registration is silent. The legitimate paths that hit
/// this case repeatedly:
///
/// * **Virtualizer placeholder → real-mount swap.** The board column's
///   `usePlaceholderRegistration` hook in `column-view.tsx` registers
///   off-screen task FQMs as placeholder scopes via
///   `spatial_register_batch`. When a task scrolls into view (or mounts
///   on the first render after measurement) its `<EntityCard>`
///   `<FocusScope>` registers at the same FQM with an identical
///   structural shape — only the rect (placeholder estimate vs. real
///   `getBoundingClientRect()`) differs. The placeholder hook
///   unregisters its entry on the next render commit; in between, the
///   kernel sees a same-shape re-register that is part of the
///   intentional swap.
///
/// * **React StrictMode dev double-mount.** The `<FocusScope>` register
///   effect runs, cleans up, and re-runs in a single mount under
///   StrictMode. Both register IPCs ship with identical structural
///   data; the cleanup's unregister IPC sits in between, so this is
///   *normally* not even a duplicate at the kernel. But if any IPC
///   reordering or batching causes the second register to land before
///   the cleanup unregister, the kernel still sees a same-shape
///   re-register.
///
/// * **ResizeObserver-driven rect refresh.** The same `<FocusScope>`
///   re-fires its register effect when its dependency tuple shifts
///   (e.g. `parent_zone` or `layer_fq` recomputed identically by
///   context, but the React reconciler still re-runs the effect).
///
/// A genuine programmer mistake — two primitives whose composed paths
/// collide with conflicting metadata (different segments, different
/// enclosing zones / layers, different override sets) — still trips
/// the error log so it stays visible.
fn warn_on_structural_mismatch(
    existing: &FocusScope,
    new_segment: &SegmentMoniker,
    new_layer_fq: &FullyQualifiedMoniker,
    new_parent_zone: Option<&FullyQualifiedMoniker>,
    new_overrides: &HashMap<Direction, Option<FullyQualifiedMoniker>>,
) {
    let segment_differs = existing.segment != *new_segment;
    let layer_differs = existing.layer_fq != *new_layer_fq;
    let parent_zone_differs = existing.parent_zone.as_ref() != new_parent_zone;
    let overrides_differ = existing.overrides != *new_overrides;

    if segment_differs || layer_differs || parent_zone_differs || overrides_differ {
        tracing::error!(
            op = "register_scope",
            fq = %existing.fq,
            segment_differs,
            layer_differs,
            parent_zone_differs,
            overrides_differ,
            "duplicate FQM registration with structural mismatch — \
             two primitives composed the same path but disagree on \
             segment / layer / parent_zone / overrides. \
             Replacing prior entry; nav may be inconsistent until \
             the offending primitive is fixed."
        );
    }
}

/// Round a [`Pixels`] coordinate to its nearest integer pixel as
/// `i64`.
///
/// Subpixel rendering produces tiny variations between successive
/// `getBoundingClientRect()` reads on the same DOM node (anti-aliased
/// borders, ResizeObserver fractional dpr math) that aren't user-
/// relevant. The same-(x, y) overlap check rounds before comparing so
/// it catches structural overlaps (parent scope wrapping a single child
/// with no offset) and ignores noise. `i64` is used rather than `i32`
/// because viewport coordinates can exceed `i32::MAX` after extreme
/// CSS transforms; the rounded value is only used for equality
/// comparison so the larger range is free.
fn rounded_pixel(p: Pixels) -> i64 {
    p.value().round() as i64
}

/// Returns `true` when the new entry's rounded `(left, top)` equals
/// the existing entry's rounded `(left, top)`.
///
/// Both rects are reduced to their `(left, top)` corner and rounded
/// via [`rounded_pixel`] before comparison — see that helper for the
/// rationale on integer rounding. Width / height are intentionally not
/// part of the comparison: the structural overlap signal we hunt for
/// is "two entries anchored at the same point", which is what catches
/// needless-nesting wrappers regardless of whether the inner entry
/// trims a few pixels of padding.
fn same_rounded_origin(new: &Rect, existing: &Rect) -> bool {
    rounded_pixel(new.left()) == rounded_pixel(existing.left())
        && rounded_pixel(new.top()) == rounded_pixel(existing.top())
}

/// Borrowed payload for [`warn_overlap`].
///
/// Bundles the entry, partner, and shared origin coordinates so the
/// warn helper takes a single argument; clippy's
/// `too_many_arguments` lint flagged the prior multi-arg form. All
/// fields are borrowed because the caller already holds them as
/// references off the registry's scopes map.
struct OverlapWarn<'a> {
    /// Calling op tag (`"register_scope"`, `"update_rect"`).
    op: &'static str,
    /// Owning layer's FQM — same for both entry and partner; same-
    /// layer is part of the overlap definition.
    layer_fq: &'a FullyQualifiedMoniker,
    /// FQM of the entry whose registration / rect update introduced
    /// the overlap.
    new_fq: &'a FullyQualifiedMoniker,
    /// Relative segment of the new entry — included for human-readable
    /// log inspection without re-fetching from the registry.
    new_segment: &'a SegmentMoniker,
    /// FQM of the pre-existing entry the new one landed on top of.
    overlap_fq: &'a FullyQualifiedMoniker,
    /// Relative segment of the partner.
    overlap_segment: &'a SegmentMoniker,
    /// Shared rounded x-coordinate in viewport space.
    rounded_x: i64,
    /// Shared rounded y-coordinate in viewport space.
    rounded_y: i64,
}

/// Emit one `WARN`-level tracing event for an overlap.
///
/// The message carries the literal `needless-nesting` substring so a
/// grep pipeline filters it out of the broader log stream without
/// risking false positives on adjacent registry warnings. See
/// [`OverlapWarn`] for the field semantics.
fn warn_overlap(payload: OverlapWarn<'_>) {
    tracing::warn!(
        target: "swissarmyhammer_focus::registry",
        op = payload.op,
        layer = %payload.layer_fq,
        new_fq = %payload.new_fq,
        new_segment = %payload.new_segment,
        overlap_fq = %payload.overlap_fq,
        overlap_segment = %payload.overlap_segment,
        x = payload.rounded_x,
        y = payload.rounded_y,
        "two entries share (x, y); likely needless-nesting — review React tree for redundant wrappers"
    );
}

// ---------------------------------------------------------------------------
// Coordinate-system invariant checks (debug-only, observability-only).
// ---------------------------------------------------------------------------

/// Maximum absolute coordinate that the kernel still considers
/// "plausibly viewport-relative". 1e6 px is two orders of magnitude
/// beyond any real desktop layout (8K display = 7680 px wide), so any
/// coordinate beyond this bound is almost certainly document-relative
/// (a unit error or a wrong coordinate system) and beam search would
/// silently mis-rank it against legitimate viewport-relative siblings.
///
/// Mirrors the same bound on the TS-side validator
/// (`kanban-app/ui/src/lib/rect-validation.ts::LARGE_COORD_BOUND`).
const LARGE_COORD_BOUND: f64 = 1_000_000.0;

/// Multiplier used by [`SpatialRegistry::validate_coordinate_consistency`]
/// to decide that a single rect is suspiciously far from the layer's
/// median. When a rect's distance from the centroid is greater than
/// this multiple of the median distance, the registry emits a
/// `WARN`-level event flagging a likely coordinate-system mismatch
/// (e.g. half the layer registered with viewport-relative rects, the
/// other half with document-relative rects).
///
/// Chosen at 10× because production layouts have ~uniform rect
/// distribution: the bottom card in a column is at most 5× further
/// from the centroid than the top card. 10× is loose enough to ignore
/// legitimate spread while still catching the order-of-magnitude
/// mismatch a coordinate-system bug introduces.
const COORDINATE_CONSISTENCY_MULTIPLIER: f64 = 10.0;

/// Validate that `rect` is well-formed before insertion / update.
///
/// In `cfg(debug_assertions)` builds, emits one `tracing::error!` per
/// detected violation:
///
/// - **Non-finite coordinates** — `NaN`, `+Infinity`, `-Infinity`. These
///   break beam search distance math (every comparison short-circuits
///   to `Equal` via `pixels_cmp`).
/// - **Non-positive dimensions** — `width <= 0` or `height <= 0`. The
///   handling depends on the op:
///
///   - On `"register_scope"` (initial registration), a zero in either
///     dimension is treated as a *pre-layout transient*:
///     `getBoundingClientRect()` legitimately returns rects with zero
///     dims for `display: none`, just-mounted-but-not-yet-laid-out, and
///     detached nodes (and in test environments, jsdom-style flex/grid
///     containers commonly produce `width × 0` zones until the first
///     layout pass). That's not a coordinate-system bug — it's "the
///     registration `useEffect` ran before the first layout pass."
///     Downgrades to a single `tracing::warn!` and continues.
///   - On `"update_rect"`, a zero in either dimension is a real error.
///     Update fires from `ResizeObserver` and the ancestor-scroll
///     listener, both of which run only after layout — a zero dim at
///     this point means a persistent broken rect, not a transient one.
///
///   Negative dims always stay in the error path: `getBoundingClientRect()`
///   never returns a negative width or height, so a negative dim is a
///   different bug class than "not laid out yet".
/// - **Implausible scale** — coordinates outside `[-1e6, 1e6]`. A rect
///   at `(50000, 50000)` is almost certainly document-relative
///   (`offsetTop` / `offsetLeft` instead of `getBoundingClientRect()`)
///   and would silently mis-rank against viewport-relative siblings.
///
/// In release builds, this function is a no-op — the validator is
/// observability, not enforcement, and the kernel must remain
/// best-effort for unknown / torn input. The TS-side validator
/// (`rect-validation.ts`) catches the same violations earlier in dev
/// mode, so the kernel-side check is the safety net for IPC adapters
/// or test fixtures that bypass the React tree.
///
/// `op` is the caller op tag (`"register_scope"`, `"update_rect"`) —
/// used as a structured tracing field so log readers can correlate the
/// event back to the IPC adapter, AND as the dispatch key for the
/// registration vs update zero-dim handling described above.
fn validate_rect_invariants(op: &'static str, fq: &FullyQualifiedMoniker, rect: &Rect) {
    #[cfg(debug_assertions)]
    {
        let components = [
            ("x", rect.x.value()),
            ("y", rect.y.value()),
            ("width", rect.width.value()),
            ("height", rect.height.value()),
        ];

        for (name, value) in components {
            if !value.is_finite() {
                tracing::error!(
                    target: "swissarmyhammer_focus::registry",
                    op,
                    fq = %fq,
                    component = name,
                    value = format!("{value}"),
                    "rect component is not finite; expected a real-valued pixel from getBoundingClientRect()"
                );
            }
        }

        let width = rect.width.value();
        let height = rect.height.value();
        let is_registration = op == "register_scope";
        // Pre-layout transient: at least one dim is exactly zero, both
        // are finite, neither is negative, AND we're on the registration
        // path. Surface as a single warning. On `update_rect`, fall
        // through to the per-component error path because layout has
        // already run by the time `ResizeObserver` / scroll fires.
        let any_zero_dim = width.is_finite()
            && height.is_finite()
            && width >= 0.0
            && height >= 0.0
            && (width == 0.0 || height == 0.0);
        if is_registration && any_zero_dim {
            // Legitimate pre-layout zero dims are common in real layouts
        } else {
            if width.is_finite() && width <= 0.0 {
                tracing::error!(
                    target: "swissarmyhammer_focus::registry",
                    op,
                    fq = %fq,
                    width = width,
                    "rect width must be > 0; zero-size rect breaks beam search distance math"
                );
            }
            if height.is_finite() && height <= 0.0 {
                tracing::error!(
                    target: "swissarmyhammer_focus::registry",
                    op,
                    fq = %fq,
                    height = height,
                    "rect height must be > 0; zero-size rect breaks beam search distance math"
                );
            }
        }

        for (name, value) in components {
            if value.is_finite() && value.abs() > LARGE_COORD_BOUND {
                tracing::error!(
                    target: "swissarmyhammer_focus::registry",
                    op,
                    fq = %fq,
                    component = name,
                    value,
                    bound = LARGE_COORD_BOUND,
                    "rect component outside plausible viewport range; likely document-relative coordinates instead of viewport-relative"
                );
            }
        }
    }

    // In release builds, suppress the unused-arguments lint without
    // touching the call sites.
    #[cfg(not(debug_assertions))]
    {
        let _ = (op, fq, rect);
    }
}

/// Headless store for spatial scopes and layers.
///
/// See module docs for the threading model. `Default` produces an empty
/// registry; `new` is provided for symmetry with `SpatialState::new`.
#[derive(Debug, Default, Clone)]
pub struct SpatialRegistry {
    /// All registered scopes keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Whether a scope is a leaf or a
    /// container is decided at runtime by whether anything is
    /// registered under it.
    scopes: HashMap<FullyQualifiedMoniker, FocusScope>,
    /// All registered layers keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Layer hierarchy is derived from each
    /// layer's `parent` field, not stored here.
    layers: HashMap<FullyQualifiedMoniker, FocusLayer>,
    /// Per-entry suppression state for the overlap warning.
    ///
    /// Maps an entry's FQM to the FQM of the partner it was last
    /// reported as overlapping. The registry consults this map
    /// before emitting a fresh overlap `WARN` on `update_rect`: when
    /// the moved entry is already overlapping the *same* partner as
    /// last time, the warning is suppressed. Per-frame scroll tracking
    /// (`01KQ9XBAG5P9W3JREQYNGAYM8Y`) re-fires `update_rect` every
    /// animation frame; without this gate every frame would re-emit
    /// the warning for an unchanged overlap.
    ///
    /// The suppression entry is cleared when the overlap clears (the
    /// entry moves off the partner) or when the partner identity
    /// changes. [`SpatialRegistry::unregister_scope`] also drops the
    /// entry's suppression slot so a fresh re-register at the same
    /// overlapping position emits a fresh warning rather than being
    /// silently swallowed by stale state.
    overlap_warn_partner: HashMap<FullyQualifiedMoniker, FullyQualifiedMoniker>,
    /// Set of layer FQMs whose coordinate consistency has already been
    /// validated by [`SpatialRegistry::validate_coordinate_consistency`].
    /// The check is O(n) over the layer's scopes and is meant to run
    /// **once per layer** — typically on the first nav into the layer
    /// — rather than on every registration. Tracking the validated
    /// set here keeps the call site (the navigator) cheap on the
    /// steady-state hot path.
    ///
    /// Re-validation is triggered by clearing this set; in practice
    /// the registry resets the entry when the layer is removed via
    /// [`SpatialRegistry::remove_layer`], when a scope is registered
    /// or updated in the layer (`register_scope`, `update_rect`), or
    /// when a scope is unregistered from the layer (`unregister_scope`).
    /// A re-mounted layer is therefore re-validated on its next first
    /// nav.
    ///
    /// `push_layer` is **intentionally not** an invalidator: re-pushing
    /// a layer (StrictMode double-mount, palette open/close cycles, IPC
    /// re-batch) does not move any scope rects, so the cached validation
    /// result remains valid. Adding `push_layer` to the invalidator set
    /// would re-walk the layer on every benign re-push without surfacing
    /// any new mismatch.
    validated_layers: HashSet<FullyQualifiedMoniker>,
    /// Most-recent focused descendant under each ancestor scope, keyed
    /// by the ancestor's [`FullyQualifiedMoniker`].
    ///
    /// Populated by [`SpatialRegistry::record_focus`] for every scope
    /// ancestor walked during a focus event. Read by
    /// [`super::state::FallbackResolution::FallbackParentZoneLastFocused`]
    /// in preference to per-scope [`FocusScope::last_focused`] — the two
    /// stay synchronized via the dual-write in `record_focus`, but this
    /// top-level map is the authoritative slot going forward as the
    /// per-scope mirror is retired.
    pub last_focused_by_fq: HashMap<FullyQualifiedMoniker, FullyQualifiedMoniker>,
}

impl SpatialRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ---------------------------------------------------------------------
    // Scope ops
    // ---------------------------------------------------------------------

    /// Register a [`FocusScope`].
    ///
    /// Replaces any prior scope under the same FQM. Re-registration at
    /// the same FQM is part of the normal lifecycle — the virtualizer's
    /// placeholder→real-mount swap, React StrictMode dev-mode double
    /// effects, scroll-into-view, and ResizeObserver-driven rect
    /// refreshes all funnel through here repeatedly under the same
    /// path. The registry treats those silently: same `(segment,
    /// layer_fq, parent_zone, overrides)` tuple is a structural
    /// no-op and only the `rect` is refreshed. Any pre-existing
    /// `last_focused` slot on the prior registration — written by the
    /// kernel via [`Self::record_focus`] as focus moved through the
    /// scope — is preserved across the swap so drill-out memory
    /// survives the placeholder / real-mount cycle.
    ///
    /// A *structural* duplicate — same FQM but a different
    /// `(segment, layer_fq, parent_zone, overrides)` tuple — IS a
    /// programmer mistake (two primitives whose composed paths collide
    /// with conflicting metadata). Those still surface via
    /// `tracing::error!` so the noise stays bounded to genuine bugs
    /// while the second registration replaces the first to keep the
    /// registry consistent.
    pub fn register_scope(&mut self, mut f: FocusScope) {
        // Coordinate-system invariant check (debug-only, observability-
        // only). See `validate_rect_invariants` for the contract; logs
        // and continues on bad input so the registry stays consistent.
        validate_rect_invariants("register_scope", &f.fq, &f.rect);
        // A new registration in this layer invalidates any prior
        // coordinate-consistency validation; clear the slot so the
        // next nav into the layer re-runs the consistency walk.
        self.validated_layers.remove(&f.layer_fq);

        // Preserve any existing `last_focused` so a real-mount swap
        // from a placeholder doesn't lose drill-out memory accumulated
        // while the placeholder was live. The kernel writer
        // (`record_focus`) populates this slot as focus moves; new
        // registrations start with whatever was passed in (typically
        // `None`).
        if let Some(existing) = self.scopes.get(&f.fq) {
            warn_on_structural_mismatch(
                existing,
                &f.segment,
                &f.layer_fq,
                f.parent_zone.as_ref(),
                &f.overrides,
            );
            if f.last_focused.is_none() && existing.last_focused.is_some() {
                f.last_focused = existing.last_focused.clone();
            }
        }

        let fq = f.fq.clone();
        self.scopes.insert(fq.clone(), f);

        // Overlap check: a `<FocusScope>` registered at the same
        // rounded `(x, y)` as an existing scope in the same layer is
        // almost always a needless-nesting candidate (parent scope
        // wrapping a single child with no offset, sibling stacked at
        // the same anchor due to a pass-through wrapper).
        self.check_overlap_warning("register_scope", &fq);
    }

    /// Remove a scope from the registry.
    ///
    /// No-op if the FQM is unknown. The registry does **not** know about
    /// focus claims — adapters that maintain a [`SpatialState`] alongside
    /// the registry must also call
    /// [`SpatialState::handle_unregister`](crate::state::SpatialState::handle_unregister)
    /// on the same FQM so the per-window focus slot is cleared and a
    /// `Some → None` event is emitted for any claim that was active.
    ///
    /// Also drops the entry's per-key overlap-warn suppression slot so
    /// a fresh re-register at the same overlapping position emits a
    /// fresh `WARN` rather than being silently swallowed by stale
    /// suppression state. See
    /// [`overlap_warn_partner`](Self#structfield.overlap_warn_partner).
    pub fn unregister_scope(&mut self, fq: &FullyQualifiedMoniker) {
        // Invalidate the coordinate-consistency cache for the affected
        // layer before the entry leaves the map — a future nav into
        // the same layer should re-validate the (now smaller) scope set
        // rather than skip the walk on a stale "already validated" bit.
        if let Some(layer_fq) = self.scopes.get(fq).map(|s| s.layer_fq.clone()) {
            self.validated_layers.remove(&layer_fq);
        }
        self.scopes.remove(fq);
        self.overlap_warn_partner.remove(fq);
    }

    /// Record `fq` as the most recently focused descendant on every
    /// scope ancestor and every layer ancestor in the chain rooted at
    /// `fq`.
    ///
    /// This is the kernel writer for [`FocusScope::last_focused`],
    /// [`SpatialRegistry::last_focused_by_fq`], and
    /// [`FocusLayer::last_focused`]. [`super::state::SpatialState::focus`]
    /// and any other code path that mutates `focus_by_window` calls this
    /// after the new focus FQM has been validated. The walk:
    ///
    /// 1. Climbs the scope ancestor chain. When `snapshot` is `Some`,
    ///    the walk reads ancestors from
    ///    [`IndexedSnapshot::parent_zone_chain`]; when `None`, it walks
    ///    the registry's own `parent_zone` chain. For each visited
    ///    ancestor, both `last_focused_by_fq[ancestor] = fq` and
    ///    (when the ancestor is still present in `scopes`)
    ///    `scopes[ancestor].last_focused = Some(fq)` are written. The
    ///    walk terminates at the layer root or at a missing FQM (torn
    ///    state).
    /// 2. Climbs the layer ancestor chain via `FocusLayer::parent`,
    ///    setting each visited layer's `last_focused` to `fq`.
    ///    Terminates at the window root (`parent` is `None`) or at a
    ///    missing layer reference.
    ///
    /// Both walks make the cascade arms
    /// [`super::state::FallbackResolution::FallbackParentZoneLastFocused`]
    /// and
    /// [`super::state::FallbackResolution::FallbackParentLayerLastFocused`]
    /// reachable in production: when the focused scope is later
    /// unregistered, the resolver consults these recorded slots to
    /// land the user back on a meaningful target.
    ///
    /// The focused FQM itself is not written into its own scope's
    /// `last_focused` — that slot is reserved for descendants. A scope's
    /// own focus event is reflected only on its ancestors (and on its
    /// owning layer plus the layer's ancestors).
    ///
    /// In the registry-walk variant (`snapshot` is `None`), this is a
    /// no-op when `fq` is not a registered scope. In the snapshot-walk
    /// variant, the layer chain is still walked from
    /// `scopes[fq].layer_fq` if `fq` is registered locally; otherwise
    /// the layer phase is skipped.
    pub fn record_focus(
        &mut self,
        fq: &FullyQualifiedMoniker,
        snapshot: Option<&IndexedSnapshot<'_>>,
    ) {
        // Phase 1: walk the scope ancestor chain.
        //
        // The dual-write keeps `last_focused_by_fq` and the per-scope
        // `last_focused` mirror synchronized while the per-scope slot
        // is on its way out. The scope-side write degrades to a no-op
        // when an ancestor named in the snapshot is not present in the
        // registry — by design during cutover, when the registry no
        // longer mirrors React's scope tree.
        match snapshot {
            Some(idx) => {
                for ancestor in idx.parent_zone_chain(fq) {
                    self.last_focused_by_fq
                        .insert(ancestor.fq.clone(), fq.clone());
                    if let Some(zone) = self.scopes.get_mut(&ancestor.fq) {
                        zone.last_focused = Some(fq.clone());
                    }
                }
            }
            None => {
                let Some(focused) = self.scopes.get(fq) else {
                    return;
                };
                let mut next_zone = focused.parent_zone.clone();

                // `visited` guards against a self-referential or cyclic
                // `parent_zone` chain — e.g. a scope whose
                // `parent_zone == fq` (a React-side bug class fixed
                // alongside this guard) — which would otherwise loop
                // indefinitely while holding the registry mutex and
                // freeze every focus IPC.
                let mut visited: HashSet<FullyQualifiedMoniker> = HashSet::new();
                while let Some(zone_fq) = next_zone {
                    if !visited.insert(zone_fq.clone()) {
                        tracing::error!(
                            op = "record_focus",
                            cycle_fq = %zone_fq,
                            "parent_zone chain cycle detected; breaking walk"
                        );
                        break;
                    }
                    let Some(zone) = self.scopes.get_mut(&zone_fq) else {
                        // Torn registry — parent_zone names an FQM
                        // with no entry. Stop the walk; callers that
                        // care about structural integrity log
                        // elsewhere.
                        break;
                    };
                    zone.last_focused = Some(fq.clone());
                    self.last_focused_by_fq.insert(zone_fq.clone(), fq.clone());
                    next_zone = zone.parent_zone.clone();
                }
            }
        }

        // Phase 2: walk the layer ancestor chain (the focused scope's
        // own layer plus every ancestor reachable via
        // `FocusLayer::parent`). Each visited layer's `last_focused`
        // slot is set to `fq`. Same cycle guard as phase 1.
        let Some(layer_fq) = self.scopes.get(fq).map(|s| s.layer_fq.clone()) else {
            return;
        };
        let mut next_layer = Some(layer_fq);
        let mut visited_layers: HashSet<FullyQualifiedMoniker> = HashSet::new();
        while let Some(layer_fq) = next_layer {
            if !visited_layers.insert(layer_fq.clone()) {
                tracing::error!(
                    op = "record_focus",
                    cycle_fq = %layer_fq,
                    "layer parent chain cycle detected; breaking walk"
                );
                break;
            }
            let Some(layer) = self.layers.get_mut(&layer_fq) else {
                break;
            };
            layer.last_focused = Some(fq.clone());
            next_layer = layer.parent.clone();
        }
    }

    /// Update the bounding rect of a registered scope.
    ///
    /// No-op if the FQM is unknown. Called from the React side via
    /// `spatial_update_rect` when ResizeObserver fires.
    ///
    /// Emits the overlap `WARN` if the new rect lands the entry on
    /// top of another entry in the same layer. Per-key suppression
    /// elides re-warnings while the same overlap pair persists —
    /// `update_rect` fires every animation frame during scroll-tracking,
    /// so without the gate every frame would re-emit.
    pub fn update_rect(&mut self, fq: &FullyQualifiedMoniker, rect: Rect) {
        // Coordinate-system invariant check (debug-only). Validates
        // before the mutation so a bad rect surfaces in the log even
        // if the FQM is unknown (in which case the mutation is
        // dropped anyway, but the bug at the caller still gets logged).
        validate_rect_invariants("update_rect", fq, &rect);

        // Invalidate the consistency cache for the affected layer so
        // a coordinate-system mismatch newly introduced by this
        // update is caught on the next nav. We look the layer up via
        // the entry rather than have the caller pass it — a moved
        // entry stays in its layer.
        if let Some(layer_fq) = self.scopes.get(fq).map(|s| s.layer_fq.clone()) {
            self.validated_layers.remove(&layer_fq);
        }

        if let Some(scope) = self.scopes.get_mut(fq) {
            scope.rect = rect;
        }
        self.check_overlap_warning("update_rect", fq);
    }

    /// Lazy coordinate-system smoke check for a layer.
    ///
    /// Walks every scope in `layer_fq`'s layer, computes the centroid
    /// of all rect centers, and emits one `tracing::warn!` per scope
    /// whose distance to the centroid is more than
    /// `COORDINATE_CONSISTENCY_MULTIPLIER` × the median distance.
    /// That magnitude jump is a strong signal of a coordinate-system
    /// mismatch — half the layer registered with viewport-relative
    /// rects, the other half registered with document-relative rects.
    /// Production layouts cluster at one to two orders of magnitude
    /// of spread; the 10× bound is loose enough to ignore that and
    /// still catch the bug class.
    ///
    /// **Lazy**: the first call per layer runs the walk; subsequent
    /// calls return immediately until the layer is mutated (a
    /// `register_scope`, `update_rect`, `unregister_scope`, or
    /// `remove_layer` clears the cache and the next call re-walks).
    /// The intended call site is the navigator's first nav into the
    /// layer, where the registry is stable and the walk is paid for
    /// once per user session.
    ///
    /// **Observability-only**: emits log events but never panics, never
    /// returns an error, never refuses to compute. A coordinate-system
    /// mismatch is a programmer bug — the kernel logs it and continues
    /// to compute (possibly wrong) nav, on the same best-effort
    /// principle as the rest of the validators in this module.
    ///
    /// No-op when the layer has fewer than two scopes (no possible
    /// mismatch) or when the FQM is not a registered layer.
    pub fn validate_coordinate_consistency(&mut self, layer_fq: &FullyQualifiedMoniker) {
        if self.validated_layers.contains(layer_fq) {
            return;
        }
        // Mark the layer as validated up front so a re-entrant call
        // (unlikely in practice) cannot infinite-loop.
        self.validated_layers.insert(layer_fq.clone());

        // Collect (fq, center_x, center_y) for every scope in the
        // layer. Centroid-based distance ignores rect size, which is
        // the right axis for the coordinate-system check: a rect at
        // `(50000, 50000)` looks wrong even when its width matches
        // its peers'.
        let centers: Vec<(FullyQualifiedMoniker, f64, f64)> = self
            .scopes
            .values()
            .filter(|s| &s.layer_fq == layer_fq)
            .map(|s| {
                let r = &s.rect;
                let cx = r.left().value() + r.width.value() / 2.0;
                let cy = r.top().value() + r.height.value() / 2.0;
                (s.fq.clone(), cx, cy)
            })
            .collect();

        if centers.len() < 2 {
            return;
        }

        // Use the **median** of x and y as the "centroid" rather than
        // the mean — the mean is dragged toward an outlier (a rect
        // 100000 px away pulls the centroid halfway out, then sits
        // "near" it by the resulting metric), defeating the whole
        // point of the check. The median position stays anchored to
        // the bulk of the rects, so a coordinate-system mismatch
        // shows up as a single rect very far from the median position.
        let mut xs: Vec<f64> = centers.iter().map(|(_, x, _)| *x).collect();
        let mut ys: Vec<f64> = centers.iter().map(|(_, _, y)| *y).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Approximate / quickselect-style median: pick the upper-middle
        // element for even-sized arrays rather than averaging the two
        // middles. The textbook median would be `(xs[n/2 - 1] + xs[n/2])
        // / 2` for even `n`, but the goal here is a robust scale
        // estimate for the outlier-detection metric below — not a
        // statistically pure summary — and the off-by-half-a-rect
        // difference is irrelevant against the 10× multiplier the metric
        // applies. The same convention is used for `ys` and
        // `sorted_distances` below.
        let centroid_x = xs[xs.len() / 2];
        let centroid_y = ys[ys.len() / 2];

        let mut distances: Vec<(FullyQualifiedMoniker, f64)> = centers
            .into_iter()
            .map(|(fq, cx, cy)| {
                let dx = cx - centroid_x;
                let dy = cy - centroid_y;
                (fq, (dx * dx + dy * dy).sqrt())
            })
            .collect();

        // Median distance — a robust scale estimate that is not
        // skewed by one outlier the way the mean would be.
        let mut sorted_distances: Vec<f64> = distances.iter().map(|(_, d)| *d).collect();
        sorted_distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted_distances[sorted_distances.len() / 2];

        // A median of zero means every rect is at the centroid (all
        // entries share one coordinate); any non-zero distance is
        // technically infinite ratio. Skip the walk in that
        // degenerate case — the alternative would emit a warning
        // for every entry that happens to be the slightest bit
        // off-center, which is noise.
        if median <= f64::EPSILON {
            return;
        }

        let bound = median * COORDINATE_CONSISTENCY_MULTIPLIER;
        // Stable iteration order makes log output deterministic; the
        // `HashMap`'s iteration order is not, so we sort by FQM here.
        distances.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        for (fq, distance) in distances {
            if distance > bound {
                tracing::warn!(
                    target: "swissarmyhammer_focus::registry",
                    op = "validate_coordinate_consistency",
                    layer = %layer_fq,
                    fq = %fq,
                    distance,
                    median,
                    bound,
                    "scope rect is far from layer centroid; likely coordinate-system mismatch (mixed viewport-relative and document-relative rects?)"
                );
            }
        }
    }

    /// Detect an overlap for the entry at `fq` and emit `WARN` once
    /// per (entry, partner) overlap pair.
    ///
    /// `op` is the caller op tag (`"register_scope"` or
    /// `"update_rect"`). The entry must already be inserted in the
    /// scopes map — this helper reads the entry's layer and rect from
    /// the registry to scan for a same-(rounded x, y) partner in the
    /// same layer (excluding itself).
    ///
    /// Suppression rules, consulted via the
    /// [`overlap_warn_partner`](Self#structfield.overlap_warn_partner)
    /// map:
    ///
    /// - **No overlap found** — clear `fq`'s suppression slot. The
    ///   entry is no longer overlapping anyone; the next time it does
    ///   overlap (potentially the same partner again), the warn
    ///   should fire fresh.
    /// - **Overlap found, suppression slot already names this partner**
    ///   — skip the warn (this is the per-frame scroll-tracking case;
    ///   the same overlap pair from last call still holds).
    /// - **Overlap found, slot empty or names a different partner** —
    ///   emit one `WARN` and record the new partner in the slot.
    ///
    /// Skips silently when the registry has fewer than two entries
    /// total (cold start; nothing to overlap with) or when the FQM is
    /// unregistered (torn state, but not this helper's concern).
    fn check_overlap_warning(&mut self, op: &'static str, fq: &FullyQualifiedMoniker) {
        // Cold start guard — nothing to overlap with.
        if self.scopes.len() < 2 {
            self.overlap_warn_partner.remove(fq);
            return;
        }
        let Some(entry) = self.scopes.get(fq) else {
            return;
        };
        let entry_layer = entry.layer_fq.clone();
        let entry_rect = entry.rect;
        let entry_segment = entry.segment.clone();

        // Scan same-layer entries for a same-rounded-origin partner,
        // excluding ourselves. Yields the first match's FQM and segment
        // as owned values so we can release the immutable borrow before
        // mutating `overlap_warn_partner`.
        let partner: Option<(FullyQualifiedMoniker, SegmentMoniker)> = self
            .scopes
            .values()
            .filter(|other| other.layer_fq == entry_layer)
            .filter(|other| &other.fq != fq)
            .find(|other| same_rounded_origin(&entry_rect, &other.rect))
            .map(|other| (other.fq.clone(), other.segment.clone()));

        let Some((partner_fq, partner_segment)) = partner else {
            // No overlap — release any stale suppression slot so the
            // next overlap-creating motion emits fresh.
            self.overlap_warn_partner.remove(fq);
            return;
        };

        // Suppression: skip if the slot already names this partner.
        if self.overlap_warn_partner.get(fq) == Some(&partner_fq) {
            return;
        }

        warn_overlap(OverlapWarn {
            op,
            layer_fq: &entry_layer,
            new_fq: fq,
            new_segment: &entry_segment,
            overlap_fq: &partner_fq,
            overlap_segment: &partner_segment,
            rounded_x: rounded_pixel(entry_rect.left()),
            rounded_y: rounded_pixel(entry_rect.top()),
        });
        self.overlap_warn_partner.insert(fq.clone(), partner_fq);
    }

    /// Borrow a [`FocusScope`] by FQM, or `None` if the FQM is
    /// unregistered.
    ///
    /// Use [`is_registered`](Self::is_registered) for presence checks
    /// when the inner data isn't needed.
    pub fn scope(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusScope> {
        self.scopes.get(fq)
    }

    /// `true` when a scope is registered under `fq`.
    pub fn is_registered(&self, fq: &FullyQualifiedMoniker) -> bool {
        self.scopes.contains_key(fq)
    }

    /// Look up a registered scope by its canonical FQM.
    ///
    /// This is the **only** lookup-by-identifier API on the registry.
    /// The FQM is the canonical key; there is no leaf-form fallback,
    /// no UUID sidecar, no topmost-layer heuristic. An unknown FQM
    /// returns `None` and the higher-level caller emits
    /// `tracing::error!` per the no-silent-dropout contract.
    pub fn find_by_fq(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusScope> {
        self.scopes.get(fq)
    }

    /// Iterate every registered [`FocusScope`] in the registry,
    /// regardless of layer.
    pub fn scopes_iter(&self) -> impl Iterator<Item = &FocusScope> + '_ {
        self.scopes.values()
    }

    /// Iterate over the direct children of a scope — scopes whose
    /// `parent_zone` equals `parent_fq`.
    ///
    /// Direct children only; grandchildren whose `parent_zone` points at
    /// some other scope are excluded. A scope is a navigable container
    /// when this iterator is non-empty, and a leaf when it is empty.
    pub fn children_of(
        &self,
        parent_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &FocusScope> + '_ {
        let parent_fq = parent_fq.clone();
        self.scopes
            .values()
            .filter(move |s| s.parent_zone.as_ref() == Some(&parent_fq))
    }

    /// `true` when at least one scope is registered with
    /// `parent_zone == Some(parent_fq)`.
    ///
    /// Equivalent to `children_of(parent_fq).next().is_some()` and
    /// shorthand for the navigator's leaves-vs-containers tie-break.
    pub fn has_children(&self, parent_fq: &FullyQualifiedMoniker) -> bool {
        self.children_of(parent_fq).next().is_some()
    }

    /// Walk the `parent_zone` chain from the scope at `fq` upward,
    /// collecting each ancestor scope in innermost-first order.
    ///
    /// The scope at `fq` is **not** included in the result — only its
    /// ancestors. If `fq` is unknown, returns an empty vector.
    pub fn ancestor_zones(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusScope> {
        let mut chain = Vec::new();
        let Some(start) = self.scopes.get(fq) else {
            return chain;
        };

        let mut next = start.parent_zone.clone();
        while let Some(parent_fq) = next {
            let Some(parent) = self.scopes.get(&parent_fq) else {
                break;
            };
            chain.push(parent);
            next = parent.parent_zone.clone();
        }
        chain
    }

    /// Iterate every [`FocusScope`] in `layer_fq`'s layer.
    ///
    /// Used by the navigator when computing beam-search candidate sets
    /// — scopes outside the active layer are filtered out at this
    /// boundary rather than during scoring.
    pub fn scopes_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &FocusScope> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes.values().filter(move |s| s.layer_fq == layer_fq)
    }

    // ---------------------------------------------------------------------
    // Drill-in / drill-out — explicit zone descent / ascent
    // ---------------------------------------------------------------------

    /// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
    /// *into* the scope at `fq`.
    ///
    /// The semantics are:
    ///
    /// - **Scope with a live `last_focused`** — returns that descendant's
    ///   FQM, restoring the user's last position inside the scope
    ///   across drill-out / drill-in cycles.
    /// - **Scope with a stale or absent `last_focused`** — falls back to
    ///   the first child by rect top-left ordering (topmost wins; ties
    ///   broken by leftmost). Matches `Direction::First` ordering so the
    ///   keyboard model stays consistent.
    /// - **Scope with no children (a leaf)** — returns `focused_fq`.
    ///   The caller compares the result against the focused FQM; equal
    ///   means "no descent happened, fall through to edit / no-op".
    /// - **Unknown `fq`** — emits `tracing::error!` and returns
    ///   `focused_fq`. The error is observable in logs; the React side
    ///   stays put visually.
    ///
    /// Pure registry query — does not mutate state. The Tauri adapter
    /// translates the returned FQM into a `SpatialState::focus` call
    /// (or back into `setFocus` on the React side). See the
    /// [no-silent-dropout contract] on the `navigate` module for the
    /// reasoning behind echoing `focused_fq` rather than returning
    /// `Option<FullyQualifiedMoniker>`.
    ///
    /// [no-silent-dropout contract]: crate::navigate
    pub fn drill_in(
        &self,
        fq: FullyQualifiedMoniker,
        focused_fq: &FullyQualifiedMoniker,
    ) -> FullyQualifiedMoniker {
        let Some(entry) = self.scopes.get(&fq) else {
            // Torn state: caller passed an FQM with no registry entry.
            // Trace and echo the input FQM.
            tracing::error!(
                op = "drill_in",
                focused_fq = %fq,
                focused = %focused_fq,
                "unknown FQM passed to SpatialRegistry::drill_in"
            );
            return focused_fq.clone();
        };

        // Honor the scope's remembered position when it still resolves
        // to a registered scope. A `last_focused` whose target was since
        // unregistered is treated the same as no memory at all.
        if let Some(remembered) = &entry.last_focused {
            if let Some(remembered_entry) = self.scopes.get(remembered) {
                return remembered_entry.fq.clone();
            }
        }

        // Cold-start fallback: first child by rect top-left, via the
        // shared `first_child_by_top_left` helper. The navigator's
        // `Direction::First` edge command calls the same helper, so
        // drill-in's cold-start pick and `nav.first` cannot drift apart.
        // When the scope has no children at all, echo the focused FQM so
        // the caller's no-descent fall-through fires.
        first_child_by_top_left(self.children_of(&entry.fq))
            .map(|s| s.fq.clone())
            .unwrap_or_else(|| focused_fq.clone())
    }

    /// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
    /// *out of* the scope at `fq`.
    ///
    /// Returns the FQM of the scope's `parent_zone`. Walks the parent
    /// chain toward the layer root.
    ///
    /// Returns `focused_fq` when:
    /// - the scope at `fq` has no `parent_zone` (sits directly under
    ///   the layer root) — semantic "stay put"; the React side compares
    ///   the result against the focused FQM, equal means "fall
    ///   through to `app.dismiss` (close topmost modal layer)". No
    ///   tracing — this is a well-formed edge.
    /// - `fq` is unknown — torn registry state; emits
    ///   `tracing::error!` and returns the input FQM.
    /// - the `parent_zone` reference points at a scope that is no
    ///   longer registered — torn state; emits `tracing::error!` and
    ///   returns the input FQM.
    ///
    /// Pure registry query — does not mutate state. See the
    /// [no-silent-dropout contract] on the `navigate` module for the
    /// reasoning behind echoing `focused_fq` rather than returning
    /// `Option<FullyQualifiedMoniker>`.
    ///
    /// [no-silent-dropout contract]: crate::navigate
    pub fn drill_out(
        &self,
        fq: FullyQualifiedMoniker,
        focused_fq: &FullyQualifiedMoniker,
    ) -> FullyQualifiedMoniker {
        let Some(entry) = self.scopes.get(&fq) else {
            // Torn state: caller passed an FQM with no registry entry.
            tracing::error!(
                op = "drill_out",
                focused_fq = %fq,
                focused = %focused_fq,
                "unknown FQM passed to SpatialRegistry::drill_out"
            );
            return focused_fq.clone();
        };
        let Some(parent_zone_fq) = &entry.parent_zone else {
            // Layer-root edge — no enclosing scope. Well-formed; the
            // React adapter dispatches `app.dismiss` on the
            // FQM-equality fall-through.
            return focused_fq.clone();
        };
        let Some(parent_entry) = self.scopes.get(parent_zone_fq) else {
            // `parent_zone` names an FQM, but nothing is registered
            // there. Torn state.
            tracing::error!(
                op = "drill_out",
                focused_fq = %fq,
                focused = %focused_fq,
                parent_zone_fq = %parent_zone_fq,
                "parent_zone references unregistered scope"
            );
            return focused_fq.clone();
        };
        parent_entry.fq.clone()
    }

    // ---------------------------------------------------------------------
    // Layer ops
    // ---------------------------------------------------------------------

    /// Register a layer.
    ///
    /// Replaces any prior layer under the same FQM. The "stack" framing
    /// is on the React side (palette opens push, palette closes pop);
    /// the kanban-side store is just a flat map keyed by
    /// [`FullyQualifiedMoniker`].
    pub fn push_layer(&mut self, mut l: FocusLayer) {
        // Preserve any existing `last_focused` so a re-push (StrictMode
        // double-mount, palette open/close cycles, IPC re-batch) does
        // not lose drill-out memory accumulated by the kernel writer
        // (`record_focus`) while the prior push was live. New
        // registrations start with whatever was passed in (typically
        // `None`).
        if l.last_focused.is_none() {
            if let Some(existing) = self.layers.get(&l.fq) {
                l.last_focused = existing.last_focused.clone();
            }
        }

        self.layers.insert(l.fq.clone(), l);
    }

    /// Remove a layer from the registry.
    ///
    /// No-op if the FQM is unknown. Does not cascade to scopes that name
    /// this layer in their `layer_fq` — the React side unmounts those
    /// scopes first via `spatial_unregister_scope`, so the registry
    /// state remains consistent without a GC pass.
    pub fn remove_layer(&mut self, fq: &FullyQualifiedMoniker) {
        self.layers.remove(fq);
        // Drop the consistency-validated bit so a remounted layer is
        // re-validated on its next first nav.
        self.validated_layers.remove(fq);
    }

    /// Borrow a layer by FQM.
    pub fn layer(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusLayer> {
        self.layers.get(fq)
    }

    /// Direct children of a layer — layers whose `parent` equals `fq`.
    ///
    /// Returns `Vec<&FocusLayer>` rather than `impl Iterator` because
    /// callers typically need to count or sort the children, and the
    /// child set per layer is small (one inspector + maybe one dialog).
    pub fn children_of_layer(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusLayer> {
        self.layers
            .values()
            .filter(|l| l.parent.as_ref() == Some(fq))
            .collect()
    }

    /// The window root layer for `label` — a layer with the matching
    /// `window_label` and `parent = None`.
    ///
    /// Returns `None` if the window has not registered a root layer yet
    /// (cold start before the React side has called `spatial_push_layer`
    /// for the window). Per the layer-forest invariant there is at most
    /// one root per window; if the registry contains more than one due
    /// to a bug, the first found is returned. The invariant is enforced
    /// with `debug_assert!` in dev builds and a `tracing::warn!` in
    /// release builds so the corruption is visible without panicking
    /// on a user.
    pub fn root_for_window(&self, label: &WindowLabel) -> Option<&FocusLayer> {
        let roots: Vec<&FocusLayer> = self
            .layers
            .values()
            .filter(|l| l.parent.is_none() && &l.window_label == label)
            .collect();

        // Per the layer-forest invariant there is at most one root per
        // window. Two roots means an adapter pushed a second window-
        // root layer without first popping the previous one — a bug
        // worth surfacing rather than silently picking one.
        debug_assert!(
            roots.len() <= 1,
            "registry corruption: window {label} has {} root layers (expected ≤ 1)",
            roots.len()
        );
        if roots.len() > 1 {
            tracing::warn!(
                window_label = %label,
                root_count = roots.len(),
                "registry corruption: window has multiple root layers; returning first"
            );
        }

        roots.into_iter().next()
    }

    /// Walk the `parent` chain from the layer at `fq` upward, collecting
    /// each ancestor [`FocusLayer`] in innermost-first order.
    ///
    /// The layer at `fq` is **not** included in the result — only its
    /// ancestors. The walk stops at the window root (whose `parent` is
    /// `None`) or at a missing layer reference, whichever comes first.
    pub fn ancestors_of_layer(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusLayer> {
        let mut chain = Vec::new();
        let Some(start) = self.layers.get(fq) else {
            return chain;
        };

        let mut next = start.parent.clone();
        while let Some(parent_fq) = next {
            let Some(parent) = self.layers.get(&parent_fq) else {
                break;
            };
            chain.push(parent);
            next = parent.parent.clone();
        }
        chain
    }

    // ---------------------------------------------------------------------
    // Batch registration
    // ---------------------------------------------------------------------

    /// Apply a batch of [`RegisterEntry`] values to the registry under a
    /// single mutable borrow.
    ///
    /// This is the headless counterpart to the Tauri
    /// `spatial_register_batch` adapter. The virtualizer in
    /// `kanban-app/ui/src/components/column-view.tsx` constructs a
    /// `Vec<RegisterEntry>` (one entry per off-screen placeholder) and
    /// ships it through a single IPC invoke; the adapter holds the
    /// registry lock once and forwards the slice here.
    ///
    /// Iteration order is the order of the input vector. Successful
    /// batches are atomic at the registry boundary — observers see
    /// all-or-nothing.
    pub fn apply_batch(&mut self, entries: Vec<RegisterEntry>) {
        // Apply each entry. The registry's per-FQM overwrite semantics
        // handle the placeholder→real-mount rect refresh transparently;
        // scopes preserve their `last_focused` slot across re-registers
        // (rather than resetting it on every virtualizer pass) so
        // drill-out memory survives the swap.
        for entry in entries {
            let RegisterEntry {
                fq,
                segment,
                rect,
                layer_fq,
                parent_zone,
                overrides,
            } = entry;
            self.register_scope(FocusScope {
                fq,
                segment,
                rect,
                layer_fq,
                parent_zone,
                last_focused: None,
                overrides,
            });
        }
    }
}

/// Pick the topmost-then-leftmost child from `children`.
///
/// Compares `rect.top()`, breaking ties on `rect.left()`. This is the
/// canonical "first child" ordering shared by:
///
/// - [`SpatialRegistry::drill_in`]'s cold-start fallback (when the
///   target scope has no `last_focused` memory).
/// - The navigator's `Direction::First` edge command
///   (`navigate::edge_command`), and the deprecated
///   `Direction::RowStart` alias that routes through the same arm.
///
/// Both call sites previously carried verbatim copies of the same
/// `min_by` comparator. Centralising the ordering here means
/// behavioural drift between drill-in and `nav.first` is impossible by
/// construction — the two ops cannot diverge unless they stop calling
/// this helper.
pub(crate) fn first_child_by_top_left<'a>(
    children: impl Iterator<Item = &'a FocusScope>,
) -> Option<&'a FocusScope> {
    children.min_by(|a, b| {
        pixels_cmp(a.rect.top(), b.rect.top()).then(pixels_cmp(a.rect.left(), b.rect.left()))
    })
}

/// Pick the bottommost-then-rightmost child from `children`.
///
/// The mirror of [`first_child_by_top_left`]: compares
/// `rect.bottom()`, breaking ties on `rect.right()`. Comparing
/// bottoms (rather than tops) means a child whose top sits higher than
/// a sibling's but whose bottom extends below still wins. Used by the
/// navigator's `Direction::Last` edge command, and by the deprecated
/// `Direction::RowEnd` alias that routes through the same arm.
pub(crate) fn last_child_by_bottom_right<'a>(
    children: impl Iterator<Item = &'a FocusScope>,
) -> Option<&'a FocusScope> {
    children.max_by(|a, b| {
        pixels_cmp(a.rect.bottom(), b.rect.bottom())
            .then(pixels_cmp(a.rect.right(), b.rect.right()))
    })
}

/// One entry in a batch registration.
///
/// The wire-shape companion to [`FocusScope`] — reuses the same fields
/// and the same newtypes so the IPC boundary can be a single
/// `Vec<RegisterEntry>` payload.
///
/// `last_focused` is intentionally **not** carried on the wire:
/// registration is the React side's "this scope just mounted" signal,
/// and `last_focused` is server-owned drill-out memory written by the
/// kernel ([`SpatialRegistry::record_focus`], invoked by
/// [`super::state::SpatialState::focus`] on every successful focus
/// transition). The registry preserves any existing `last_focused`
/// slot when a scope is re-registered (the placeholder/real-mount
/// swap), so the lack of a wire field is correct rather than lossy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterEntry {
    /// Canonical FQM for this mount.
    pub fq: FullyQualifiedMoniker,
    /// Relative segment the consumer declared.
    pub segment: SegmentMoniker,
    /// Bounding rect in viewport coordinates.
    pub rect: Rect,
    /// Owning layer's FQM.
    pub layer_fq: FullyQualifiedMoniker,
    /// Immediate enclosing scope's FQM, if any.
    pub parent_zone: Option<FullyQualifiedMoniker>,
    /// Per-direction overrides.
    pub overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
}

impl RegisterEntry {
    /// Read the entry's [`FullyQualifiedMoniker`].
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        &self.fq
    }
}

#[cfg(test)]
mod tests {
    //! Unit-level coverage of the registry. Mirrors the integration
    //! coverage in `tests/focus_registry.rs` so contract drift is caught
    //! at the inner-crate compile step.

    use super::*;
    use crate::types::{FullyQualifiedMoniker, LayerName, Pixels, SegmentMoniker};
    use std::collections::HashMap;

    fn rect() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        }
    }

    fn make_scope(fq: &str, layer: &str, parent_zone: Option<&str>) -> FocusScope {
        FocusScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            rect: rect(),
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            last_focused: None,
            overrides: HashMap::new(),
        }
    }

    fn make_layer(fq: &str, window: &str, parent: Option<&str>) -> FocusLayer {
        FocusLayer {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            name: LayerName::from_string("window"),
            parent: parent.map(FullyQualifiedMoniker::from_string),
            window_label: WindowLabel::from_string(window),
            last_focused: None,
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope("/L/k", "/L", None));
        assert!(reg
            .scope(&FullyQualifiedMoniker::from_string("/L/k"))
            .is_some());
    }

    #[test]
    fn ancestor_zones_walks_chain() {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope("/L/outer", "/L", None));
        reg.register_scope(make_scope("/L/outer/inner", "/L", Some("/L/outer")));
        reg.register_scope(make_scope(
            "/L/outer/inner/leaf",
            "/L",
            Some("/L/outer/inner"),
        ));

        let chain: Vec<_> = reg
            .ancestor_zones(&FullyQualifiedMoniker::from_string("/L/outer/inner/leaf"))
            .into_iter()
            .map(|z| z.fq.as_str().to_string())
            .collect();
        assert_eq!(
            chain,
            vec!["/L/outer/inner".to_string(), "/L/outer".to_string()]
        );
    }

    #[test]
    fn has_children_reflects_registrations() {
        let mut reg = SpatialRegistry::new();
        reg.register_scope(make_scope("/L/parent", "/L", None));
        let parent_fq = FullyQualifiedMoniker::from_string("/L/parent");
        assert!(!reg.has_children(&parent_fq));

        reg.register_scope(make_scope("/L/parent/child", "/L", Some("/L/parent")));
        assert!(reg.has_children(&parent_fq));
    }

    #[test]
    fn root_for_window_finds_window_root() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(make_layer("/win-a", "win-a", None));
        reg.push_layer(make_layer("/win-a/ins", "win-a", Some("/win-a")));

        let root = reg
            .root_for_window(&WindowLabel::from_string("win-a"))
            .unwrap();
        assert_eq!(root.fq, FullyQualifiedMoniker::from_string("/win-a"));
    }

    // ---------------------------------------------------------------------
    // Coordinate-system validators (debug-only, observability-only).
    // ---------------------------------------------------------------------

    /// Tracing capture utility for the validator tests.
    ///
    /// Records every event at or above `Level::WARN` (which includes
    /// the `tracing::error!` calls in the rect-invariant validator and
    /// the `tracing::warn!` call in the coordinate-consistency walk),
    /// returning the structured field rendering of each one. Mirrors
    /// the per-test `capture_warns` helper in
    /// `tests/overlap_tracing.rs` but keeps the layer machinery local
    /// so the unit-test module owns a single copy.
    mod capture {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};
        use tracing::{
            field::{Field, Visit},
            span::Attributes,
            Event, Id, Level, Subscriber,
        };
        use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

        /// One captured event with its structured fields rendered to
        /// strings. The visitor below stores everything by name; tests
        /// look up specific keys (`op`, `fq`, `component`, …) to
        /// assert on contract details.
        #[derive(Debug, Default, Clone)]
        pub(super) struct CapturedEvent {
            pub(super) level: Option<Level>,
            pub(super) message: String,
            pub(super) fields: HashMap<String, String>,
        }

        impl CapturedEvent {
            pub(super) fn op(&self) -> Option<&str> {
                self.fields.get("op").map(String::as_str)
            }

            pub(super) fn field(&self, name: &str) -> Option<&str> {
                self.fields.get(name).map(String::as_str)
            }

            /// Returns `true` when the event was logged with the
            /// caller-supplied op tag, regardless of severity.
            pub(super) fn is_op(&self, op: &str) -> bool {
                self.op() == Some(op)
            }
        }

        struct FieldVisitor<'a> {
            fields: &'a mut HashMap<String, String>,
            message: &'a mut String,
        }

        impl<'a> Visit for FieldVisitor<'a> {
            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "message" {
                    self.message.push_str(value);
                } else {
                    self.fields
                        .insert(field.name().to_string(), value.to_string());
                }
            }
            fn record_i64(&mut self, field: &Field, value: i64) {
                self.fields
                    .insert(field.name().to_string(), value.to_string());
            }
            fn record_u64(&mut self, field: &Field, value: u64) {
                self.fields
                    .insert(field.name().to_string(), value.to_string());
            }
            fn record_f64(&mut self, field: &Field, value: f64) {
                self.fields
                    .insert(field.name().to_string(), format!("{value}"));
            }
            fn record_bool(&mut self, field: &Field, value: bool) {
                self.fields
                    .insert(field.name().to_string(), value.to_string());
            }
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message.push_str(&format!("{value:?}"));
                } else {
                    self.fields
                        .insert(field.name().to_string(), format!("{value:?}"));
                }
            }
        }

        struct CapturingLayer {
            events: Arc<Mutex<Vec<CapturedEvent>>>,
        }

        impl<S> Layer<S> for CapturingLayer
        where
            S: Subscriber + for<'a> LookupSpan<'a>,
        {
            fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
                let level = *event.metadata().level();
                if level > Level::WARN {
                    return;
                }
                let mut captured = CapturedEvent {
                    level: Some(level),
                    ..CapturedEvent::default()
                };
                let mut visitor = FieldVisitor {
                    fields: &mut captured.fields,
                    message: &mut captured.message,
                };
                event.record(&mut visitor);
                self.events.lock().unwrap().push(captured);
            }

            fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {}
        }

        /// Run `f` under a tracing subscriber that captures WARN/ERROR
        /// events into a `Vec<CapturedEvent>`. Returns the captured
        /// events plus whatever `f` returned.
        pub(super) fn capture<F, R>(f: F) -> (R, Vec<CapturedEvent>)
        where
            F: FnOnce() -> R,
        {
            let events = Arc::new(Mutex::new(Vec::new()));
            let layer = CapturingLayer {
                events: events.clone(),
            };
            let subscriber = tracing_subscriber::registry().with(layer);
            let result = tracing::subscriber::with_default(subscriber, f);
            let captured = events.lock().unwrap().clone();
            (result, captured)
        }
    }

    fn rect_xywh(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x: Pixels::new(x),
            y: Pixels::new(y),
            width: Pixels::new(w),
            height: Pixels::new(h),
        }
    }

    fn scope_with_rect(fq: &str, layer: &str, r: Rect) -> FocusScope {
        FocusScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            rect: r,
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: None,
            last_focused: None,
            overrides: HashMap::new(),
        }
    }

    /// Filter captured events down to rect-invariant validator events
    /// only — every such event carries either a `component` field
    /// (finite / scale violations) or a `width`/`height` field
    /// (positive-dim violations). We exclude consistency events
    /// (op = "validate_coordinate_consistency") so callers can mix
    /// the two predicates as needed.
    fn rect_invariant_events(events: &[capture::CapturedEvent]) -> Vec<&capture::CapturedEvent> {
        events
            .iter()
            .filter(|e| {
                matches!(e.op(), Some("register_scope") | Some("update_rect"))
                    && (e.field("component").is_some()
                        || e.field("width").is_some()
                        || e.field("height").is_some())
            })
            .collect()
    }

    #[test]
    fn register_scope_with_negative_width_logs_error() {
        let bad = scope_with_rect("/L/x", "/L", rect_xywh(0.0, 0.0, -10.0, 10.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(bad);
        });
        let events: Vec<_> = captured
            .iter()
            .filter(|e| e.is_op("register_scope") && e.field("width").is_some())
            .collect();
        assert!(
            !events.is_empty(),
            "expected at least one error for negative width, got {captured:?}"
        );
        assert_eq!(events[0].level, Some(tracing::Level::ERROR));
        assert_eq!(events[0].field("fq"), Some("/L/x"));
    }

    #[test]
    fn register_scope_with_infinite_y_logs_error() {
        let bad = scope_with_rect("/L/y", "/L", rect_xywh(0.0, f64::INFINITY, 10.0, 10.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(bad);
        });
        let events: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.is_op("register_scope")
                    && e.field("component") == Some("y")
                    && e.message.contains("not finite")
            })
            .collect();
        assert!(
            !events.is_empty(),
            "expected at least one finite-violation error for y, got {captured:?}"
        );
        assert_eq!(events[0].level, Some(tracing::Level::ERROR));
    }

    #[test]
    fn register_scope_with_sane_rect_logs_no_validator_event() {
        let good = scope_with_rect("/L/ok", "/L", rect_xywh(100.0, 80.0, 50.0, 30.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(good);
        });
        let bad = rect_invariant_events(&captured);
        assert!(
            bad.is_empty(),
            "expected no rect-invariant events for sane rect, got {bad:?}"
        );
    }

    #[test]
    fn register_scope_with_implausible_scale_logs_error() {
        let bad = scope_with_rect("/L/far", "/L", rect_xywh(50_000_000.0, 0.0, 10.0, 10.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(bad);
        });
        let events: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.is_op("register_scope")
                    && e.field("component") == Some("x")
                    && e.message.contains("plausible viewport range")
            })
            .collect();
        assert!(
            !events.is_empty(),
            "expected error for implausible x coordinate, got {captured:?}"
        );
    }

    #[test]
    fn register_scope_with_zero_dim_emits_no_error_or_warning() {
        // `getBoundingClientRect()` returns rects with zero dims for
        // `display: none`, just-mounted-but-not-yet-laid-out, and
        // detached nodes (in test environments, jsdom-style flex/grid
        // containers commonly produce `width × 0` zones too). On
        // registration that is not a coordinate-system bug — it's "the
        // registration `useEffect` ran before the first layout pass."
        // The validator falls through silently in this case (the
        // companion TS-side validator was de-noised in the same way);
        // the channel stays completely clean for the registration →
        // first-layout transition.
        let transient = scope_with_rect("/L/transient", "/L", rect_xywh(0.0, 0.0, 100.0, 0.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(transient);
        });
        let errors: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.is_op("register_scope")
                    && e.level == Some(tracing::Level::ERROR)
                    && (e.field("width").is_some() || e.field("height").is_some())
            })
            .collect();
        assert!(
            errors.is_empty(),
            "expected no width/height errors for zero-dim register, got {errors:?}"
        );
        let warnings: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.is_op("register_scope")
                    && e.level == Some(tracing::Level::WARN)
                    && e.message.contains("zero dimension")
            })
            .collect();
        assert!(
            warnings.is_empty(),
            "expected no pre-layout-transient warning (de-noised), got {warnings:?}"
        );
    }

    #[test]
    fn register_scope_with_both_zero_rect_warns_not_errors() {
        // The full structural pre-layout case: both dims are zero. Same
        // result as the partial-zero case on registration — single
        // warning, no error.
        let transient = scope_with_rect("/L/zero", "/L", rect_xywh(0.0, 0.0, 0.0, 0.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(transient);
        });
        let errors: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.is_op("register_scope")
                    && e.level == Some(tracing::Level::ERROR)
                    && (e.field("width").is_some() || e.field("height").is_some())
            })
            .collect();
        assert!(
            errors.is_empty(),
            "expected no width/height errors for 0x0 register, got {errors:?}"
        );
    }

    #[test]
    fn update_rect_with_zero_dim_logs_error_not_transient_warning() {
        // `update_rect` runs from ResizeObserver / ancestor-scroll, both
        // of which fire only after layout. A zero dim at this point is
        // a real bug — the kernel will record a persistent broken rect.
        let good = scope_with_rect("/L/post", "/L", rect_xywh(10.0, 10.0, 100.0, 100.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(good);
            reg.update_rect(
                &FullyQualifiedMoniker::from_string("/L/post"),
                rect_xywh(10.0, 10.0, 100.0, 0.0),
            );
        });
        let errors: Vec<_> = captured
            .iter()
            .filter(|e| {
                e.is_op("update_rect")
                    && e.level == Some(tracing::Level::ERROR)
                    && e.field("height").is_some()
            })
            .collect();
        assert!(
            !errors.is_empty(),
            "expected an error for zero-height update_rect, got {captured:?}"
        );
    }

    #[test]
    fn update_rect_with_bad_rect_logs_error() {
        let good = scope_with_rect("/L/m", "/L", rect_xywh(10.0, 10.0, 20.0, 20.0));
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            reg.register_scope(good);
            reg.update_rect(
                &FullyQualifiedMoniker::from_string("/L/m"),
                rect_xywh(0.0, 0.0, -1.0, 20.0),
            );
        });
        let events: Vec<_> = captured
            .iter()
            .filter(|e| e.is_op("update_rect") && e.field("width").is_some())
            .collect();
        assert!(
            !events.is_empty(),
            "expected update_rect to log on negative width, got {captured:?}"
        );
    }

    #[test]
    fn validate_coordinate_consistency_with_uniform_rects_does_not_warn() {
        let layer_fq = FullyQualifiedMoniker::from_string("/L");
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            // Five rects on a regular grid — all roughly equidistant
            // from the centroid.
            reg.register_scope(scope_with_rect(
                "/L/a",
                "/L",
                rect_xywh(0.0, 0.0, 50.0, 50.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/b",
                "/L",
                rect_xywh(100.0, 0.0, 50.0, 50.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/c",
                "/L",
                rect_xywh(0.0, 100.0, 50.0, 50.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/d",
                "/L",
                rect_xywh(100.0, 100.0, 50.0, 50.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/e",
                "/L",
                rect_xywh(50.0, 50.0, 50.0, 50.0),
            ));
            reg.validate_coordinate_consistency(&layer_fq);
        });
        let events: Vec<_> = captured
            .iter()
            .filter(|e| e.is_op("validate_coordinate_consistency"))
            .collect();
        assert!(
            events.is_empty(),
            "expected no consistency warnings for uniform rects, got {events:?}"
        );
    }

    #[test]
    fn validate_coordinate_consistency_with_one_far_outlier_warns() {
        let layer_fq = FullyQualifiedMoniker::from_string("/L");
        let (_, captured) = capture::capture(|| {
            let mut reg = SpatialRegistry::new();
            // Four rects clustered near the origin, one rect 10000×
            // further out — the classic "half viewport-relative,
            // half document-relative" signal.
            reg.register_scope(scope_with_rect(
                "/L/a",
                "/L",
                rect_xywh(0.0, 0.0, 10.0, 10.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/b",
                "/L",
                rect_xywh(10.0, 0.0, 10.0, 10.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/c",
                "/L",
                rect_xywh(0.0, 10.0, 10.0, 10.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/d",
                "/L",
                rect_xywh(10.0, 10.0, 10.0, 10.0),
            ));
            reg.register_scope(scope_with_rect(
                "/L/far",
                "/L",
                rect_xywh(100_000.0, 100_000.0, 10.0, 10.0),
            ));
            reg.validate_coordinate_consistency(&layer_fq);
        });
        let events: Vec<_> = captured
            .iter()
            .filter(|e| e.is_op("validate_coordinate_consistency"))
            .collect();
        assert!(
            !events.is_empty(),
            "expected consistency warning for the far outlier, got {captured:?}"
        );
        // The far rect should be the one flagged; depending on
        // centroid math the other rects might also be flagged when
        // the far rect drags the centroid heavily. At minimum the
        // outlier itself must appear.
        assert!(
            events.iter().any(|e| e.field("fq") == Some("/L/far")),
            "expected /L/far in flagged FQMs, got {:?}",
            events.iter().map(|e| e.field("fq")).collect::<Vec<_>>()
        );
    }

    #[test]
    fn validate_coordinate_consistency_is_lazy_per_layer() {
        let layer_fq = FullyQualifiedMoniker::from_string("/L");
        let mut reg = SpatialRegistry::new();
        reg.register_scope(scope_with_rect(
            "/L/a",
            "/L",
            rect_xywh(0.0, 0.0, 10.0, 10.0),
        ));
        reg.register_scope(scope_with_rect(
            "/L/far",
            "/L",
            rect_xywh(100_000.0, 100_000.0, 10.0, 10.0),
        ));
        reg.register_scope(scope_with_rect(
            "/L/b",
            "/L",
            rect_xywh(10.0, 0.0, 10.0, 10.0),
        ));
        reg.register_scope(scope_with_rect(
            "/L/c",
            "/L",
            rect_xywh(0.0, 10.0, 10.0, 10.0),
        ));
        reg.register_scope(scope_with_rect(
            "/L/d",
            "/L",
            rect_xywh(10.0, 10.0, 10.0, 10.0),
        ));

        // First call walks the layer.
        let (_, first) = capture::capture(|| {
            reg.validate_coordinate_consistency(&layer_fq);
        });
        let first_events: Vec<_> = first
            .iter()
            .filter(|e| e.is_op("validate_coordinate_consistency"))
            .collect();
        assert!(!first_events.is_empty(), "first call should walk and warn");

        // Second call is a no-op until the layer is mutated.
        let (_, second) = capture::capture(|| {
            reg.validate_coordinate_consistency(&layer_fq);
        });
        let second_events: Vec<_> = second
            .iter()
            .filter(|e| e.is_op("validate_coordinate_consistency"))
            .collect();
        assert!(
            second_events.is_empty(),
            "second call without mutation should be a no-op, got {second_events:?}"
        );

        // Mutation invalidates the cache: a fresh register_scope
        // resets the slot, and the next call re-walks.
        reg.register_scope(scope_with_rect(
            "/L/e",
            "/L",
            rect_xywh(20.0, 20.0, 10.0, 10.0),
        ));
        let (_, third) = capture::capture(|| {
            reg.validate_coordinate_consistency(&layer_fq);
        });
        let third_events: Vec<_> = third
            .iter()
            .filter(|e| e.is_op("validate_coordinate_consistency"))
            .collect();
        assert!(
            !third_events.is_empty(),
            "post-mutation call should re-walk and warn, got {third_events:?}"
        );
    }
}
