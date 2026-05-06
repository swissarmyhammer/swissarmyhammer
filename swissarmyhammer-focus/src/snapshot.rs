//! Per-decision navigation snapshots — the wire shape the kernel reads
//! at decision time, plus the read-only index helper that pathfinding
//! and fallback resolution will use.
//!
//! # What this module is, and why
//!
//! Step 2 of the spatial-nav redesign described in card
//! `01KQTC1VNQM9KC90S65P7QX9N1`. The redesign moves the scope registry
//! out of the kernel and into the React side; the kernel becomes
//! stateless with respect to scope geometry and structure, and reads a
//! fresh [`NavSnapshot`] per decision instead. This file defines the
//! Rust mirror of the TypeScript wire shape introduced by step 1
//! (`kanban-app/ui/src/lib/layer-scope-registry-context.tsx`) — the
//! frontend already builds these payloads, but no IPC commands carry
//! them yet, so this module has no production callers in step 2. That
//! is intentional: steps 3–5 will adapt `geometric_pick`,
//! `resolve_fallback`, and `record_focus` to take a snapshot argument,
//! and steps 6–8 will plumb the IPC.
//!
//! # Field-name mirror with TypeScript
//!
//! The struct fields use snake_case names that match the TS interfaces
//! verbatim (`layer_fq`, `parent_zone`, `nav_override`, `fq`, `rect`).
//! Both sides agree on the JSON shape so a snapshot built in React can
//! be deserialized into [`NavSnapshot`] without wrapper renames.
//!
//! # Index helper
//!
//! Pathfinding and fallback walk a snapshot many times per decision —
//! once for the candidate set, repeatedly for the parent-zone chain.
//! [`IndexedSnapshot`] builds a one-time `HashMap<FQ, &SnapshotScope>`
//! over the snapshot so each lookup is O(1) and the walks are linear in
//! chain length, not in scope count.
//!
//! # Cycle-guarding
//!
//! [`IndexedSnapshot::parent_zone_chain`] walks ancestors via each
//! entry's `parent_zone`. The walk degrades gracefully on torn input
//! (an FQM whose `parent_zone` names a missing entry) by stopping at
//! the missing edge, and on a malformed cycle by emitting
//! `tracing::error!` and breaking — matching the same defensive
//! pattern that `SpatialRegistry::record_focus` uses when walking
//! `FocusScope::parent_zone` chains today. The kernel does not produce
//! cycles in well-formed code, but the guard keeps a malformed React-
//! side payload from freezing a focus IPC under the registry mutex.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::types::{Direction, FullyQualifiedMoniker, Rect};

/// Per-direction navigation overrides for a single scope.
///
/// Mirrors the React `FocusOverrides` type (a partial map from
/// `Direction` to a target FQM or `null`). The same shape is used
/// inline today on [`crate::scope::FocusScope::overrides`]; this alias
/// gives the snapshot wire format a stable name for the type and
/// matches the TypeScript shape one-for-one.
///
/// Semantics follow the existing `overrides` field on `FocusScope`:
///
/// - missing key — fall through to beam search;
/// - `Some(target_fq)` — redirect navigation to the named target;
/// - `None` — explicit "wall" that blocks navigation in that direction.
pub type FocusOverrides = HashMap<Direction, Option<FullyQualifiedMoniker>>;

