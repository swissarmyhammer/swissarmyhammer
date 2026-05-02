//! `SpatialRegistry` — the headless store for spatial scopes and layers.
//!
//! The registry holds two flat maps:
//!
//! - `scopes: HashMap<FullyQualifiedMoniker, RegisteredScope>` — every
//!   registered leaf or container, keyed by its canonical FQM. The
//!   discriminator between leaves and zones lives on an internal enum
//!   ([`super::scope::RegisteredScope`]); the public API exposes the
//!   two typed structs ([`FocusScope`], [`FocusZone`]) directly.
//! - `layers: HashMap<FullyQualifiedMoniker, FocusLayer>` — every
//!   registered layer node, keyed by its FQM.
//!
//! Tree / forest structure is **derived**, not stored: zone hierarchy
//! comes from each scope's `parent_zone`, layer hierarchy from each
//! layer's `parent`. This keeps mutation simple (one map insert per mount)
//! and makes the structural queries (`children_of_zone`, `ancestor_zones`,
//! `children_of_layer`, `ancestors_of_layer`) the source of truth for
//! "what's inside what".
//!
//! ## Path-monikers identifier model
//!
//! The kernel uses **one** identifier shape per primitive: the
//! [`FullyQualifiedMoniker`]. The path through the focus hierarchy IS
//! the spatial key. A consumer constructing a `<FocusZone>` declares
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
//! ## Three peers, not four
//!
//! The kernel exposes three peer types: [`super::layer::FocusLayer`],
//! [`FocusZone`], and [`FocusScope`]. There is no public sum-type enum
//! that conflates leaves and zones — consumers iterate the registry via
//! the variant-aware helpers ([`leaves_in_layer`], [`zones_in_layer`],
//! [`leaves_iter`], [`zones_iter`]) which yield the typed structs. This
//! mirrors the React side, where `<FocusLayer>`, `<FocusZone>`, and
//! `<FocusScope>` are the three components.
//!
//! [`leaves_in_layer`]: SpatialRegistry::leaves_in_layer
//! [`zones_in_layer`]: SpatialRegistry::zones_in_layer
//! [`leaves_iter`]: SpatialRegistry::leaves_iter
//! [`zones_iter`]: SpatialRegistry::zones_iter
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
//! `SpatialRegistry` tracks the geometry / layer / zone structure that
//! the navigator reads to compute the next focus target. The two are
//! intentionally separate: focus state mutates frequently (every
//! keystroke), structural data mutates only on mount / unmount / resize.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::layer::FocusLayer;
use super::scope::{FocusScope, FocusZone, RegisteredScope};
use super::types::{
    pixels_cmp, Direction, FullyQualifiedMoniker, Pixels, Rect, SegmentMoniker, WindowLabel,
};

/// Emit `tracing::error!` only when re-registering at an already-occupied
/// FQM with a *structurally different* entry — i.e. a mismatch in the
/// kind discriminator (zone vs scope) or in any of the structural
/// fields (`segment`, `layer_fq`, `parent_zone`, `overrides`).
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
/// * **React StrictMode dev double-mount.** The `<FocusScope>` /
///   `<FocusZone>` register effect runs, cleans up, and re-runs in a
///   single mount under StrictMode. Both register IPCs ship with
///   identical structural data; the cleanup's unregister IPC sits in
///   between, so this is *normally* not even a duplicate at the kernel.
///   But if any IPC reordering or batching causes the second register
///   to land before the cleanup unregister, the kernel still sees a
///   same-shape re-register.
///
/// * **ResizeObserver-driven rect refresh.** The same `<FocusScope>`
///   re-fires its register effect when its dependency tuple shifts
///   (e.g. `parent_zone` or `layer_fq` recomputed identically by
///   context, but the React reconciler still re-runs the effect).
///
/// A genuine programmer mistake — two primitives whose composed paths
/// collide with conflicting metadata (different segments, different
/// enclosing zones / layers, different override sets) or with a kind
/// flip — still trips the error log so it stays visible.
///
/// `op` is the calling registration op for log readability
/// (`"register_scope"` or `"register_zone"`).
fn warn_on_structural_mismatch(
    op: &'static str,
    existing: &RegisteredScope,
    new_segment: &SegmentMoniker,
    new_layer_fq: &FullyQualifiedMoniker,
    new_parent_zone: Option<&FullyQualifiedMoniker>,
    new_overrides: &HashMap<Direction, Option<FullyQualifiedMoniker>>,
    new_is_zone: bool,
) {
    let kind_flipped = existing.is_zone() != new_is_zone;
    let segment_differs = existing.segment() != new_segment;
    let layer_differs = existing.layer_fq() != new_layer_fq;
    let parent_zone_differs = existing.parent_zone() != new_parent_zone;
    let overrides_differ = existing.overrides() != new_overrides;

    if kind_flipped || segment_differs || layer_differs || parent_zone_differs || overrides_differ {
        tracing::error!(
            op,
            fq = %existing.fq(),
            kind_flipped,
            segment_differs,
            layer_differs,
            parent_zone_differs,
            overrides_differ,
            "duplicate FQM registration with structural mismatch — \
             two primitives composed the same path but disagree on \
             segment / layer / parent_zone / overrides / kind. \
             Replacing prior entry; nav may be inconsistent until \
             the offending primitive is fixed."
        );
    }
}

/// Emit `tracing::error!` when the entry being registered violates the
/// **scope-is-leaf** invariant.
///
/// The kernel's three peers are [`super::layer::FocusLayer`] (modal
/// boundary), [`FocusZone`] (navigable container — may have children),
/// and [`FocusScope`] (leaf — no navigable children). Wrapping a
/// non-leaf as a [`FocusScope`] confuses beam search (the scope's rect
/// is treated as a single leaf candidate even though it spans a whole
/// sub-region) and breaks "drill into the bar and remember the
/// last-focused leaf" — the enclosing zone's `last_focused` ends up
/// pointing at the scope wrapper, not the actually-focused inner
/// control. The misuse silently degrades keyboard nav in toolbars; this
/// log surfaces it so a developer can `just logs | grep scope-not-leaf`
/// and find the offender.
///
/// The violation is detected through one of two **relations** between
/// the offender and the ancestor Scope:
///
/// - `"parent-zone"` — the offender's `parent_zone` field literally
///   names the ancestor Scope's FQM. Rare in production: descendants
///   read `parent_zone` from `useParentZoneFq()`, which walks
///   `FocusZoneContext` and skips Scopes (Scopes do not push that
///   context).
/// - `"path-prefix"` — the offender's FQM is a strict path-descendant of
///   the ancestor Scope's FQM, i.e. the React tree composed the offender
///   inside the Scope's `<FocusScope>` (which DOES push
///   `<FullyQualifiedMonikerContext.Provider>`). This is the common case
///   in production and the path-prefix branch is what catches misused
///   `<FocusScope>` wrappers like the entity card and the board view.
/// - `"both"` — both relations apply; emitted once per offender × ancestor
///   pair so the log is one event per logical violation, not per relation.
///
/// The message carries the literal `scope-not-leaf` substring so a grep
/// pipeline filters it out of the broader log stream without risking
/// false positives on adjacent registry warnings.
///
/// `kind` is the offending child's own kind discriminator (`"scope"`,
/// `"zone"`, or `"layer"`); `parent_kind` is the resolved parent's kind
/// discriminator (always `"scope"` today — the helper is used by the
/// scope-is-leaf invariant only). Both are passed by the caller because
/// `RegisteredScope` is private. `relation` discriminates the detection
/// branch, as described above. Layers only ever match the path-prefix
/// branch — they do not have a `parent_zone` field, only a `parent`
/// (layer) field that always points at another Layer FQM, never a
/// scope/zone FQM.
fn warn_scope_not_leaf(
    fq: &FullyQualifiedMoniker,
    segment: &SegmentMoniker,
    parent_zone: &FullyQualifiedMoniker,
    parent_segment: &SegmentMoniker,
    kind: &'static str,
    parent_kind: &'static str,
    relation: &'static str,
) {
    tracing::error!(
        target: "swissarmyhammer_focus::registry",
        kind,
        fq = %fq,
        segment = %segment,
        parent_zone = %parent_zone,
        parent_segment = %parent_segment,
        parent_kind,
        relation,
        "scope-not-leaf — FocusScope registered under a parent that is itself \
         a leaf scope; scope must be a leaf, parent must be a Zone"
    );
}