/// A single scope's contribution to a navigation snapshot.
///
/// Mirrors the TypeScript `SnapshotScope` introduced by step 1 of the
/// parent card. Fields are snake_case so a JSON payload built on the
/// React side deserializes verbatim.
///
/// Construction is plain struct-literal — there is no validating
/// constructor because (a) the only producer is the React-side
/// snapshot builder and (b) the kernel walks snapshot scopes
/// defensively, tolerating torn input via the cycle-guards in
/// [`IndexedSnapshot::parent_zone_chain`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotScope {
    /// Canonical fully-qualified path to this scope. Acts as the
    /// snapshot's primary key — duplicate FQMs in a single snapshot
    /// are a programmer error on the React side; the index helper
    /// silently keeps the last one wins (matching `HashMap` insert
    /// semantics).
    pub fq: FullyQualifiedMoniker,
    /// Bounding rect in viewport pixel coordinates, read at snapshot
    /// time via `getBoundingClientRect` on the React side. Drives
    /// beam-search distance and overlap math.
    pub rect: Rect,
    /// FQM of the immediate enclosing scope or zone, or `None` when
    /// this scope is registered directly under the layer root.
    /// `resolve_fallback` walks this chain to locate sibling-in-zone
    /// and parent-zone fall-back targets.
    pub parent_zone: Option<FullyQualifiedMoniker>,
    /// Per-direction navigation overrides for this scope. Mirrors the
    /// existing `FocusScope::overrides` shape; the redesign reads it
    /// at decision time so mid-life changes to the React-side
    /// `navOverride` prop take effect on the next nav (the previous
    /// register-time snapshot semantics are explicitly improved
    /// here).
    pub nav_override: FocusOverrides,
}

/// A snapshot of every scope mounted under a single layer.
///
/// Mirrors the TypeScript `NavSnapshot` introduced by step 1. Built
/// per decision (per-nav, per-focus, per-focus-lost) on the React
/// side and shipped to the kernel inline so the kernel never has to
/// read scope state out-of-band.
///
/// The `scopes` vector is treated as flat — pathfinding and fallback
/// wrap the snapshot in [`IndexedSnapshot`] for O(1) FQM lookups when
/// they need to walk parent-zone chains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavSnapshot {
    /// FQM of the layer this snapshot describes. Snapshots are
    /// layer-scoped because the kernel's pathfinding and fallback
    /// algorithms never cross a layer boundary.
    pub layer_fq: FullyQualifiedMoniker,
    /// All scopes registered in the layer at snapshot time.
    pub scopes: Vec<SnapshotScope>,
}

/// Read-only index over a [`NavSnapshot`].
///
/// Pathfinding and fallback resolution call into a snapshot many
/// times per decision — once to enumerate candidates, repeatedly to
/// walk an FQM's parent-zone chain. This wrapper builds a single
/// `HashMap<FQ, &SnapshotScope>` over the snapshot so each lookup is
/// O(1) and parent-zone walks are linear in chain length, not in
/// scope count.
///
/// The wrapper borrows from the underlying [`NavSnapshot`] — no data
/// is copied. Callers are expected to build an `IndexedSnapshot` once
/// per decision and pass it down by reference.
#[derive(Debug)]
pub struct IndexedSnapshot<'a> {
    snapshot: &'a NavSnapshot,
    by_fq: HashMap<FullyQualifiedMoniker, &'a SnapshotScope>,
}

impl<'a> IndexedSnapshot<'a> {
    /// Build an index over the given snapshot.
    ///
    /// Walks the snapshot's `scopes` once and inserts each entry into
    /// the lookup map keyed by FQM. Duplicate FQMs in the input are a
    /// React-side programmer mistake; this constructor follows the
    /// same "last write wins" convention `HashMap::insert` uses, so
    /// the kernel sees one canonical entry per FQM regardless of how
    /// torn the upstream payload is.
    pub fn new(snapshot: &'a NavSnapshot) -> Self {
        let mut by_fq = HashMap::with_capacity(snapshot.scopes.len());
        for scope in &snapshot.scopes {
            by_fq.insert(scope.fq.clone(), scope);
        }
        Self { snapshot, by_fq }
    }

    /// Look up a scope by its fully-qualified path. Returns `None`
    /// when `fq` is not present in the snapshot — a rare race during
    /// animation where a scope was removed between snapshot build
    /// and consumption, or a torn React-side payload.
    pub fn get(&self, fq: &FullyQualifiedMoniker) -> Option<&SnapshotScope> {
        self.by_fq.get(fq).copied()
    }

    /// FQM of the layer this snapshot describes. Convenience accessor
    /// so callers don't have to reach through to the wrapped
    /// [`NavSnapshot`].
    pub fn layer_fq(&self) -> &FullyQualifiedMoniker {
        &self.snapshot.layer_fq
    }