/// `true` when `child_fq` is a strict path-descendant of `ancestor_fq`,
/// i.e. its FQM string begins with `"{ancestor_fq}/"`. The trailing slash
/// guard prevents false matches between sibling FQMs that share a prefix
/// up to the segment boundary (e.g. `/L/task:T1A` vs `/L/task:T1`).
///
/// Path-descendant is distinct from `parent_zone` ancestry: it captures
/// the React tree shape (`<FullyQualifiedMonikerContext>` composition)
/// rather than the spatial-graph parent_zone field. The two diverge for a
/// `<FocusScope>` containing `<FocusZone>` descendants — exactly the
/// violation [`warn_scope_not_leaf_by_path`] is designed to catch.
fn is_path_descendant(
    child_fq: &FullyQualifiedMoniker,
    ancestor_fq: &FullyQualifiedMoniker,
) -> bool {
    let ancestor = ancestor_fq.as_str();
    let child = child_fq.as_str();
    // Strict descendant: child must be longer than ancestor and the next
    // char after the ancestor prefix must be the path separator.
    child.len() > ancestor.len() + 1
        && child.starts_with(ancestor)
        && child.as_bytes()[ancestor.len()] == FQ_PATH_SEPARATOR_BYTE
}

/// The `'/'` path separator as a byte. Mirrors the wire-format constant
/// used by the React side and by [`FullyQualifiedMoniker::compose`]; kept
/// local so [`is_path_descendant`] does not pull the FQ separator from
/// the [`super::types`] module's private constant.
const FQ_PATH_SEPARATOR_BYTE: u8 = b'/';

/// Compare an existing registry entry against a pending registration and
/// return `true` when the structural shape is unchanged.
///
/// Same shape ⇔ same kind discriminator AND identical
/// `(segment, layer_fq, parent_zone, overrides)` tuple. This mirrors the
/// invariant pinned by [`warn_on_structural_mismatch`]: rect refreshes are
/// not structural, kind flips and metadata changes are.
///
/// Used by [`SpatialRegistry::register_scope`] and
/// [`SpatialRegistry::register_zone`] to gate the **scope-is-leaf**
/// checks: same-shape re-registration is the hot path
/// (StrictMode double-mount, ResizeObserver rect refresh, virtualizer
/// placeholder→real-mount swap) and must stay silent — otherwise an
/// already-reported illegal edge re-fires `scope-not-leaf` on every
/// render.
fn same_shape(
    existing: &RegisteredScope,
    new_segment: &SegmentMoniker,
    new_layer_fq: &FullyQualifiedMoniker,
    new_parent_zone: Option<&FullyQualifiedMoniker>,
    new_overrides: &HashMap<Direction, Option<FullyQualifiedMoniker>>,
    new_is_zone: bool,
) -> bool {
    existing.is_zone() == new_is_zone
        && existing.segment() == new_segment
        && existing.layer_fq() == new_layer_fq
        && existing.parent_zone() == new_parent_zone
        && existing.overrides() == new_overrides
}

/// Compare an existing registered [`FocusLayer`] against a pending
/// `push_layer` payload and return `true` when the structural shape is
/// unchanged.
///
/// Same shape ⇔ identical `(segment, name, parent, window_label)` tuple.
/// `last_focused` is mutable runtime state populated by the navigator on
/// focus changes inside the layer; it intentionally does NOT participate
/// in the shape comparison so a layer that has acquired focus history is
/// not mis-classified as "structurally novel" on a same-shape re-mount.
///
/// Used by [`SpatialRegistry::push_layer`] to gate the **scope-is-leaf**
/// path-prefix check for layers: same-shape re-registration is the hot
/// path (StrictMode double-mount, palette open/close cycles that re-push
/// the same layer) and must stay silent so an already-reported illegal
/// Layer-under-Scope edge does not re-fire on every render.
fn same_shape_layer(existing: &FocusLayer, candidate: &FocusLayer) -> bool {
    existing.segment == candidate.segment
        && existing.name == candidate.name
        && existing.parent == candidate.parent
        && existing.window_label == candidate.window_label
}

/// Round a [`Pixels`] coordinate to its nearest integer pixel as
/// `i64`.
///
/// Subpixel rendering produces tiny variations between successive
/// `getBoundingClientRect()` reads on the same DOM node (anti-aliased
/// borders, ResizeObserver fractional dpr math) that aren't user-
/// relevant. The same-(x, y) overlap check rounds before comparing so
/// it catches structural overlaps (parent zone wrapping a single child
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
/// is "two same-kind entries anchored at the same point", which is
/// what catches needless-nesting wrappers regardless of whether the
/// inner entry trims a few pixels of padding.
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
    /// Calling op tag (`"register_scope"`, `"register_zone"`,
    /// `"update_rect"`).
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
    /// FQM of the pre-existing same-kind entry the new one landed on
    /// top of.
    overlap_fq: &'a FullyQualifiedMoniker,
    /// Relative segment of the partner.
    overlap_segment: &'a SegmentMoniker,
    /// Shared rounded x-coordinate in viewport space.
    rounded_x: i64,
    /// Shared rounded y-coordinate in viewport space.
    rounded_y: i64,
}

/// Emit one `WARN`-level tracing event for a same-kind overlap.
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
        "two same-kind entries share (x, y); likely needless-nesting — review React tree for redundant wrappers"
    );
}

/// Headless store for spatial scopes and layers.
///
/// See module docs for the threading model and the split between scopes
/// and layers. `Default` produces an empty registry; `new` is provided
/// for symmetry with `SpatialState::new`.
#[derive(Debug, Default, Clone)]
pub struct SpatialRegistry {
    /// All registered focus points keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Both [`FocusScope`] leaves and
    /// [`FocusZone`] containers live here behind the internal
    /// [`RegisteredScope`] enum — the public API exposes the typed
    /// structs only.
    scopes: HashMap<FullyQualifiedMoniker, RegisteredScope>,
    /// All registered layers keyed by their canonical
    /// [`FullyQualifiedMoniker`]. Layer hierarchy is derived from each
    /// layer's `parent` field, not stored here.
    layers: HashMap<FullyQualifiedMoniker, FocusLayer>,
    /// Per-entry suppression state for the same-kind overlap warning.
    ///
    /// Maps an entry's FQM to the FQM of the same-kind partner it was
    /// last reported as overlapping. The registry consults this map
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
}

impl SpatialRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ---------------------------------------------------------------------
    // Scope ops
    // ---------------------------------------------------------------------

    /// Register a [`FocusScope`] leaf.
    ///
    /// Replaces any prior scope under the same FQM. Re-registration at
    /// the same FQM is part of the normal lifecycle — the virtualizer's
    /// placeholder→real-mount swap, React StrictMode dev-mode double
    /// effects, scroll-into-view, and ResizeObserver-driven rect
    /// refreshes all funnel through here repeatedly under the same
    /// path. The registry treats those silently: same `(segment,
    /// layer_fq, parent_zone, overrides)` tuple is a structural
    /// no-op and only the `rect` is refreshed.
    ///
    /// A *structural* duplicate — same FQM but a different
    /// `(segment, layer_fq, parent_zone, overrides)` tuple, or a kind
    /// flip from zone→scope — IS a programmer mistake (two primitives
    /// whose composed paths collide with conflicting metadata, or two
    /// disagreeing variants). Those still surface via `tracing::error!`
    /// so the noise stays bounded to genuine bugs while the second
    /// registration replaces the first to keep the registry consistent.
    ///
    /// A `<FocusScope>` is a **leaf** in the spatial graph: it must not
    /// contain a [`FocusScope`], a [`FocusZone`], **or** a
    /// [`FocusLayer`]. The kernel enforces this with five checks — four
    /// against the scopes map (scope/zone descendants and ancestors) and
    /// one against the layers map (layer descendants). All five are
    /// gated on structural novelty:
    ///
    /// 1. *Forward (parent-zone)* — if the new scope's `parent_zone`
    ///    resolves to an existing leaf [`FocusScope`], emit one
    ///    `scope-not-leaf` error pointing at the new scope. A
    ///    `<FocusScope>` cannot wrap further focus primitives — see
    ///    [`warn_scope_not_leaf`] and
    ///    `swissarmyhammer-focus/tests/scope_is_leaf.rs`.
    /// 2. *Backward (parent-zone)* — after the new scope is inserted,
    ///    scan for any pre-existing entries whose `parent_zone` names
    ///    this scope's FQM and emit one `scope-not-leaf` error per
    ///    such offender. This makes the check order-independent: a
    ///    child registered before its parent's kind is known is
    ///    re-validated here when the parent eventually registers as a
    ///    leaf.
    /// 3. *Forward (path-prefix)* — if some already-registered Scope's
    ///    FQM is a strict path-prefix of the new scope's FQM (i.e. the
    ///    new scope's React subtree was rendered inside that scope's
    ///    `<FocusScope>`), emit one `scope-not-leaf` error per such
    ///    ancestor. This is the catch-all that fires when the React
    ///    side composes through a Scope without that Scope appearing in
    ///    `parent_zone` (because `<FocusScope>` does not push a
    ///    `FocusZoneContext.Provider`, so descendants pick the nearest
    ///    enclosing `<FocusZone>` for `parent_zone`, skipping the
    ///    offending Scope).
    /// 4. *Backward (path-prefix, scopes/zones)* — after the new scope
    ///    is inserted, scan the scopes map for any pre-existing entries
    ///    whose FQM is a strict path-descendant of this scope's FQM and
    ///    emit one `scope-not-leaf` error per such offender. The
    ///    path-prefix branch is what catches the entity-card /
    ///    board-view shape where the scope wraps a non-trivial subtree
    ///    containing `<FocusZone>` (e.g. `<Field>`) descendants.
    /// 5. *Backward (path-prefix, layers)* — after the new scope is
    ///    inserted, scan the layers map for any pre-existing
    ///    [`FocusLayer`] whose FQM is a strict path-descendant of this
    ///    scope's FQM and emit one `scope-not-leaf` error per such
    ///    offender, tagged `kind = "layer"`. A layer mounted inside a
    ///    `<FocusScope>` is just as illegal as a zone mounted inside
    ///    one — the scope is a leaf, period. Walked in the same single
    ///    pass as check 4 (see [`warn_backward_scope_descendants`]) so
    ///    a single backward scan covers all three primitive kinds.
    ///
    /// All five checks are gated on **structural novelty** (new FQM, kind
    /// flip Zone→Scope, or any change to the
    /// `(segment, layer_fq, parent_zone, overrides)` tuple). Same-shape
    /// re-registration is silent on the same hot paths that
    /// [`warn_on_structural_mismatch`] silences — StrictMode
    /// double-mount, ResizeObserver rect refresh, and the virtualizer
    /// placeholder→real-mount swap. Without that gate, an already-known
    /// illegal edge would re-fire `scope-not-leaf` on every render.
    /// The contract is therefore: **exactly one error per structurally
    /// novel offending edge**, regardless of registration order.
    ///
    /// [`warn_backward_scope_descendants`]: Self::warn_backward_scope_descendants
    pub fn register_scope(&mut self, f: FocusScope) {
        let shape_unchanged = self
            .scopes
            .get(&f.fq)
            .map(|existing| {
                warn_on_structural_mismatch(
                    "register_scope",
                    existing,
                    /* new_segment */ &f.segment,
                    /* new_layer_fq */ &f.layer_fq,
                    /* new_parent_zone */ f.parent_zone.as_ref(),
                    /* new_overrides */ &f.overrides,
                    /* new_is_zone */ false,
                );
                same_shape(
                    existing,
                    &f.segment,
                    &f.layer_fq,
                    f.parent_zone.as_ref(),
                    &f.overrides,
                    /* new_is_zone */ false,
                )
            })
            .unwrap_or(false);

        if !shape_unchanged {
            // Forward checks: emit one `scope-not-leaf` per ancestor
            // Scope of the new entry. An ancestor is detected via
            // either its `parent_zone` field (literal naming) or its
            // FQM path (DOM-subtree containment). When both relations
            // apply to the same `(offender, ancestor)` pair the helper
            // emits a single event tagged with `relation = "both"` so
            // the log stays one-event-per-offender-per-ancestor.
            self.warn_forward_scope_ancestors(&f.fq, &f.segment, f.parent_zone.as_ref(), "scope");
        }

        let fq = f.fq.clone();
        let parent_segment = f.segment.clone();
        self.scopes.insert(fq.clone(), RegisteredScope::Scope(f));

        if !shape_unchanged {
            // Backward checks: we just inserted as a (structurally
            // novel) Scope. Any pre-existing entry that named us as
            // their `parent_zone` *or* whose FQM is a strict
            // path-descendant of ours is now retroactively illegal.
            // Fire one event per descendant — order-independent
            // detection without a deferred-validation queue.
            self.warn_backward_scope_descendants(&fq, &parent_segment);
        }

        // Same-kind overlap check: a `<FocusScope>` registered at the
        // same rounded `(x, y)` as an existing scope in the same layer
        // is almost always a needless-nesting candidate (parent zone
        // wrapping a single child with no offset, sibling stacked at
        // the same anchor due to a pass-through wrapper).
        self.check_overlap_warning("register_scope", &fq);
    }

    /// Register a [`FocusZone`] container.
    ///
    /// Replaces any prior scope under the same FQM. Same semantics as
    /// [`register_scope`] — same-shape re-registration is a silent
    /// no-op (the placeholder→real-mount and StrictMode-double-mount
    /// paths land here every render); a structural mismatch still
    /// surfaces via `tracing::error!`.
    ///
    /// Two **forward** scope-is-leaf checks fire when the new entry is
    /// structurally novel:
    ///
    /// - *parent-zone* — if the new zone's `parent_zone` resolves to an
    ///   existing leaf [`FocusScope`], emit one `scope-not-leaf` error.
    ///   A leaf cannot contain a navigable container.
    /// - *path-prefix* — if any already-registered [`FocusScope`]'s FQM
    ///   is a strict prefix of the new zone's FQM, the React tree
    ///   composed this zone inside a misused `<FocusScope>` — fire one
    ///   `scope-not-leaf` error per offending Scope.
    ///
    /// Same-shape re-registration is silent — the already-reported edge
    /// would otherwise re-fire on every render under StrictMode /
    /// ResizeObserver / the virtualizer swap.
    ///
    /// No backward check is needed here — a [`FocusZone`] is a legal
    /// parent for both scopes and zones, so any pre-existing children
    /// pointing at this FQM stay valid. The forward-only branch is the
    /// asymmetry between the two register entry points.
    ///
    /// [`register_scope`]: SpatialRegistry::register_scope
    pub fn register_zone(&mut self, z: FocusZone) {
        let shape_unchanged = self
            .scopes
            .get(&z.fq)
            .map(|existing| {
                warn_on_structural_mismatch(
                    "register_zone",
                    existing,
                    /* new_segment */ &z.segment,
                    /* new_layer_fq */ &z.layer_fq,
                    /* new_parent_zone */ z.parent_zone.as_ref(),
                    /* new_overrides */ &z.overrides,
                    /* new_is_zone */ true,
                );
                same_shape(
                    existing,
                    &z.segment,
                    &z.layer_fq,
                    z.parent_zone.as_ref(),
                    &z.overrides,
                    /* new_is_zone */ true,
                )
            })
            .unwrap_or(false);

        if !shape_unchanged {
            // Forward checks: emit one `scope-not-leaf` per ancestor
            // Scope of the new Zone, detected via either `parent_zone`
            // naming or FQM path-prefix. A Zone under a Scope is
            // illegal under both relations; the helper deduplicates
            // when both apply to the same ancestor.
            self.warn_forward_scope_ancestors(&z.fq, &z.segment, z.parent_zone.as_ref(), "zone");
        }
        // No backward check needed when registering a Zone: a Zone is a
        // legal parent for both Scopes and Zones, so any pre-existing
        // children that named us as their parent_zone are still legal.
        // Path-prefix backward check is also unnecessary — only Scope
        // ancestors create the violation, and inserting a Zone at this
        // FQM does not introduce a new Scope ancestor for any existing
        // descendant (their existing Scope ancestors, if any, would
        // already have been flagged when those descendants registered).
        let fq = z.fq.clone();
        self.scopes.insert(fq.clone(), RegisteredScope::Zone(z));

        // Same-kind overlap check: a `<FocusZone>` registered at the
        // same rounded `(x, y)` as an existing zone in the same layer
        // is almost always a needless-nesting candidate.
        self.check_overlap_warning("register_zone", &fq);
    }

    /// Forward scope-is-leaf check used by both [`register_scope`] and
    /// [`register_zone`]: if `parent_zone` is `Some(p)` and `p` is
    /// already registered as a leaf [`FocusScope`], the new entry
    /// violates the invariant that scopes are leaves. Emit one
    /// `scope-not-leaf` error and let the insert proceed so the rest of
    /// the registry stays consistent.
    ///
    /// Silent when:
    /// - `parent_zone` is `None` (the new entry sits under the layer root).
    /// - The parent's FQM is not yet registered (deferred to the
    ///   [`warn_existing_children_of_scope`] backward scan that fires
    ///   when the parent eventually registers).
    /// - The parent is a [`FocusZone`] (the legal layout).
    ///
    /// `kind` is the offending child's own kind discriminator (`"scope"`
    /// for [`register_scope`], `"zone"` for [`register_zone`]) — used as
    /// a structured tracing field so log readers can tell whether they
    /// have a misused `<FocusScope>` wrapping a sub-tree of
    /// `<FocusScope>` leaves or a misused `<FocusScope>` enclosing a
    /// nested `<FocusZone>`.
    ///
    /// [`warn_existing_children_of_scope`]: Self::warn_existing_children_of_scope
    fn warn_forward_scope_ancestors(
        &self,
        fq: &FullyQualifiedMoniker,
        segment: &SegmentMoniker,
        parent_zone: Option<&FullyQualifiedMoniker>,
        kind: &'static str,
    ) {
        // Discover all Scope ancestors of the new entry, deduplicated by
        // FQM. Two relations contribute:
        //
        //   - parent-zone: the new entry's `parent_zone` resolves to a
        //     Scope. At most one Scope per offender from this branch
        //     (an entry has exactly one `parent_zone`).
        //   - path-prefix: some registered Scope's FQM is a strict
        //     prefix of the new entry's FQM. Multiple Scopes can match
        //     in pathological registries; in practice there is at most
        //     one (the nearest enclosing Scope).
        //
        // For each unique ancestor Scope FQM, emit exactly one
        // `scope-not-leaf` event, tagging the relation appropriately.
        let mut emitted_for: std::collections::HashSet<FullyQualifiedMoniker> =
            std::collections::HashSet::new();

        // parent-zone branch.
        if let Some(parent_fq) = parent_zone {
            if let Some(RegisteredScope::Scope(parent_scope)) = self.scopes.get(parent_fq) {
                let path_match = is_path_descendant(fq, parent_fq);
                let relation = if path_match { "both" } else { "parent-zone" };
                warn_scope_not_leaf(
                    fq,
                    segment,
                    parent_fq,
                    &parent_scope.segment,
                    kind,
                    /* parent_kind */ "scope",
                    relation,
                );
                emitted_for.insert(parent_fq.clone());
            }
        }

        // path-prefix branch — emit only for Scopes not already covered
        // by the parent-zone branch above.
        for entry in self.scopes.values() {
            let RegisteredScope::Scope(ancestor) = entry else {
                continue;
            };
            if !is_path_descendant(fq, &ancestor.fq) {
                continue;
            }
            if emitted_for.contains(&ancestor.fq) {
                continue;
            }
            warn_scope_not_leaf(
                fq,
                segment,
                &ancestor.fq,
                &ancestor.segment,
                kind,
                /* parent_kind */ "scope",
                "path-prefix",
            );
            emitted_for.insert(ancestor.fq.clone());
        }
    }

    /// Backward scope-is-leaf check used by [`register_scope`] only:
    /// when a new entry is inserted as a leaf [`FocusScope`], any
    /// pre-existing primitive (scope, zone, OR layer) that names this
    /// scope as its `parent_zone` **or** whose FQM is a strict
    /// path-descendant of this scope's FQM is now retroactively illegal.
    /// Fire one event per offender — deduplicated when both relations
    /// apply to the same descendant — so the invariant is enforced
    /// regardless of registration order.
    ///
    /// Walks both the scopes map and the layers map once per call. The
    /// cost is O(n_scopes + n_layers) per `register_scope`, which is in
    /// line with the existing scan and acceptable given registration
    /// burst frequency. Walking both maps in a single pass keeps the
    /// "exactly one event per structurally novel offending edge"
    /// contract uniform across all three primitive kinds (scope, zone,
    /// layer).
    ///
    /// Layers do not have a `parent_zone` field — their `parent` field
    /// always names another Layer FQM, never a scope/zone FQM — so the
    /// path-prefix relation is the only one that can match a Layer
    /// descendant. The relation field on the emitted event is therefore
    /// always `"path-prefix"` for layer offenders.
    ///
    /// `parent_segment` is supplied by the caller (it owns the
    /// just-inserted scope's segment) so the helper does not have to
    /// re-read its own entry from the map.
    fn warn_backward_scope_descendants(
        &self,
        parent_fq: &FullyQualifiedMoniker,
        parent_segment: &SegmentMoniker,
    ) {
        for entry in self.scopes.values() {
            // Skip self — `is_path_descendant` returns false for self
            // anyway, but the explicit guard keeps the parent-zone arm
            // honest if a future change ever weakens that helper.
            if entry.fq() == parent_fq {
                continue;
            }
            let parent_zone_match = entry.parent_zone() == Some(parent_fq);
            let path_match = is_path_descendant(entry.fq(), parent_fq);
            if !parent_zone_match && !path_match {
                continue;
            }
            let relation = match (parent_zone_match, path_match) {
                (true, true) => "both",
                (true, false) => "parent-zone",
                (false, true) => "path-prefix",
                (false, false) => unreachable!(),
            };
            let kind = if entry.is_zone() { "zone" } else { "scope" };
            warn_scope_not_leaf(
                entry.fq(),
                entry.segment(),
                parent_fq,
                parent_segment,
                kind,
                /* parent_kind */ "scope",
                relation,
            );
        }
        // Layer pass — only the path-prefix relation can match because
        // a Layer's `parent` field always points at another Layer FQM.
        for layer in self.layers.values() {
            if !is_path_descendant(&layer.fq, parent_fq) {
                continue;
            }
            warn_scope_not_leaf(
                &layer.fq,
                &layer.segment,
                parent_fq,
                parent_segment,
                /* kind */ "layer",
                /* parent_kind */ "scope",
                /* relation */ "path-prefix",
            );
        }
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
        self.scopes.remove(fq);
        self.overlap_warn_partner.remove(fq);
    }

    /// Update the bounding rect of a registered scope.
    ///
    /// No-op if the FQM is unknown. Called from the React side via
    /// `spatial_update_rect` when ResizeObserver fires.
    ///
    /// Emits the same-kind overlap `WARN` if the new rect lands the
    /// entry on top of another same-kind entry in the same layer. Per-
    /// key suppression elides re-warnings while the same overlap pair
    /// persists — `update_rect` fires every animation frame during
    /// scroll-tracking, so without the gate every frame would re-emit.
    pub fn update_rect(&mut self, fq: &FullyQualifiedMoniker, rect: Rect) {
        if let Some(scope) = self.scopes.get_mut(fq) {
            *scope.rect_mut() = rect;
        }
        self.check_overlap_warning("update_rect", fq);
    }

    /// Detect a same-kind overlap for the entry at `fq` and emit
    /// `WARN` once per (entry, partner) overlap pair.
    ///
    /// `op` is the caller op tag (`"register_scope"`,
    /// `"register_zone"`, or `"update_rect"`). The entry must already
    /// be inserted in the scopes map — this helper reads the entry's
    /// kind, layer, and rect from the registry to scan for a same-kind
    /// same-(rounded x, y) partner in the same layer (excluding itself).
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
        let entry_is_zone = entry.is_zone();
        let entry_layer = entry.layer_fq().clone();
        let entry_rect = *entry.rect();
        let entry_segment = entry.segment().clone();

        // Scan same-layer entries for a same-kind same-rounded-origin
        // partner, excluding ourselves. Yields the first match's FQM
        // and segment as owned values so we can release the immutable
        // borrow before mutating `overlap_warn_partner`.
        let partner: Option<(FullyQualifiedMoniker, SegmentMoniker)> = self
            .scopes
            .values()
            .filter(|other| other.layer_fq() == &entry_layer)
            .filter(|other| other.is_zone() == entry_is_zone)
            .filter(|other| other.fq() != fq)
            .find(|other| same_rounded_origin(&entry_rect, other.rect()))
            .map(|other| (other.fq().clone(), other.segment().clone()));

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

    /// Borrow a leaf [`FocusScope`] by FQM, or `None` if the FQM is
    /// unregistered or registered as a zone.
    ///
    /// Use [`zone`](Self::zone) to look up zones, [`is_registered`](Self::is_registered)
    /// for variant-blind presence checks.
    pub fn scope(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusScope> {
        self.scopes.get(fq).and_then(RegisteredScope::as_scope)
    }

    /// Borrow a [`FocusZone`] by FQM, or `None` if the FQM is
    /// unregistered or registered as a leaf.
    ///
    /// `last_focused` is populated at registration (the kernel preserves
    /// it across re-registers via [`apply_batch`](Self::apply_batch));
    /// the registry does not mutate it after the fact.
    pub fn zone(&self, fq: &FullyQualifiedMoniker) -> Option<&FocusZone> {
        self.scopes.get(fq).and_then(RegisteredScope::as_zone)
    }

    /// `true` when **any** scope (leaf or zone) is registered under
    /// `fq`. Convenience for callers that don't care which variant —
    /// the navigator uses this to validate a starting FQM before
    /// consulting a strategy.
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
    ///
    /// Returns a public [`FocusEntry`] view — a variant-aware borrow
    /// over the registered scope so callers that need leaf-vs-zone
    /// variant access can branch via
    /// [`FocusEntry::as_scope`](FocusEntry::as_scope) /
    /// [`FocusEntry::as_zone`](FocusEntry::as_zone). For most consumers,
    /// [`scope`](Self::scope) / [`zone`](Self::zone) are the
    /// variant-typed shorthands.
    pub fn find_by_fq(&self, fq: &FullyQualifiedMoniker) -> Option<FocusEntry<'_>> {
        self.scopes.get(fq).map(FocusEntry::from_registered)
    }

    /// Crate-internal accessor returning the discriminated entry directly.
    ///
    /// External callers should use [`find_by_fq`](Self::find_by_fq),
    /// [`scope`](Self::scope), or [`zone`](Self::zone). The
    /// internal navigator and focus-state code pattern-match on the
    /// entry variant; rather than expose that enum publicly (the kernel
    /// has three peers, not four), we keep the match site inside the
    /// crate.
    pub(crate) fn entry(&self, fq: &FullyQualifiedMoniker) -> Option<&RegisteredScope> {
        self.scopes.get(fq)
    }

    /// Iterate every registered [`FocusScope`] leaf in the registry,
    /// regardless of layer.
    pub fn leaves_iter(&self) -> impl Iterator<Item = &FocusScope> + '_ {
        self.scopes.values().filter_map(RegisteredScope::as_scope)
    }

    /// Iterate every registered [`FocusZone`] container in the registry,
    /// regardless of layer.
    pub fn zones_iter(&self) -> impl Iterator<Item = &FocusZone> + '_ {
        self.scopes.values().filter_map(RegisteredScope::as_zone)
    }

    /// Iterate over the direct children of a zone — scopes whose
    /// `parent_zone` equals `zone_fq`.
    ///
    /// Direct children only; grandchildren whose `parent_zone` points at
    /// some other zone are excluded. Yields a small variant-aware view
    /// (`ChildScope::Leaf` or `ChildScope::Zone`) so callers that need
    /// to distinguish leaf vs container do so without pattern-matching
    /// a public enum.
    pub fn children_of_zone(
        &self,
        zone_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = ChildScope<'_>> + '_ {
        let zone_fq = zone_fq.clone();
        self.scopes.values().filter_map(move |s| {
            if s.parent_zone() == Some(&zone_fq) {
                Some(child_scope_from_entry(s))
            } else {
                None
            }
        })
    }

    /// Crate-internal version of [`children_of_zone`](Self::children_of_zone)
    /// that yields the discriminated entry directly. Used by the
    /// navigator and state, which already pattern-match internally.
    pub(crate) fn child_entries_of_zone(
        &self,
        zone_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &RegisteredScope> + '_ {
        let zone_fq = zone_fq.clone();
        self.scopes
            .values()
            .filter(move |s| s.parent_zone() == Some(&zone_fq))
    }

    /// Walk the `parent_zone` chain from the scope at `fq` upward,
    /// collecting each ancestor [`FocusZone`] in innermost-first order.
    ///
    /// The scope at `fq` is **not** included in the result — only its
    /// ancestors. If `fq` is unknown, returns an empty vector. The walk
    /// stops at the first ancestor that is not itself a zone (which
    /// should not happen in a well-formed registry but is handled
    /// defensively rather than panicking).
    pub fn ancestor_zones(&self, fq: &FullyQualifiedMoniker) -> Vec<&FocusZone> {
        let mut chain = Vec::new();
        let Some(start) = self.scopes.get(fq) else {
            return chain;
        };

        let mut next = start.parent_zone().cloned();
        while let Some(parent_fq) = next {
            let Some(parent) = self.scopes.get(&parent_fq) else {
                break;
            };
            let Some(zone) = parent.as_zone() else {
                // A scope's parent_zone always names a Zone; if the
                // registry is in an inconsistent state, stop walking
                // rather than misclassifying the chain.
                break;
            };
            chain.push(zone);
            next = zone.parent_zone.clone();
        }
        chain
    }

    /// Iterate every leaf [`FocusScope`] in `layer_fq`'s layer.
    ///
    /// Used by the navigator when computing beam-search candidate sets
    /// — leaves outside the active layer are filtered out at this
    /// boundary rather than during scoring.
    pub fn leaves_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &FocusScope> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes.values().filter_map(move |s| match s {
            RegisteredScope::Scope(f) if f.layer_fq == layer_fq => Some(f),
            _ => None,
        })
    }

    /// Iterate every [`FocusZone`] in `layer_fq`'s layer.
    pub fn zones_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &FocusZone> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes.values().filter_map(move |s| match s {
            RegisteredScope::Zone(z) if z.layer_fq == layer_fq => Some(z),
            _ => None,
        })
    }

    /// Crate-internal: iterate every entry (leaf or zone) in
    /// `layer_fq`'s layer.
    pub(crate) fn entries_in_layer(
        &self,
        layer_fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &RegisteredScope> + '_ {
        let layer_fq = layer_fq.clone();
        self.scopes
            .values()
            .filter(move |s| s.layer_fq() == &layer_fq)
    }

    // ---------------------------------------------------------------------
    // Drill-in / drill-out — explicit zone descent / ascent
    // ---------------------------------------------------------------------

    /// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
    /// *into* the scope at `fq`.
    ///
    /// The semantics are zone-aware:
    ///
    /// - **Zone with a live `last_focused`** — returns that descendant's
    ///   FQM, restoring the user's last position inside the zone
    ///   across drill-out / drill-in cycles.
    /// - **Zone with a stale or absent `last_focused`** — falls back to
    ///   the first child by rect top-left ordering (topmost wins; ties
    ///   broken by leftmost). Matches `Direction::First` ordering so the
    ///   keyboard model stays consistent.
    /// - **Zone with no children** — returns `focused_fq`. The caller
    ///   compares the result against the focused FQM; equal means "no
    ///   descent happened, fall through to edit / no-op".
    /// - **[`FocusScope`] leaf** — returns `focused_fq`. Leaves do not
    ///   have children to drill into; the React side decides
    ///   separately whether the leaf has an inline-edit affordance.
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

        let Some(zone) = entry.as_zone() else {
            // Leaf focused — leaves have no children to descend into.
            // Semantic "stay put"; React falls through to inline edit.
            return focused_fq.clone();
        };

        // Honor the zone's remembered position when it still resolves to
        // a registered scope. A `last_focused` whose target was since
        // unregistered is treated the same as no memory at all.
        if let Some(remembered) = &zone.last_focused {
            if let Some(remembered_entry) = self.scopes.get(remembered) {
                return remembered_entry.fq().clone();
            }
        }

        // Cold-start fallback: first child by rect top-left. Tie-break on
        // `left()` so two rows at the same `top` produce a deterministic
        // winner. Borrows from the registry; only the chosen FQM is
        // cloned out. When the zone has no children at all, echo the
        // focused FQM so the caller's no-descent fall-through fires.
        self.child_entries_of_zone(&zone.fq)
            .min_by(|a, b| {
                pixels_cmp(a.rect().top(), b.rect().top())
                    .then(pixels_cmp(a.rect().left(), b.rect().left()))
            })
            .map(|s| s.fq().clone())
            .unwrap_or_else(|| focused_fq.clone())
    }

    /// Pick the [`FullyQualifiedMoniker`] to focus when the user drills
    /// *out of* the scope at `fq`.
    ///
    /// Returns the FQM of the scope's `parent_zone`. Works the same for
    /// both [`FocusScope`] leaves and nested [`FocusZone`] containers —
    /// the result is always the enclosing zone, so a repeated drill-out
    /// walks the zone chain toward the layer root.
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
        let Some(parent_zone_fq) = entry.parent_zone() else {
            // Layer-root edge — no enclosing zone. Well-formed; the
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
        parent_entry.fq().clone()
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
    ///
    /// One **scope-is-leaf** check fires here, gated on structural
    /// novelty: if any already-registered [`FocusScope`]'s FQM is a
    /// strict path-prefix of the new layer's FQM, the React tree
    /// composed this layer inside a misused `<FocusScope>`. Fire one
    /// `scope-not-leaf` error per offending Scope ancestor with
    /// `kind = "layer"` and `relation = "path-prefix"`.
    ///
    /// Layers do not have a `parent_zone` field — their `parent` field
    /// always points at another Layer FQM — so the parent-zone branch
    /// of the scope-is-leaf check does not apply to layers; only the
    /// path-prefix branch does.
    ///
    /// Same-shape re-registration is silent. A layer's structural shape
    /// is `(segment, name, parent, window_label)`; `last_focused` is
    /// mutable runtime state and intentionally excluded so a layer that
    /// has acquired focus history is not mis-classified as "novel" on
    /// re-mount. The hot paths that re-push the same layer (StrictMode
    /// double-mount, palette open/close cycles, IPC re-batch) all flow
    /// through here repeatedly; without the gate an already-reported
    /// illegal Layer-under-Scope edge would re-fire `scope-not-leaf` on
    /// every render.
    pub fn push_layer(&mut self, l: FocusLayer) {
        let shape_unchanged = self
            .layers
            .get(&l.fq)
            .map(|existing| same_shape_layer(existing, &l))
            .unwrap_or(false);

        if !shape_unchanged {
            // Forward path-prefix scan: a Layer cannot be composed
            // inside a `<FocusScope>` (Scopes are leaves). Walk the
            // registered Scopes once and emit one event per ancestor
            // Scope FQM that is a strict path-prefix of `l.fq`.
            for entry in self.scopes.values() {
                let RegisteredScope::Scope(ancestor) = entry else {
                    continue;
                };
                if !is_path_descendant(&l.fq, &ancestor.fq) {
                    continue;
                }
                warn_scope_not_leaf(
                    &l.fq,
                    &l.segment,
                    &ancestor.fq,
                    &ancestor.segment,
                    /* kind */ "layer",
                    /* parent_kind */ "scope",
                    /* relation */ "path-prefix",
                );
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
    /// Iteration order is the order of the input vector. Each entry is
    /// validated before mutating any existing scope: if any entry fails
    /// the kind-stability check (an FQM registered as one variant being
    /// re-registered as the other), the call returns
    /// [`BatchRegisterError::KindMismatch`] **without** applying any
    /// part of the batch. Successful batches are atomic at the registry
    /// boundary — observers see all-or-nothing.
    ///
    /// # Errors
    /// - [`BatchRegisterError::KindMismatch`] when an entry's variant
    ///   disagrees with the variant already registered under the same
    ///   FQM. The placeholder/real-mount swap relies on
    ///   `register_scope` and `register_zone` being **idempotent on
    ///   FQM but not silently variant-changing**, so the error surface
    ///   is the kernel's contract enforcement point.
    pub fn apply_batch(&mut self, entries: Vec<RegisterEntry>) -> Result<(), BatchRegisterError> {
        // Validate every entry up front so a partial application cannot
        // leave the registry in a half-applied state. Cheap because the
        // current scope set is read-only here — we only check the variant
        // discriminator.
        for entry in &entries {
            let fq = entry.fq();
            if let Some(existing) = self.scopes.get(fq) {
                let existing_is_zone = existing.is_zone();
                let entry_is_zone = matches!(entry, RegisterEntry::Zone { .. });
                if existing_is_zone != entry_is_zone {
                    return Err(BatchRegisterError::KindMismatch {
                        fq: fq.clone(),
                        existing_kind: if existing_is_zone {
                            ScopeKind::Zone
                        } else {
                            ScopeKind::Scope
                        },
                        requested_kind: if entry_is_zone {
                            ScopeKind::Zone
                        } else {
                            ScopeKind::Scope
                        },
                    });
                }
            }
        }

        // Validation passed — apply each entry. The registry's per-FQM
        // overwrite semantics handle the placeholder→real-mount rect
        // refresh transparently; zones preserve their `last_focused`
        // slot across re-registers (rather than resetting it on every
        // virtualizer pass) so drill-out memory survives the swap.
        for entry in entries {
            match entry {
                RegisterEntry::Scope {
                    fq,
                    segment,
                    rect,
                    layer_fq,
                    parent_zone,
                    overrides,
                } => {
                    self.register_scope(FocusScope {
                        fq,
                        segment,
                        rect,
                        layer_fq,
                        parent_zone,
                        overrides,
                    });
                }
                RegisterEntry::Zone {
                    fq,
                    segment,
                    rect,
                    layer_fq,
                    parent_zone,
                    overrides,
                } => {
                    // Preserve any existing `last_focused` so a real-mount
                    // swap from a placeholder doesn't lose drill-out memory
                    // accumulated while the placeholder was live. New zones
                    // start with `None` as before.
                    let last_focused = self
                        .scopes
                        .get(&fq)
                        .and_then(|s| s.as_zone())
                        .and_then(|z| z.last_focused.clone());
                    self.register_zone(FocusZone {
                        fq,
                        segment,
                        rect,
                        layer_fq,
                        parent_zone,
                        last_focused,
                        overrides,
                    });
                }
            }
        }

        Ok(())
    }
}

/// Public variant-aware view over a registered scope, returned by
/// [`SpatialRegistry::entry_for`] (the canonical FQM-keyed lookup).
///
/// Provides the leaf-vs-container split without exposing the internal
/// [`RegisteredScope`] enum. Most consumers use the variant-typed
/// shorthands [`SpatialRegistry::scope`] / [`SpatialRegistry::zone`];
/// [`FocusEntry`] is for callers that need a uniform handle through
/// which to inspect the FQM, segment, or rect regardless of variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusEntry<'a> {
    /// The entry is a leaf [`FocusScope`].
    Leaf(&'a FocusScope),
    /// The entry is a [`FocusZone`] container.
    Zone(&'a FocusZone),
}

impl<'a> FocusEntry<'a> {
    /// Map an internal [`RegisteredScope`] to the public view.
    pub(crate) fn from_registered(reg: &'a RegisteredScope) -> Self {
        match reg {
            RegisteredScope::Scope(f) => Self::Leaf(f),
            RegisteredScope::Zone(z) => Self::Zone(z),
        }
    }

    /// Canonical FQM of the entry, regardless of variant.
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Leaf(f) => &f.fq,
            Self::Zone(z) => &z.fq,
        }
    }

    /// Relative segment of the entry, regardless of variant. For
    /// human-readable logging.
    pub fn segment(&self) -> &SegmentMoniker {
        match self {
            Self::Leaf(f) => &f.segment,
            Self::Zone(z) => &z.segment,
        }
    }

    /// Owning layer's FQM, regardless of variant.
    pub fn layer_fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Leaf(f) => &f.layer_fq,
            Self::Zone(z) => &z.layer_fq,
        }
    }

    /// Bounding rect of the entry, regardless of variant.
    pub fn rect(&self) -> Rect {
        match self {
            Self::Leaf(f) => f.rect,
            Self::Zone(z) => z.rect,
        }
    }

    /// Borrow the inner [`FocusScope`] when the entry is a leaf, else
    /// `None`. The variant-typed counterpart of
    /// [`SpatialRegistry::scope`](SpatialRegistry::scope) for callers
    /// that already hold a [`FocusEntry`].
    pub fn as_scope(&self) -> Option<&'a FocusScope> {
        match self {
            Self::Leaf(f) => Some(*f),
            Self::Zone(_) => None,
        }
    }

    /// Borrow the inner [`FocusZone`] when the entry is a zone, else
    /// `None`. The variant-typed counterpart of
    /// [`SpatialRegistry::zone`](SpatialRegistry::zone) for callers
    /// that already hold a [`FocusEntry`].
    pub fn as_zone(&self) -> Option<&'a FocusZone> {
        match self {
            Self::Zone(z) => Some(*z),
            Self::Leaf(_) => None,
        }
    }
}