    /// All scopes in the snapshot, in the original insertion order.
    /// Convenience accessor for callers that need the flat list (e.g.
    /// the geometric-pick candidate enumeration).
    pub fn scopes(&self) -> &[SnapshotScope] {
        &self.snapshot.scopes
    }

    /// Walk the ancestor chain for `fq`, yielding each
    /// [`SnapshotScope`] whose FQM appears as a `parent_zone` link
    /// from `fq` up toward the layer root.
    ///
    /// The first item yielded is the `parent_zone` of `fq` itself
    /// (not `fq`'s own scope); subsequent items are the
    /// `parent_zone` of the previous one. The walk stops cleanly
    /// when it reaches:
    ///
    /// - a scope whose `parent_zone` is `None` (`fq` was registered
    ///   directly under the layer root, or the chain has reached the
    ///   topmost zone) — the `None`-entry's scope IS yielded; only
    ///   the lookup that follows it is skipped;
    /// - an FQM that is not present in the snapshot — torn input;
    /// - an FQM that has already been visited — a malformed cycle.
    ///   The cycle-break path emits `tracing::error!` matching the
    ///   shape used by [`crate::registry::SpatialRegistry::record_focus`].
    ///
    /// The starting `fq` is NOT yielded — only its ancestors are. If
    /// `fq` itself is missing from the snapshot, the iterator is
    /// empty.
    pub fn parent_zone_chain(
        &self,
        fq: &FullyQualifiedMoniker,
    ) -> impl Iterator<Item = &'a SnapshotScope> + '_ {
        let start = self
            .by_fq
            .get(fq)
            .and_then(|scope| scope.parent_zone.clone());
        ParentZoneChain {
            index: self,
            next: start,
            visited: HashSet::new(),
        }
    }
}

/// Lazy iterator over a snapshot's ancestor chain.
///
/// Constructed by [`IndexedSnapshot::parent_zone_chain`]; not part of
/// the public surface. Holds the next FQM to visit (cloned out of the
/// snapshot so the iterator does not borrow the entry it is about to
/// yield) plus a cycle-guard set keyed by FQM.
struct ParentZoneChain<'a, 'idx> {
    /// The index this iterator walks against. Borrows for `'idx`,
    /// yielded references for `'a` (the snapshot's lifetime).
    index: &'idx IndexedSnapshot<'a>,
    /// FQM of the next ancestor to visit, or `None` when the walk is
    /// complete (parent-zone reached the layer root, encountered a
    /// missing FQM, or hit a cycle).
    next: Option<FullyQualifiedMoniker>,
    /// FQMs already yielded from this iterator. Used to break cleanly
    /// on a malformed cycle (a `parent_zone` chain that revisits an
    /// earlier ancestor) so the walk cannot run forever under a
    /// torn React-side payload.
    visited: HashSet<FullyQualifiedMoniker>,
}

impl<'a, 'idx> Iterator for ParentZoneChain<'a, 'idx> {
    type Item = &'a SnapshotScope;

    /// Advance the chain by one ancestor.
    ///
    /// On each call we (a) read the queued FQM, (b) look it up in
    /// the index, (c) record it in the cycle-guard, and (d) advance
    /// `next` to the entry's own `parent_zone`. Any of those steps
    /// failing — missing FQM, cycle revisit — terminates the walk
    /// by clearing `next`.
    fn next(&mut self) -> Option<Self::Item> {
        let current_fq = self.next.take()?;

        if !self.visited.insert(current_fq.clone()) {
            // Cycle: an ancestor's `parent_zone` named an FQM we
            // already yielded. Log and stop. Matches the shape used
            // by `SpatialRegistry::record_focus` so log scrapers can
            // grep both code paths the same way.
            tracing::error!(
                op = "parent_zone_chain",
                cycle_fq = %current_fq,
                "parent_zone chain cycle detected; breaking walk"
            );
            return None;
        }

        let Some(scope) = self.index.by_fq.get(&current_fq).copied() else {
            // Torn snapshot — `parent_zone` named an FQM with no
            // entry. Stop the walk. Callers that care about
            // structural integrity log elsewhere; we degrade
            // gracefully here so the kernel does not freeze under a
            // bad payload.
            return None;
        };

        // Queue the next ancestor before yielding. `None` here means
        // we have reached the topmost zone — the current scope IS
        // yielded, but no further lookup follows.
        self.next = scope.parent_zone.clone();
        Some(scope)
    }
}

#[cfg(test)]
mod tests {
    //! Unit coverage for the snapshot data types and the
    //! [`IndexedSnapshot`] walks. Tests deliberately stay small and
    //! focused — no production callers exist yet (steps 3–5 will
    //! introduce them) so the bar is "the helpers behave as
    //! documented", not "every kernel decision is covered".

    use super::*;
    use crate::types::{Pixels, SegmentMoniker};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tracing::{
        field::{Field, Visit},
        span::Attributes,
        Event, Id, Level, Subscriber,
    };
    use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

    // -----------------------------------------------------------------
    // Construction helpers
    // -----------------------------------------------------------------

    /// Build a [`Rect`] from raw `f64` corners. Tests exercise the
    /// snapshot helpers structurally, so concrete pixel values are
    /// unimportant — every test uses the same throwaway origin rect.
    fn rect_zero() -> Rect {
        Rect {
            x: Pixels::new(0.0),
            y: Pixels::new(0.0),
            width: Pixels::new(10.0),
            height: Pixels::new(10.0),
        }
    }

    /// Build a [`SnapshotScope`] from its FQM, parent-zone FQM, and
    /// the zero rect. Used by every test that constructs a snapshot
    /// by hand — keeps the common shape out of the test bodies.
    fn scope(fq: &str, parent_zone: Option<&str>) -> SnapshotScope {
        SnapshotScope {
            fq: FullyQualifiedMoniker::from_string(fq),
            rect: rect_zero(),
            parent_zone: parent_zone.map(FullyQualifiedMoniker::from_string),
            nav_override: HashMap::new(),
        }
    }

    /// Build a [`NavSnapshot`] for layer `/L` with the given scopes.
    fn snapshot_for(layer: &str, scopes: Vec<SnapshotScope>) -> NavSnapshot {
        NavSnapshot {
            layer_fq: FullyQualifiedMoniker::from_string(layer),
            scopes,
        }
    }

    // -----------------------------------------------------------------
    // Tracing capture — local copy of the helper used elsewhere in
    // the crate, so the cycle-break test can assert that the right
    // event landed at error level.
    // -----------------------------------------------------------------

    #[derive(Debug, Default, Clone)]
    struct CapturedEvent {
        level: Option<Level>,
        message: String,
        fields: HashMap<String, String>,
    }

    impl CapturedEvent {
        fn op(&self) -> Option<&str> {
            self.fields.get("op").map(String::as_str)
        }
        fn field(&self, name: &str) -> Option<&str> {
            self.fields.get(name).map(String::as_str)
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
    /// events and return them alongside `f`'s result. Mirrors the
    /// helper in `registry.rs::tests::capture` so the cycle-break
    /// test can assert the right event landed.
    fn capture<F, R>(f: F) -> (R, Vec<CapturedEvent>)
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

    // -----------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------

    /// `NavSnapshot` round-trips through `serde_json` byte-identically
    /// for at least one populated payload. The wire shape is the
    /// kernel↔React contract — any breaking change here would also
    /// break the TypeScript mirror in step 1.
    #[test]
    fn nav_snapshot_round_trips_through_serde() {
        let mut overrides: FocusOverrides = HashMap::new();
        overrides.insert(
            Direction::Left,
            Some(FullyQualifiedMoniker::from_string("/L/redirect")),
        );
        overrides.insert(Direction::Right, None);

        let snapshot = NavSnapshot {
            layer_fq: FullyQualifiedMoniker::from_string("/L"),
            scopes: vec![
                SnapshotScope {
                    fq: FullyQualifiedMoniker::from_string("/L/zone"),
                    rect: Rect {
                        x: Pixels::new(1.0),
                        y: Pixels::new(2.0),
                        width: Pixels::new(3.0),
                        height: Pixels::new(4.0),
                    },
                    parent_zone: None,
                    nav_override: HashMap::new(),
                },
                SnapshotScope {
                    fq: FullyQualifiedMoniker::from_string("/L/zone/leaf"),
                    rect: Rect {
                        x: Pixels::new(5.0),
                        y: Pixels::new(6.0),
                        width: Pixels::new(7.0),
                        height: Pixels::new(8.0),
                    },
                    parent_zone: Some(FullyQualifiedMoniker::from_string("/L/zone")),
                    nav_override: overrides,
                },
            ],
        };

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: NavSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, snapshot);
    }