/// Variant-aware view returned by [`SpatialRegistry::children_of_zone`].
///
/// Provides the leaf vs container split without exposing the internal
/// [`RegisteredScope`] enum. Consumers that only need the shared fields
/// (`fq`, `segment`, `rect`, `parent_zone`) can use the accessor methods;
/// consumers that need a typed view of one variant pattern-match on the
/// enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChildScope<'a> {
    /// A leaf [`FocusScope`] child.
    Leaf(&'a FocusScope),
    /// A nested [`FocusZone`] child.
    Zone(&'a FocusZone),
}

impl<'a> ChildScope<'a> {
    /// Canonical FQM of the child, regardless of variant.
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Leaf(f) => &f.fq,
            Self::Zone(z) => &z.fq,
        }
    }

    /// Relative segment of the child, regardless of variant.
    pub fn segment(&self) -> &SegmentMoniker {
        match self {
            Self::Leaf(f) => &f.segment,
            Self::Zone(z) => &z.segment,
        }
    }

    /// Bounding rect of the child, regardless of variant.
    pub fn rect(&self) -> Rect {
        match self {
            Self::Leaf(f) => f.rect,
            Self::Zone(z) => z.rect,
        }
    }
}

/// Adapter from the internal [`RegisteredScope`] enum to the public
/// [`ChildScope`] variant-aware view. Crate-private so the internal
/// enum stays hidden.
fn child_scope_from_entry(entry: &RegisteredScope) -> ChildScope<'_> {
    match entry {
        RegisteredScope::Scope(f) => ChildScope::Leaf(f),
        RegisteredScope::Zone(z) => ChildScope::Zone(z),
    }
}

/// One entry in a batch registration.
///
/// The wire-shape companion to [`FocusScope`] / [`FocusZone`] —
/// reuses the same fields and the same newtypes so the IPC boundary
/// can be a single `Vec<RegisterEntry>` payload. The discriminator
/// uses a `kind` tag with `snake_case` rename so the React side reads
/// the variant the same way it reads other tagged enums in the kernel.
///
/// `last_focused` is intentionally **not** carried on the wire for
/// the `Zone` variant: registration is the React side's "this scope
/// just mounted" signal, and `last_focused` is server-owned drill-out
/// memory that the navigator populates as focus moves. The registry
/// preserves any existing `last_focused` slot when a zone is
/// re-registered (the placeholder/real-mount swap), so the lack of a
/// wire field is correct rather than lossy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegisterEntry {
    /// A leaf focus scope — see [`FocusScope`].
    Scope {
        /// Canonical FQM for this mount.
        fq: FullyQualifiedMoniker,
        /// Relative segment the consumer declared.
        segment: SegmentMoniker,
        /// Bounding rect in viewport coordinates.
        rect: Rect,
        /// Owning layer's FQM.
        layer_fq: FullyQualifiedMoniker,
        /// Immediate enclosing zone's FQM, if any.
        parent_zone: Option<FullyQualifiedMoniker>,
        /// Per-direction overrides.
        overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
    },
    /// A navigable container — see [`FocusZone`].
    Zone {
        /// Canonical FQM for this mount.
        fq: FullyQualifiedMoniker,
        /// Relative segment the consumer declared.
        segment: SegmentMoniker,
        /// Bounding rect in viewport coordinates.
        rect: Rect,
        /// Owning layer's FQM.
        layer_fq: FullyQualifiedMoniker,
        /// Immediate enclosing zone's FQM, if any.
        parent_zone: Option<FullyQualifiedMoniker>,
        /// Per-direction overrides.
        overrides: HashMap<Direction, Option<FullyQualifiedMoniker>>,
    },
}