    /// The on-wire JSON keys are the snake_case names the TypeScript
    /// step-1 builder produces (`layer_fq`, `parent_zone`,
    /// `nav_override`). Pinning the field names here catches a future
    /// rename that would silently break the React-side payload.
    #[test]
    fn nav_snapshot_json_uses_snake_case_field_names() {
        let snapshot = snapshot_for("/L", vec![scope("/L/zone", None)]);
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"layer_fq\":"), "json missing layer_fq: {json}");
        assert!(json.contains("\"parent_zone\":"), "json missing parent_zone: {json}");
        assert!(json.contains("\"nav_override\":"), "json missing nav_override: {json}");
    }

    /// `IndexedSnapshot::get` returns the scope whose FQM matches and
    /// `None` otherwise. Sanity-checks the lookup map the indexer
    /// builds in `new`.
    #[test]
    fn indexed_snapshot_get_returns_matching_scope() {
        let snap = snapshot_for(
            "/L",
            vec![scope("/L/a", None), scope("/L/b", Some("/L/a"))],
        );
        let idx = IndexedSnapshot::new(&snap);

        let a = idx
            .get(&FullyQualifiedMoniker::from_string("/L/a"))
            .expect("expected /L/a in index");
        assert_eq!(a.fq, FullyQualifiedMoniker::from_string("/L/a"));
        assert_eq!(a.parent_zone, None);

        let b = idx
            .get(&FullyQualifiedMoniker::from_string("/L/b"))
            .expect("expected /L/b in index");
        assert_eq!(
            b.parent_zone,
            Some(FullyQualifiedMoniker::from_string("/L/a"))
        );

        assert!(idx
            .get(&FullyQualifiedMoniker::from_string("/L/missing"))
            .is_none());
    }

    /// `layer_fq` and `scopes` accessors expose the wrapped snapshot
    /// without copying — the index borrows from it.
    #[test]
    fn indexed_snapshot_accessors_expose_wrapped_snapshot() {
        let snap = snapshot_for("/L", vec![scope("/L/a", None)]);
        let idx = IndexedSnapshot::new(&snap);

        assert_eq!(idx.layer_fq().as_str(), "/L");
        assert_eq!(idx.scopes().len(), 1);
        assert_eq!(idx.scopes()[0].fq.as_str(), "/L/a");
    }

    /// `parent_zone_chain` walks every ancestor in order across a
    /// 3-level chain (leaf → zone → layer-root scope) and stops at
    /// the topmost ancestor's `parent_zone == None`. The starting
    /// FQM is NOT yielded — only its ancestors are.
    #[test]
    fn parent_zone_chain_walks_three_level_chain() {
        let snap = snapshot_for(
            "/L",
            vec![
                scope("/L/root", None),
                scope("/L/root/zone", Some("/L/root")),
                scope("/L/root/zone/leaf", Some("/L/root/zone")),
            ],
        );
        let idx = IndexedSnapshot::new(&snap);

        let walked: Vec<_> = idx
            .parent_zone_chain(&FullyQualifiedMoniker::from_string("/L/root/zone/leaf"))
            .map(|s| s.fq.as_str().to_string())
            .collect();
        assert_eq!(walked, vec!["/L/root/zone".to_string(), "/L/root".to_string()]);
    }

    /// When the starting FQM is not in the snapshot the iterator is
    /// empty — no panic, no log, just an early termination. Step 3+
    /// callers can rely on this when validating against a torn
    /// payload.
    #[test]
    fn parent_zone_chain_empty_for_missing_fq() {
        let snap = snapshot_for("/L", vec![scope("/L/a", None)]);
        let idx = IndexedSnapshot::new(&snap);

        let walked: Vec<_> = idx
            .parent_zone_chain(&FullyQualifiedMoniker::from_string("/L/missing"))
            .collect();
        assert!(walked.is_empty(), "expected empty chain, got {walked:?}");
    }

    /// On a synthetic cycle (`a.parent_zone = b`, `b.parent_zone = a`)
    /// the walk yields each entry exactly once, then breaks with a
    /// `tracing::error!` event tagged `op = "parent_zone_chain"` and
    /// the cycle FQM. Without the guard the walk would loop
    /// indefinitely under a torn React-side payload.
    #[test]
    fn parent_zone_chain_breaks_on_cycle_and_logs_error() {
        let snap = snapshot_for(
            "/L",
            vec![
                scope("/L/a", Some("/L/b")),
                scope("/L/b", Some("/L/a")),
            ],
        );

        let (walked, captured) = capture(|| {
            let idx = IndexedSnapshot::new(&snap);
            idx.parent_zone_chain(&FullyQualifiedMoniker::from_string("/L/a"))
                .map(|s| s.fq.as_str().to_string())
                .collect::<Vec<_>>()
        });

        // Walk visits b, then a (the cycle revisit), then breaks.
        // The cycle revisit is NOT yielded — the guard catches it
        // before the lookup, so the walk yields `b` once and stops.
        assert_eq!(walked, vec!["/L/b".to_string()]);

        let cycle_events: Vec<_> = captured
            .iter()
            .filter(|e| e.op() == Some("parent_zone_chain"))
            .collect();
        assert_eq!(
            cycle_events.len(),
            1,
            "expected one cycle event, got {captured:?}"
        );
        assert_eq!(cycle_events[0].level, Some(Level::ERROR));
        assert_eq!(cycle_events[0].field("cycle_fq"), Some("/L/a"));
        assert!(
            cycle_events[0]
                .message
                .contains("parent_zone chain cycle detected"),
            "unexpected message: {:?}",
            cycle_events[0].message
        );
    }

    /// The walk stops cleanly at the first ancestor whose
    /// `parent_zone` is `None` — that ancestor IS yielded (it is a
    /// real entry on the chain) but no further lookup follows. This
    /// is how a leaf reaches the layer-root scope and stops without
    /// trying to chase a non-existent grandparent.
    #[test]
    fn parent_zone_chain_stops_at_first_none_parent_zone() {
        let snap = snapshot_for(
            "/L",
            vec![
                scope("/L/root", None),
                scope("/L/root/leaf", Some("/L/root")),
            ],
        );
        let idx = IndexedSnapshot::new(&snap);

        let walked: Vec<_> = idx
            .parent_zone_chain(&FullyQualifiedMoniker::from_string("/L/root/leaf"))
            .map(|s| s.fq.as_str().to_string())
            .collect();
        assert_eq!(walked, vec!["/L/root".to_string()]);
    }

    /// `IndexedSnapshot::new` is robust to a snapshot whose `scopes`
    /// is empty — the index is empty too and every lookup returns
    /// `None`. This is the boring base case but pins the contract so
    /// step-3 callers don't have to special-case empty input.
    #[test]
    fn indexed_snapshot_handles_empty_scopes() {
        let snap = snapshot_for("/L", vec![]);
        let idx = IndexedSnapshot::new(&snap);

        assert_eq!(idx.scopes().len(), 0);
        assert!(idx
            .get(&FullyQualifiedMoniker::from_string("/L/anything"))
            .is_none());
        assert_eq!(
            idx.parent_zone_chain(&FullyQualifiedMoniker::from_string("/L/anything"))
                .count(),
            0
        );
    }

    /// `SegmentMoniker` is referenced by the test imports for clarity
    /// even though no test currently constructs one — pinning the
    /// import here keeps the typecheck honest if the helpers grow a
    /// segment-keyed accessor later. (This `_` binding does not
    /// suppress the unused-import lint by itself; the call below
    /// does.)
    #[test]
    fn _segment_moniker_type_is_in_scope() {
        let _ = SegmentMoniker::from_string("noop");
    }
}