impl RegisterEntry {
    /// Read the entry's [`FullyQualifiedMoniker`] regardless of variant.
    pub fn fq(&self) -> &FullyQualifiedMoniker {
        match self {
            Self::Scope { fq, .. } | Self::Zone { fq, .. } => fq,
        }
    }
}

/// Discriminator for the [`BatchRegisterError::KindMismatch`] error
/// payload. The variant-on-the-wire `kind` tag in [`RegisterEntry`]
/// uses `snake_case`; this enum is internal to the error surface so
/// it can stay in PascalCase for ergonomic `match` arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Matches [`RegisterEntry::Scope`] — a leaf [`FocusScope`].
    Scope,
    /// Matches [`RegisterEntry::Zone`] — a [`FocusZone`].
    Zone,
}

/// Errors produced by [`SpatialRegistry::apply_batch`].
///
/// The batch entry point validates kind-stability before mutating any
/// scope, so a returned error guarantees the registry is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchRegisterError {
    /// An entry's variant disagrees with the variant already
    /// registered under the same FQM.
    ///
    /// The placeholder/real-mount swap requires that whoever generates
    /// the placeholder uses the same FQM **and** the same kind as the
    /// eventual real mount. A mismatch indicates a bug on the React
    /// side (e.g. a zone placeholder for a card that mounts as a leaf),
    /// which the kernel surfaces rather than silently converting types.
    KindMismatch {
        /// The offending FQM.
        fq: FullyQualifiedMoniker,
        /// Kind currently registered under that FQM.
        existing_kind: ScopeKind,
        /// Kind requested by the entry.
        requested_kind: ScopeKind,
    },
}

impl std::fmt::Display for BatchRegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KindMismatch {
                fq,
                existing_kind,
                requested_kind,
            } => write!(
                f,
                "FQM {fq} already registered as {existing_kind:?}; cannot re-register as {requested_kind:?}",
            ),
        }
    }
}

impl std::error::Error for BatchRegisterError {}

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

    fn focus_scope(fq: &str, layer: &str, parent_zone: Option<&str>) -> FocusScope {
        FocusScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            rect: rect(),
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            overrides: HashMap::new(),
        }
    }

    fn zone(fq: &str, layer: &str, parent_zone: Option<&str>) -> FocusZone {
        FocusZone {
            fq: FullyQualifiedMoniker::from_string(fq),
            segment: SegmentMoniker::from_string(fq.rsplit('/').next().unwrap_or(fq)),
            rect: rect(),
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            last_focused: None,
            overrides: HashMap::new(),
        }
    }

    fn layer(fq: &str, window: &str, parent: Option<&str>) -> FocusLayer {
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
        reg.register_scope(focus_scope("/L/k", "/L", None));
        assert!(reg
            .scope(&FullyQualifiedMoniker::from_string("/L/k"))
            .is_some());
    }

    #[test]
    fn ancestor_zones_walks_chain() {
        let mut reg = SpatialRegistry::new();
        reg.register_zone(zone("/L/outer", "/L", None));
        reg.register_zone(zone("/L/outer/inner", "/L", Some("/L/outer")));
        reg.register_scope(focus_scope(
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
    fn root_for_window_finds_window_root() {
        let mut reg = SpatialRegistry::new();
        reg.push_layer(layer("/win-a", "win-a", None));
        reg.push_layer(layer("/win-a/ins", "win-a", Some("/win-a")));

        let root = reg
            .root_for_window(&WindowLabel::from_string("win-a"))
            .unwrap();
        assert_eq!(root.fq, FullyQualifiedMoniker::from_string("/win-a"));
    }
}
