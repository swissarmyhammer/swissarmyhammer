//! Post-mutation re-enrichment fan-out for tasks with stale computed fields.
//!
//! When a task's `position_column` or `depends_on` field changes on disk, the
//! watcher emits an `EntityFieldChanged` event for that task only. But its
//! computed fields (BLOCKED/READY/BLOCKING virtual tags, `ready` flag,
//! `blocked_by`, `blocks`) depend on cross-entity state — moving task A into
//! the terminal column changes whether task B (which depends on A) is BLOCKED,
//! even though B's own file is unchanged.
//!
//! This module provides the fan-out logic that:
//! 1. Identifies "trigger" tasks whose change may have invalidated other
//!    tasks' computed fields.
//! 2. Walks the dependency graph (forward and reverse) to find affected tasks.
//! 3. Re-runs `enrich_task_entity` on each affected task.
//! 4. Diffs against a per-board cache of last-emitted enriched values, so
//!    re-enrichment that produces identical values does NOT emit an event.
//!
//! Architecture rule (event-architecture): synthetic events emitted from
//! fan-out carry only the changed computed fields, never raw on-disk fields.
//! Each event remains a thin field-level patch, not a full entity refresh.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_kanban::task_helpers::{enrich_task_entity, find_dependent_task_ids};
use swissarmyhammer_kanban::virtual_tags::VirtualTagRegistry;

use crate::watcher::{FieldChange, WatchEvent};

/// Names of the five computed task fields that depend on cross-entity state.
///
/// These are the only fields the fan-out pass diffs and emits — raw on-disk
/// fields are owned by the watcher's diff path and never touched here.
pub const COMPUTED_TASK_FIELDS: &[&str] = &[
    "virtual_tags",
    "filter_tags",
    "ready",
    "blocked_by",
    "blocks",
];

/// Snapshot of the most recently emitted enriched task fields plus the raw
/// `depends_on` value at that emission.
///
/// The `fields` map is used to detect whether re-enrichment of a task during
/// the fan-out pass produced values different from what the frontend last
/// saw. The `depends_on` field is used to reconstruct the OLD forward
/// dependency set when the trigger's own `depends_on` changes, so removed
/// dependency targets (tasks the trigger USED to depend on but no longer
/// does) also get fan-out re-enrichment — otherwise their `blocks` list
/// would stay stale.
///
/// Storing JSON values (rather than typed `Vec<String>` / `bool`) keeps the
/// diff agnostic to underlying field type and matches the [`FieldChange`]
/// wire format.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskEnrichmentSnapshot {
    /// Map from computed-field name (one of [`COMPUTED_TASK_FIELDS`]) to its
    /// last-emitted value. Missing entries mean "never emitted".
    pub fields: HashMap<String, Value>,
    /// The task's `depends_on` list at the time of the snapshot. Used by the
    /// fan-out pass to detect removed forward dependencies when the trigger
    /// changes its `depends_on`. Empty when the task had no `depends_on`
    /// field (or an empty one) at snapshot time.
    pub depends_on: Vec<String>,
}

impl TaskEnrichmentSnapshot {
    /// Build a snapshot by reading the five computed fields plus `depends_on`
    /// from a task entity that has already been run through [`enrich_task_entity`].
    pub fn from_entity(entity: &Entity) -> Self {
        let mut fields = HashMap::with_capacity(COMPUTED_TASK_FIELDS.len());
        for name in COMPUTED_TASK_FIELDS {
            if let Some(v) = entity.get(name) {
                fields.insert((*name).to_string(), v.clone());
            }
        }
        Self {
            fields,
            depends_on: entity.get_string_list("depends_on"),
        }
    }

    /// Return the field changes that differ between this snapshot and `other`.
    ///
    /// Each entry is a (field_name, new_value) drawn from `other`. Used by the
    /// fan-out pass to assemble synthetic [`WatchEvent::EntityFieldChanged`]
    /// payloads that carry only the fields whose values actually changed.
    pub fn diff_to(&self, other: &Self) -> Vec<FieldChange> {
        let mut changes = Vec::new();
        for name in COMPUTED_TASK_FIELDS {
            let new_val = match other.fields.get(*name) {
                Some(v) => v,
                None => continue,
            };
            let unchanged = self
                .fields
                .get(*name)
                .map(|old| old == new_val)
                .unwrap_or(false);
            if !unchanged {
                changes.push(FieldChange {
                    field: (*name).to_string(),
                    value: new_val.clone(),
                });
            }
        }
        changes
    }
}

/// Per-board cache of the last enrichment snapshot emitted for each task.
///
/// Wrapped in `Arc<Mutex<…>>` so the cache survives across `flush_and_emit`
/// calls (the `BoardHandle` owns it) and stays correct under the synchronous
/// access pattern used by `enrich_computed_fields`. The cache is NOT
/// pre-populated on board open — the first emission for any task primes it.
pub type EnrichmentCache = Arc<Mutex<HashMap<String, TaskEnrichmentSnapshot>>>;

/// Construct an empty enrichment cache for a fresh `BoardHandle`.
pub fn new_enrichment_cache() -> EnrichmentCache {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Scan a slice of `WatchEvent`s and return the IDs of tasks whose change
/// may have invalidated another task's computed dependency state.
///
/// A task event is a "trigger" when it can flip BLOCKED/READY/BLOCKING on
/// tasks linked via `depends_on`. Three event shapes qualify:
///
/// - **`EntityFieldChanged`** for a task where the `changes` list touches
///   `position_column` or `depends_on`. A column move can flip BLOCKED on
///   reverse dependents; a `depends_on` edit shifts which forward/reverse
///   tasks need re-enrichment.
/// - **`EntityCreated`** for a task. The new entity's own computed fields are
///   handled by the primary loop, but if the create payload carries a
///   non-empty `depends_on`, the tasks it points at gain a new reverse
///   dependent (their `blocks` list grows, BLOCKING may flip). Including the
///   new task as a trigger lets [`compute_fanout_targets`] discover those
///   forward targets through the normal path — current_deps from `all_tasks`
///   covers the newly created entity.
/// - **`EntityRemoved`** for a task. The deleted task is no longer in
///   `all_tasks`, so reverse dependents need re-enrichment (they may lose
///   BLOCKED) and the cache entry should be pruned (see
///   [`collect_removed_task_ids`] / [`prune_cache_for_removed`]). Including
///   the deleted ID as a trigger routes its reverse lookup through
///   [`compute_fanout_targets`]; forward dependents come from the cached
///   previous `depends_on` via [`snapshot_previous_depends_on`].
///
/// These IDs are the "triggers" for the fan-out pass — any task they depend
/// on, or any task that depends on them, may have stale BLOCKED/READY/BLOCKING
/// computed fields and needs re-enrichment.
pub fn collect_trigger_task_ids(events: &[WatchEvent]) -> HashSet<String> {
    let mut triggers = HashSet::new();
    for evt in events {
        match evt {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
            } if entity_type == "task" => {
                let touches_graph = changes
                    .iter()
                    .any(|c| c.field == "position_column" || c.field == "depends_on");
                if touches_graph {
                    triggers.insert(id.clone());
                }
            }
            WatchEvent::EntityCreated {
                entity_type, id, ..
            } if entity_type == "task" => {
                // Every new task potentially affects the graph: its reverse
                // dependents (rare at create time) via find_dependent_task_ids,
                // and its forward dependents via current_deps in
                // compute_fanout_targets. Inclusion is unconditional — the
                // diff-against-cache guard in fan_out_synthetic_events drops
                // any re-enrichment that produces no real change.
                triggers.insert(id.clone());
            }
            WatchEvent::EntityRemoved { entity_type, id } if entity_type == "task" => {
                // Deletion affects both directions: reverse dependents may
                // lose BLOCKED (the blocker vanished), and forward
                // dependents lose a reverse pointer. The previous depends_on
                // list needed for forward fan-out comes from the cache
                // snapshot, which survives until prune_cache_for_removed runs.
                triggers.insert(id.clone());
            }
            _ => {}
        }
    }
    triggers
}

/// Collect the IDs of every task whose entity was removed in this event batch.
///
/// The fan-out wrapper uses this set to prune stale entries from the
/// enrichment cache after [`snapshot_previous_depends_on`] has captured
/// whatever forward-dependency info the fan-out pass needs. Without pruning,
/// the cache would grow unboundedly with every deleted task.
pub fn collect_removed_task_ids(events: &[WatchEvent]) -> HashSet<String> {
    let mut removed = HashSet::new();
    for evt in events {
        if let WatchEvent::EntityRemoved { entity_type, id } = evt {
            if entity_type == "task" {
                removed.insert(id.clone());
            }
        }
    }
    removed
}

/// Remove cache entries for task IDs whose entities were deleted.
///
/// Call this AFTER [`snapshot_previous_depends_on`] so the forward-dependency
/// lookup for removed triggers still sees the pre-deletion state, and AFTER
/// the fan-out pass itself so a removed task's own (now meaningless) cache
/// entry doesn't leak. Safe to call with an empty set — that's a no-op.
pub fn prune_cache_for_removed(cache: &EnrichmentCache, removed_ids: &HashSet<String>) {
    if removed_ids.is_empty() {
        return;
    }
    let mut guard = cache.lock().unwrap();
    for id in removed_ids {
        guard.remove(id);
    }
}

/// Capture the `depends_on` list cached for each trigger task BEFORE the
/// primary enrichment loop overwrites it.
///
/// Used by `compute_fanout_targets` to detect removed forward dependencies —
/// task IDs that were in a trigger's `depends_on` previously but are no
/// longer there. Returns an empty Vec for triggers with no cache entry
/// (first emission) so callers see a uniform "no previous value" shape.
pub fn snapshot_previous_depends_on(
    triggers: &HashSet<String>,
    cache: &EnrichmentCache,
) -> HashMap<String, Vec<String>> {
    let guard = cache.lock().unwrap();
    triggers
        .iter()
        .map(|id| {
            let prev = guard
                .get(id)
                .map(|snap| snap.depends_on.clone())
                .unwrap_or_default();
            (id.clone(), prev)
        })
        .collect()
}

/// Compute the union of fan-out task IDs reachable from the trigger set.
///
/// For each trigger ID:
/// - **Reverse**: any task whose `depends_on` currently includes the trigger
///   (its `ready` / `blocked_by` may flip, and the BLOCKED virtual tag with it).
/// - **Forward (new)**: any task ID in the trigger's current `depends_on` (its
///   `blocks` list and BLOCKING virtual tag may change).
/// - **Forward (removed)**: any task ID that appeared in
///   `previous_depends_on[trigger]` but not the current one. These are tasks
///   the trigger no longer depends on — their `blocks` list and BLOCKING
///   virtual tag would otherwise stay stale.
///
/// Trigger IDs themselves are excluded from the returned set — those are
/// handled by the primary enrichment loop in `enrich_computed_fields`.
///
/// `previous_depends_on` should be the result of
/// [`snapshot_previous_depends_on`] called BEFORE the primary loop updates
/// the cache — once the cache reflects the post-mutation state, the "old
/// forward" case is no longer detectable.
pub fn compute_fanout_targets(
    triggers: &HashSet<String>,
    all_tasks: &[Entity],
    previous_depends_on: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut targets: HashSet<String> = HashSet::new();
    for trigger_id in triggers {
        // Reverse: tasks that depend on the trigger.
        for dep_id in find_dependent_task_ids(trigger_id, all_tasks) {
            if !triggers.contains(&dep_id) {
                targets.insert(dep_id);
            }
        }
        // Forward (new): tasks the trigger currently depends on.
        let current_deps: Vec<String> = if let Some(trigger) = all_tasks
            .iter()
            .find(|t| t.id.as_ref() == trigger_id.as_str())
        {
            trigger.get_string_list("depends_on")
        } else {
            Vec::new()
        };
        for fwd_id in &current_deps {
            if !triggers.contains(fwd_id) {
                targets.insert(fwd_id.clone());
            }
        }
        // Forward (removed): tasks that WERE in the trigger's previous
        // `depends_on` but are no longer there.
        if let Some(prev_deps) = previous_depends_on.get(trigger_id) {
            for prev_dep in prev_deps {
                if !current_deps.contains(prev_dep) && !triggers.contains(prev_dep) {
                    targets.insert(prev_dep.clone());
                }
            }
        }
    }
    targets
}

/// Re-enrich each fan-out target and emit synthetic `EntityFieldChanged`
/// events for the ones whose computed fields changed since the last cache
/// snapshot.
///
/// Updates the cache with the freshly computed snapshot for every target —
/// even when no event is emitted — so subsequent fan-outs diff against the
/// most recent state.
///
/// `targets` MUST exclude tasks already covered by the primary enrichment
/// loop; callers should hand in the result of [`compute_fanout_targets`].
/// Targets that don't exist in `all_tasks` are silently skipped (the entity
/// may have been deleted concurrently).
pub fn fan_out_synthetic_events(
    targets: &HashSet<String>,
    all_tasks: &[Entity],
    terminal_column_id: &str,
    registry: &VirtualTagRegistry,
    cache: &EnrichmentCache,
) -> Vec<WatchEvent> {
    let mut events = Vec::new();
    for target_id in targets {
        let Some(entity) = all_tasks
            .iter()
            .find(|t| t.id.as_ref() == target_id.as_str())
        else {
            continue;
        };
        let mut enriched = entity.clone();
        enrich_task_entity(&mut enriched, all_tasks, terminal_column_id, registry);
        let new_snapshot = TaskEnrichmentSnapshot::from_entity(&enriched);

        let mut cache_guard = cache.lock().unwrap();
        let prev = cache_guard.get(target_id).cloned().unwrap_or_default();
        let changes = prev.diff_to(&new_snapshot);
        cache_guard.insert(target_id.clone(), new_snapshot);
        drop(cache_guard);

        if !changes.is_empty() {
            events.push(WatchEvent::EntityFieldChanged {
                entity_type: "task".to_string(),
                id: target_id.clone(),
                changes,
            });
        }
    }
    events
}

/// Update the cache for a task that the primary enrichment loop already
/// emitted an event for, so the next fan-out pass diffs against the same
/// state the frontend just received.
///
/// `entity` must already have been enriched via `enrich_task_entity` (or its
/// equivalent) so the five computed fields are populated.
pub fn record_primary_enrichment(cache: &EnrichmentCache, task_id: &str, entity: &Entity) {
    let snapshot = TaskEnrichmentSnapshot::from_entity(entity);
    cache.lock().unwrap().insert(task_id.to_string(), snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use swissarmyhammer_kanban::virtual_tags::default_virtual_tag_registry;

    fn make_task(id: &str, column: &str, depends_on: &[&str]) -> Entity {
        let mut e = Entity::new("task", id);
        e.set("position_column", json!(column));
        if !depends_on.is_empty() {
            e.set("depends_on", json!(depends_on));
        }
        e
    }

    fn field_changed_event(id: &str, fields: &[&str]) -> WatchEvent {
        WatchEvent::EntityFieldChanged {
            entity_type: "task".to_string(),
            id: id.to_string(),
            changes: fields
                .iter()
                .map(|f| FieldChange {
                    field: (*f).to_string(),
                    value: json!(null),
                })
                .collect(),
        }
    }

    // ── collect_trigger_task_ids tests ────────────────────────────────

    #[test]
    fn trigger_collects_position_column_changes() {
        let events = vec![field_changed_event("a", &["position_column"])];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.contains("a"));
        assert_eq!(triggers.len(), 1);
    }

    #[test]
    fn trigger_collects_depends_on_changes() {
        let events = vec![field_changed_event("a", &["depends_on"])];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.contains("a"));
    }

    #[test]
    fn trigger_ignores_unrelated_fields() {
        // A title-only change is not a graph mutation; no fan-out needed.
        let events = vec![field_changed_event("a", &["title", "body"])];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.is_empty());
    }

    #[test]
    fn trigger_ignores_non_task_entities() {
        let events = vec![WatchEvent::EntityFieldChanged {
            entity_type: "column".to_string(),
            id: "c1".to_string(),
            changes: vec![FieldChange {
                field: "position_column".to_string(),
                value: json!("doing"),
            }],
        }];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.is_empty());
    }

    #[test]
    fn trigger_collects_task_create_events() {
        // Creating a new task potentially changes reverse/forward dependents'
        // computed tags, so it must be a trigger.
        let events = vec![WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "a".to_string(),
            fields: HashMap::new(),
        }];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.contains("a"));
    }

    #[test]
    fn trigger_collects_task_remove_events() {
        // Deleting a task breaks dependency edges; its reverse dependents
        // and the tasks it formerly depended on both need re-enrichment.
        let events = vec![WatchEvent::EntityRemoved {
            entity_type: "task".to_string(),
            id: "b".to_string(),
        }];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.contains("b"));
    }

    #[test]
    fn trigger_ignores_non_task_create_and_remove_events() {
        // Columns, tags, etc. don't participate in the depends_on graph.
        let events = vec![
            WatchEvent::EntityCreated {
                entity_type: "column".to_string(),
                id: "new-col".to_string(),
                fields: HashMap::new(),
            },
            WatchEvent::EntityRemoved {
                entity_type: "tag".to_string(),
                id: "stale-tag".to_string(),
            },
        ];
        let triggers = collect_trigger_task_ids(&events);
        assert!(triggers.is_empty());
    }

    // ── collect_removed_task_ids tests ────────────────────────────────

    #[test]
    fn removed_ids_collects_task_deletes() {
        let events = vec![
            WatchEvent::EntityRemoved {
                entity_type: "task".to_string(),
                id: "a".to_string(),
            },
            WatchEvent::EntityRemoved {
                entity_type: "task".to_string(),
                id: "b".to_string(),
            },
        ];
        let removed = collect_removed_task_ids(&events);
        assert_eq!(removed.len(), 2);
        assert!(removed.contains("a"));
        assert!(removed.contains("b"));
    }

    #[test]
    fn removed_ids_ignores_non_task_deletes() {
        let events = vec![WatchEvent::EntityRemoved {
            entity_type: "column".to_string(),
            id: "old-col".to_string(),
        }];
        let removed = collect_removed_task_ids(&events);
        assert!(removed.is_empty());
    }

    #[test]
    fn removed_ids_ignores_create_and_change_events() {
        let events = vec![
            WatchEvent::EntityCreated {
                entity_type: "task".to_string(),
                id: "a".to_string(),
                fields: HashMap::new(),
            },
            field_changed_event("b", &["position_column"]),
        ];
        let removed = collect_removed_task_ids(&events);
        assert!(removed.is_empty());
    }

    // ── prune_cache_for_removed tests ─────────────────────────────────

    #[test]
    fn prune_cache_removes_named_entries() {
        let cache = new_enrichment_cache();
        let mut e_a = Entity::new("task", "a");
        e_a.set("ready", json!(true));
        let mut e_b = Entity::new("task", "b");
        e_b.set("ready", json!(false));
        record_primary_enrichment(&cache, "a", &e_a);
        record_primary_enrichment(&cache, "b", &e_b);

        let removed = HashSet::from(["a".to_string()]);
        prune_cache_for_removed(&cache, &removed);

        let guard = cache.lock().unwrap();
        assert!(!guard.contains_key("a"), "a should be pruned");
        assert!(guard.contains_key("b"), "b should remain");
    }

    #[test]
    fn prune_cache_is_noop_for_empty_set() {
        let cache = new_enrichment_cache();
        let mut e = Entity::new("task", "a");
        e.set("ready", json!(true));
        record_primary_enrichment(&cache, "a", &e);

        prune_cache_for_removed(&cache, &HashSet::new());
        assert!(cache.lock().unwrap().contains_key("a"));
    }

    #[test]
    fn prune_cache_tolerates_missing_ids() {
        // Removing an id that was never cached should not panic.
        let cache = new_enrichment_cache();
        let removed = HashSet::from(["ghost".to_string()]);
        prune_cache_for_removed(&cache, &removed);
        assert!(cache.lock().unwrap().is_empty());
    }

    // ── End-to-end: EntityCreated / EntityRemoved fan-out ─────────────

    #[test]
    fn end_to_end_creating_b_depending_on_a_updates_a_blocks() {
        // When B is created with depends_on=[A], A's `blocks` list gains B
        // and A's BLOCKING virtual tag should appear. The primary loop only
        // emits a create event for B; the fan-out must catch A's stale state.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Pre-state: only A exists, cache primed with A's un-blocking state.
        let a_initial = make_task("a", "todo", &[]);
        let initial = vec![a_initial.clone()];
        let mut a_enriched = a_initial.clone();
        enrich_task_entity(&mut a_enriched, &initial, "done", registry);
        record_primary_enrichment(&cache, "a", &a_enriched);

        // Sanity: A currently has no BLOCKING tag.
        {
            let guard = cache.lock().unwrap();
            let vtags = guard
                .get("a")
                .unwrap()
                .fields
                .get("virtual_tags")
                .unwrap()
                .as_array()
                .unwrap()
                .clone();
            assert!(!vtags.contains(&json!("BLOCKING")));
        }

        // Mutation: B is created with depends_on=[A].
        let a_final = make_task("a", "todo", &[]);
        let b_final = make_task("b", "todo", &["a"]);
        let post = vec![a_final, b_final.clone()];

        let create_event = WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "b".to_string(),
            fields: HashMap::new(),
        };
        let triggers = collect_trigger_task_ids(&[create_event]);
        assert!(triggers.contains("b"));
        let previous = snapshot_previous_depends_on(&triggers, &cache);

        // Fan-out computes A as a target (reverse dependent from B's
        // current depends_on).
        let targets = compute_fanout_targets(&triggers, &post, &previous);
        assert!(targets.contains("a"));

        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        assert_eq!(events.len(), 1);
        let WatchEvent::EntityFieldChanged { id, changes, .. } = &events[0] else {
            panic!("expected EntityFieldChanged");
        };
        assert_eq!(id, "a");
        let by_field: HashMap<&str, &Value> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        assert_eq!(by_field.get("blocks"), Some(&&json!(["b"])));
        let vtags = by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(vtags.contains(&json!("BLOCKING")));
    }

    #[test]
    fn end_to_end_removing_last_blocker_unblocks_dependent() {
        // B depends on A (A in "todo" — B is BLOCKED). A is deleted.
        // Post-deletion, B's `depends_on` still references the (now-missing)
        // id; per `task_is_ready` semantics, a missing dependency keeps B
        // BLOCKED. The fan-out pass must still emit no spurious event for B
        // whose state is unchanged from cache.
        //
        // The regression captured by this test: if EntityRemoved is NOT a
        // trigger, reverse dependents are never re-evaluated at all, so any
        // state change they DO have (e.g. a different blocker count) would
        // silently stay stale until the next manual refresh.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Pre-state: A and B, with B BLOCKED by A.
        let a_initial = make_task("a", "todo", &[]);
        let b_initial = make_task("b", "todo", &["a"]);
        let initial = vec![a_initial, b_initial.clone()];
        for id in ["a", "b"] {
            let mut e = initial
                .iter()
                .find(|t| t.id.as_ref() == id)
                .unwrap()
                .clone();
            enrich_task_entity(&mut e, &initial, "done", registry);
            record_primary_enrichment(&cache, id, &e);
        }

        // Mutation: A deleted. Post-state has only B.
        let post = vec![b_initial];

        let remove_event = WatchEvent::EntityRemoved {
            entity_type: "task".to_string(),
            id: "a".to_string(),
        };
        let events_in = vec![remove_event];

        let triggers = collect_trigger_task_ids(&events_in);
        assert!(triggers.contains("a"));
        let removed = collect_removed_task_ids(&events_in);
        let previous = snapshot_previous_depends_on(&triggers, &cache);

        // Fan-out targets: B (reverse dependent — its depends_on still points
        // at the missing "a").
        let targets = compute_fanout_targets(&triggers, &post, &previous);
        assert!(targets.contains("b"));

        // Fan out — B's computed state (still BLOCKED + blocked_by=[a]) is
        // unchanged from cache; no synthetic event is needed.
        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        assert!(events.is_empty(), "B unchanged, no event expected");

        // Now prune A from cache. This must happen AFTER the snapshot/fan-out
        // so A's pre-deletion depends_on was available to the fan-out pass.
        prune_cache_for_removed(&cache, &removed);
        assert!(!cache.lock().unwrap().contains_key("a"));
        // B's entry survives the prune.
        assert!(cache.lock().unwrap().contains_key("b"));
    }

    #[test]
    fn end_to_end_removing_blocker_with_multi_dep_unblocks_dependent() {
        // B depends on [A, X] where A is done and X is todo. B is BLOCKED by
        // X. A is deleted. Post-deletion, B's `depends_on` still contains
        // [A, X] — "A" is missing, "X" still in todo — so B is still BLOCKED
        // but blocked_by is now [A, X] (missing counts as blocking) instead
        // of the pre-deletion [X] (A was in done, so not blocking).
        //
        // The fan-out should emit a blocked_by change for B.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        let a_initial = make_task("a", "done", &[]);
        let x_initial = make_task("x", "todo", &[]);
        let b_initial = make_task("b", "todo", &["a", "x"]);
        let initial = vec![a_initial, x_initial.clone(), b_initial.clone()];
        for id in ["a", "x", "b"] {
            let mut e = initial
                .iter()
                .find(|t| t.id.as_ref() == id)
                .unwrap()
                .clone();
            enrich_task_entity(&mut e, &initial, "done", registry);
            record_primary_enrichment(&cache, id, &e);
        }
        // Sanity: B blocked_by = [x] only (A is done).
        {
            let guard = cache.lock().unwrap();
            let b_blocked_by = guard
                .get("b")
                .unwrap()
                .fields
                .get("blocked_by")
                .unwrap()
                .as_array()
                .unwrap()
                .clone();
            assert_eq!(b_blocked_by, vec![json!("x")]);
        }

        // A deleted.
        let post = vec![x_initial, b_initial];
        let events_in = vec![WatchEvent::EntityRemoved {
            entity_type: "task".to_string(),
            id: "a".to_string(),
        }];

        let triggers = collect_trigger_task_ids(&events_in);
        let previous = snapshot_previous_depends_on(&triggers, &cache);
        let targets = compute_fanout_targets(&triggers, &post, &previous);
        assert!(targets.contains("b"));

        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        assert_eq!(events.len(), 1);
        let WatchEvent::EntityFieldChanged { id, changes, .. } = &events[0] else {
            panic!("expected EntityFieldChanged");
        };
        assert_eq!(id, "b");
        let by_field: HashMap<&str, &Value> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        // blocked_by now includes both A (missing → blocking) and X.
        let new_blocked = by_field.get("blocked_by").unwrap().as_array().unwrap();
        assert!(new_blocked.contains(&json!("a")));
        assert!(new_blocked.contains(&json!("x")));
    }

    // ── compute_fanout_targets tests ──────────────────────────────────

    #[test]
    fn fanout_includes_reverse_dependents() {
        // B depends on A. Trigger = A. Fan-out target = B.
        let a = make_task("a", "todo", &[]);
        let b = make_task("b", "todo", &["a"]);
        let triggers = HashSet::from(["a".to_string()]);
        let targets = compute_fanout_targets(&triggers, &[a, b], &HashMap::new());
        assert!(targets.contains("b"));
    }

    #[test]
    fn fanout_includes_forward_dependents() {
        // A depends on B. Trigger = A (its depends_on changed).
        // Fan-out target = B (its `blocks` may have changed).
        let a = make_task("a", "todo", &["b"]);
        let b = make_task("b", "todo", &[]);
        let triggers = HashSet::from(["a".to_string()]);
        let targets = compute_fanout_targets(&triggers, &[a, b], &HashMap::new());
        assert!(targets.contains("b"));
    }

    #[test]
    fn fanout_excludes_trigger_ids_themselves() {
        // The primary loop already enriches triggers; the fan-out skips them.
        let a = make_task("a", "todo", &["b"]);
        let b = make_task("b", "todo", &["a"]);
        let triggers = HashSet::from(["a".to_string(), "b".to_string()]);
        let targets = compute_fanout_targets(&triggers, &[a, b], &HashMap::new());
        assert!(targets.is_empty());
    }

    #[test]
    fn fanout_handles_missing_trigger_entity() {
        // Trigger ID not present in all_tasks (e.g. trigger was just deleted).
        // Should not panic; should still find reverse dependents.
        let b = make_task("b", "todo", &["ghost"]);
        let triggers = HashSet::from(["ghost".to_string()]);
        let targets = compute_fanout_targets(&triggers, &[b], &HashMap::new());
        assert!(targets.contains("b"));
    }

    #[test]
    fn fanout_dedups_targets_across_triggers() {
        // Two triggers, both pointing at C — C should appear only once.
        let a = make_task("a", "todo", &["c"]);
        let b = make_task("b", "todo", &["c"]);
        let c = make_task("c", "todo", &[]);
        let triggers = HashSet::from(["a".to_string(), "b".to_string()]);
        let targets = compute_fanout_targets(&triggers, &[a, b, c], &HashMap::new());
        assert_eq!(targets.len(), 1);
        assert!(targets.contains("c"));
    }

    #[test]
    fn fanout_includes_previous_forward_deps_that_were_removed() {
        // A USED to depend on [C], now depends on [D]. Trigger = A.
        // Reverse: none. Forward (new): D. Forward (removed): C.
        // Expected targets: {C, D}.
        let a = make_task("a", "todo", &["d"]);
        let c = make_task("c", "todo", &[]);
        let d = make_task("d", "todo", &[]);
        let triggers = HashSet::from(["a".to_string()]);
        let previous = HashMap::from([("a".to_string(), vec!["c".to_string()])]);
        let targets = compute_fanout_targets(&triggers, &[a, c, d], &previous);
        assert!(
            targets.contains("c"),
            "removed forward dep should be targeted"
        );
        assert!(targets.contains("d"), "new forward dep should be targeted");
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn fanout_previous_forward_deduped_against_current() {
        // A depends on both C and D now; previously also [C].
        // C is both "previous" and "current" — should only count once.
        let a = make_task("a", "todo", &["c", "d"]);
        let c = make_task("c", "todo", &[]);
        let d = make_task("d", "todo", &[]);
        let triggers = HashSet::from(["a".to_string()]);
        let previous = HashMap::from([("a".to_string(), vec!["c".to_string()])]);
        let targets = compute_fanout_targets(&triggers, &[a, c, d], &previous);
        assert!(targets.contains("c"));
        assert!(targets.contains("d"));
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn fanout_previous_forward_excluded_when_same_as_trigger() {
        // Trigger set also includes B; even if B was in A's previous deps,
        // we exclude triggers from fan-out targets (they're already handled
        // by the primary loop).
        let a = make_task("a", "todo", &[]);
        let b = make_task("b", "todo", &[]);
        let triggers = HashSet::from(["a".to_string(), "b".to_string()]);
        let previous = HashMap::from([("a".to_string(), vec!["b".to_string()])]);
        let targets = compute_fanout_targets(&triggers, &[a, b], &previous);
        assert!(targets.is_empty());
    }

    // ── snapshot_previous_depends_on tests ────────────────────────────

    #[test]
    fn snapshot_previous_depends_on_returns_empty_for_uncached() {
        let cache = new_enrichment_cache();
        let triggers = HashSet::from(["a".to_string(), "b".to_string()]);
        let snap = snapshot_previous_depends_on(&triggers, &cache);
        assert_eq!(snap.len(), 2);
        assert!(snap.get("a").unwrap().is_empty());
        assert!(snap.get("b").unwrap().is_empty());
    }

    #[test]
    fn snapshot_previous_depends_on_returns_cached_value() {
        let cache = new_enrichment_cache();
        let mut entity = Entity::new("task", "a");
        entity.set("depends_on", json!(["x", "y"]));
        record_primary_enrichment(&cache, "a", &entity);

        let triggers = HashSet::from(["a".to_string()]);
        let snap = snapshot_previous_depends_on(&triggers, &cache);
        assert_eq!(
            snap.get("a").unwrap(),
            &vec!["x".to_string(), "y".to_string()]
        );
    }

    // ── snapshot diff tests ───────────────────────────────────────────

    #[test]
    fn snapshot_from_entity_extracts_computed_fields_only() {
        let mut e = Entity::new("task", "t1");
        e.set("title", json!("Hello"));
        e.set("ready", json!(true));
        e.set("blocked_by", json!(["x"]));
        e.set("virtual_tags", json!(["READY"]));
        e.set("filter_tags", json!(["READY"]));
        e.set("blocks", json!([]));
        let snap = TaskEnrichmentSnapshot::from_entity(&e);
        assert_eq!(snap.fields.len(), 5);
        assert!(!snap.fields.contains_key("title"));
        assert_eq!(snap.fields.get("ready"), Some(&json!(true)));
    }

    #[test]
    fn snapshot_diff_returns_only_changed_fields() {
        let mut a = TaskEnrichmentSnapshot::default();
        a.fields.insert("ready".to_string(), json!(false));
        a.fields.insert("blocked_by".to_string(), json!(["dep1"]));
        a.fields
            .insert("virtual_tags".to_string(), json!(["BLOCKED"]));
        a.fields
            .insert("filter_tags".to_string(), json!(["BLOCKED"]));
        a.fields.insert("blocks".to_string(), json!([]));

        let mut b = a.clone();
        b.fields.insert("ready".to_string(), json!(true));
        b.fields
            .insert("blocked_by".to_string(), json!(Vec::<String>::new()));
        b.fields
            .insert("virtual_tags".to_string(), json!(["READY"]));
        b.fields.insert("filter_tags".to_string(), json!(["READY"]));

        let diff = a.diff_to(&b);
        let names: HashSet<&str> = diff.iter().map(|c| c.field.as_str()).collect();
        assert!(names.contains("ready"));
        assert!(names.contains("blocked_by"));
        assert!(names.contains("virtual_tags"));
        assert!(names.contains("filter_tags"));
        assert!(!names.contains("blocks"));
    }

    #[test]
    fn snapshot_diff_against_empty_emits_all_present_fields() {
        // First emission for a task: previous snapshot is empty, so every
        // computed field that has a value gets emitted.
        let empty = TaskEnrichmentSnapshot::default();
        let mut new = TaskEnrichmentSnapshot::default();
        new.fields.insert("ready".to_string(), json!(true));
        new.fields
            .insert("virtual_tags".to_string(), json!(["READY"]));
        let diff = empty.diff_to(&new);
        assert_eq!(diff.len(), 2);
    }

    #[test]
    fn snapshot_diff_identical_emits_nothing() {
        let mut a = TaskEnrichmentSnapshot::default();
        a.fields.insert("ready".to_string(), json!(true));
        let b = a.clone();
        assert!(a.diff_to(&b).is_empty());
    }

    // ── fan_out_synthetic_events integration ──────────────────────────

    #[test]
    fn fan_out_emits_event_when_blocked_state_flips() {
        // Setup: B depends on A. A is in "todo", so B is BLOCKED + not ready.
        // Cache reflects that prior emission.
        // Then A moves to "done". Fan-out target B should now be READY +
        // un-BLOCKED, and we expect a synthetic event with the updated fields.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Prime the cache to reflect "B was BLOCKED" (the last-known state).
        let mut prior = TaskEnrichmentSnapshot::default();
        prior.fields.insert("ready".to_string(), json!(false));
        prior.fields.insert("blocked_by".to_string(), json!(["a"]));
        prior
            .fields
            .insert("virtual_tags".to_string(), json!(["BLOCKED"]));
        prior
            .fields
            .insert("filter_tags".to_string(), json!(["BLOCKED"]));
        prior.fields.insert("blocks".to_string(), json!([]));
        cache.lock().unwrap().insert("b".to_string(), prior);

        // Post-mutation state: A is in "done".
        let a = make_task("a", "done", &[]);
        let b = make_task("b", "todo", &["a"]);
        let all = vec![a, b];
        let targets = HashSet::from(["b".to_string()]);

        let events = fan_out_synthetic_events(&targets, &all, "done", registry, &cache);
        assert_eq!(events.len(), 1);
        let WatchEvent::EntityFieldChanged { id, changes, .. } = &events[0] else {
            panic!("expected EntityFieldChanged event");
        };
        assert_eq!(id, "b");

        // The diff should contain ready=true, blocked_by=[], and updated tags.
        let by_field: HashMap<&str, &Value> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        assert_eq!(by_field.get("ready"), Some(&&json!(true)));
        assert_eq!(
            by_field.get("blocked_by"),
            Some(&&json!(Vec::<String>::new()))
        );
        // BLOCKED removed, READY added — virtual_tags should now contain READY.
        let vtags = by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(vtags.contains(&json!("READY")));
        assert!(!vtags.contains(&json!("BLOCKED")));
    }

    #[test]
    fn fan_out_emits_nothing_when_state_unchanged() {
        // B's enriched state matches the cache exactly — no event should fire.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Prepopulate the cache with what B would naturally enrich to.
        // B is in "todo" with no deps, so it's READY.
        let b = make_task("b", "todo", &[]);
        let all = vec![b.clone()];
        let mut enriched = b.clone();
        enrich_task_entity(&mut enriched, &all, "done", registry);
        record_primary_enrichment(&cache, "b", &enriched);

        // Now fan out — nothing changed since the cache snapshot.
        let targets = HashSet::from(["b".to_string()]);
        let events = fan_out_synthetic_events(&targets, &all, "done", registry, &cache);
        assert!(events.is_empty());
    }

    #[test]
    fn fan_out_emits_blocking_change_for_forward_target() {
        // A depends on B. A's depends_on changes (not modeled here directly,
        // but trigger=A means we fan out forward to B). B's `blocks` was empty,
        // now contains [A] — fan-out should emit an event for B.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Prime cache for B with stale state where B did NOT block anything.
        let mut prior = TaskEnrichmentSnapshot::default();
        prior.fields.insert("ready".to_string(), json!(true));
        prior
            .fields
            .insert("blocked_by".to_string(), json!(Vec::<String>::new()));
        prior
            .fields
            .insert("virtual_tags".to_string(), json!(["READY"]));
        prior
            .fields
            .insert("filter_tags".to_string(), json!(["READY"]));
        prior
            .fields
            .insert("blocks".to_string(), json!(Vec::<String>::new()));
        cache.lock().unwrap().insert("b".to_string(), prior);

        let a = make_task("a", "todo", &["b"]);
        let b = make_task("b", "todo", &[]);
        let all = vec![a, b];

        let targets = HashSet::from(["b".to_string()]);
        let events = fan_out_synthetic_events(&targets, &all, "done", registry, &cache);
        assert_eq!(events.len(), 1);
        let WatchEvent::EntityFieldChanged { id, changes, .. } = &events[0] else {
            panic!("expected EntityFieldChanged event");
        };
        assert_eq!(id, "b");
        let by_field: HashMap<&str, &Value> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        assert_eq!(by_field.get("blocks"), Some(&&json!(["a"])));
        // BLOCKING should now be in virtual_tags
        let vtags = by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(vtags.contains(&json!("BLOCKING")));
    }

    #[test]
    fn fan_out_skips_targets_missing_from_all_tasks() {
        // Target ID not present in all_tasks (e.g. concurrent delete).
        // Should not panic, should not emit.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();
        let targets = HashSet::from(["ghost".to_string()]);
        let events = fan_out_synthetic_events(&targets, &[], "done", registry, &cache);
        assert!(events.is_empty());
    }

    #[test]
    fn fan_out_updates_cache_even_when_no_event_emitted() {
        // The cache must always reflect the most recent enrichment, even if
        // diffing produces no event. This way the next fan-out diffs against
        // the freshest possible state.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();
        let b = make_task("b", "todo", &[]);
        let all = vec![b];
        let targets = HashSet::from(["b".to_string()]);

        // First pass: cache empty, B has no computed fields yet → first
        // emission seeds the cache.
        let _ = fan_out_synthetic_events(&targets, &all, "done", registry, &cache);
        assert!(cache.lock().unwrap().contains_key("b"));

        // Second pass: nothing changed, no event — but cache still present.
        let events = fan_out_synthetic_events(&targets, &all, "done", registry, &cache);
        assert!(events.is_empty());
        assert!(cache.lock().unwrap().contains_key("b"));
    }

    // ── End-to-end: A→B BLOCKED removal scenario from the card ────────

    #[test]
    fn end_to_end_moving_a_to_done_emits_blocked_removal_for_b() {
        // This is the headline acceptance criterion from the card:
        // create A and B (B depends_on A), move A to done, verify the
        // emitted events include B with BLOCKED removed from virtual_tags.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Initial state: A in "todo", B blocked by A.
        let a_initial = make_task("a", "todo", &[]);
        let b_initial = make_task("b", "todo", &["a"]);
        let initial = vec![a_initial, b_initial];

        // Prime the cache by enriching both tasks (simulates the prior
        // emissions at board-open time).
        for task_id in ["a", "b"] {
            let mut e = initial
                .iter()
                .find(|t| t.id.as_ref() == task_id)
                .unwrap()
                .clone();
            enrich_task_entity(&mut e, &initial, "done", registry);
            record_primary_enrichment(&cache, task_id, &e);
        }

        // Sanity check: B's primed cache has BLOCKED.
        {
            let guard = cache.lock().unwrap();
            let b_snap = guard.get("b").unwrap();
            let vtags = b_snap
                .fields
                .get("virtual_tags")
                .unwrap()
                .as_array()
                .unwrap();
            assert!(vtags.contains(&json!("BLOCKED")));
        }

        // Mutation: A moves to "done". A is the trigger; its event would have
        // been processed by the primary loop and the cache updated for A.
        let a_final = make_task("a", "done", &[]);
        let b_final = make_task("b", "todo", &["a"]);
        let post = vec![a_final.clone(), b_final];

        // Trigger set = {A}. Capture previous depends_on BEFORE the primary
        // loop overwrites the cache.
        let triggers = HashSet::from(["a".to_string()]);
        let previous = snapshot_previous_depends_on(&triggers, &cache);

        // Update cache for A (mimicking the primary enrichment loop).
        let mut a_enriched = a_final.clone();
        enrich_task_entity(&mut a_enriched, &post, "done", registry);
        record_primary_enrichment(&cache, "a", &a_enriched);

        // Compute fan-out targets using the captured previous state.
        let targets = compute_fanout_targets(&triggers, &post, &previous);
        assert!(targets.contains("b"));

        // Fan out and assert B's BLOCKED tag is removed in the synthetic event.
        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        assert_eq!(events.len(), 1);
        let WatchEvent::EntityFieldChanged { id, changes, .. } = &events[0] else {
            panic!("expected EntityFieldChanged event");
        };
        assert_eq!(id, "b");
        let by_field: HashMap<&str, &Value> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        let vtags = by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(!vtags.contains(&json!("BLOCKED")));
        assert!(vtags.contains(&json!("READY")));
        assert_eq!(by_field.get("ready"), Some(&&json!(true)));
        assert_eq!(
            by_field.get("blocked_by"),
            Some(&&json!(Vec::<String>::new()))
        );
    }

    #[test]
    fn end_to_end_moving_a_back_out_of_done_re_emits_blocked() {
        // The reverse of the previous test: A was in done (B unblocked),
        // A moves back to doing → B should regain BLOCKED.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        // Initial state: A in "done", B unblocked.
        let a_initial = make_task("a", "done", &[]);
        let b_initial = make_task("b", "todo", &["a"]);
        let initial = vec![a_initial, b_initial];
        for task_id in ["a", "b"] {
            let mut e = initial
                .iter()
                .find(|t| t.id.as_ref() == task_id)
                .unwrap()
                .clone();
            enrich_task_entity(&mut e, &initial, "done", registry);
            record_primary_enrichment(&cache, task_id, &e);
        }

        // Mutation: A moves to "doing".
        let a_final = make_task("a", "doing", &[]);
        let b_final = make_task("b", "todo", &["a"]);
        let post = vec![a_final.clone(), b_final];

        let triggers = HashSet::from(["a".to_string()]);
        let previous = snapshot_previous_depends_on(&triggers, &cache);

        let mut a_enriched = a_final.clone();
        enrich_task_entity(&mut a_enriched, &post, "done", registry);
        record_primary_enrichment(&cache, "a", &a_enriched);

        let targets = compute_fanout_targets(&triggers, &post, &previous);
        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        assert_eq!(events.len(), 1);
        let WatchEvent::EntityFieldChanged { id, changes, .. } = &events[0] else {
            panic!("expected EntityFieldChanged event");
        };
        assert_eq!(id, "b");
        let by_field: HashMap<&str, &Value> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        let vtags = by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(vtags.contains(&json!("BLOCKED")));
        assert!(!vtags.contains(&json!("READY")));
        assert_eq!(by_field.get("ready"), Some(&&json!(false)));
        assert_eq!(by_field.get("blocked_by"), Some(&&json!(["a"])));
    }

    #[test]
    fn end_to_end_editing_depends_on_refreshes_old_and_new_targets() {
        // Acceptance criterion: editing depends_on on task A triggers
        // re-enrichment of BOTH the old and new dependency targets (their
        // BLOCKING status may change).
        //
        // Initial: A depends on C. C is BLOCKING (A depends on it).
        // Mutation: A's depends_on becomes [D]. C should lose BLOCKING,
        //           D should gain BLOCKING.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        let a_initial = make_task("a", "todo", &["c"]);
        let c_initial = make_task("c", "todo", &[]);
        let d_initial = make_task("d", "todo", &[]);
        let initial = vec![a_initial, c_initial, d_initial];
        for task_id in ["a", "c", "d"] {
            let mut e = initial
                .iter()
                .find(|t| t.id.as_ref() == task_id)
                .unwrap()
                .clone();
            enrich_task_entity(&mut e, &initial, "done", registry);
            record_primary_enrichment(&cache, task_id, &e);
        }

        // Sanity: C has BLOCKING, D does not.
        {
            let guard = cache.lock().unwrap();
            let c_vtags = guard
                .get("c")
                .unwrap()
                .fields
                .get("virtual_tags")
                .unwrap()
                .as_array()
                .unwrap()
                .clone();
            assert!(c_vtags.contains(&json!("BLOCKING")));
            let d_vtags = guard
                .get("d")
                .unwrap()
                .fields
                .get("virtual_tags")
                .unwrap()
                .as_array()
                .unwrap()
                .clone();
            assert!(!d_vtags.contains(&json!("BLOCKING")));
        }

        // Mutation: A's depends_on changes from [C] to [D].
        let a_final = make_task("a", "todo", &["d"]);
        let c_final = make_task("c", "todo", &[]);
        let d_final = make_task("d", "todo", &[]);
        let post = vec![a_final.clone(), c_final, d_final];

        let triggers = HashSet::from(["a".to_string()]);
        let previous = snapshot_previous_depends_on(&triggers, &cache);
        assert_eq!(previous.get("a").unwrap(), &vec!["c".to_string()]);

        // Primary loop emits A (updates cache with new depends_on=[d]).
        let mut a_enriched = a_final.clone();
        enrich_task_entity(&mut a_enriched, &post, "done", registry);
        record_primary_enrichment(&cache, "a", &a_enriched);

        // Fan-out: C (removed forward) AND D (new forward) should both be
        // re-enriched and get synthetic events.
        let targets = compute_fanout_targets(&triggers, &post, &previous);
        assert!(
            targets.contains("c"),
            "removed forward dep C must be in fan-out"
        );
        assert!(
            targets.contains("d"),
            "new forward dep D must be in fan-out"
        );

        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        assert_eq!(events.len(), 2);

        // Verify C lost BLOCKING, D gained BLOCKING.
        let event_map: HashMap<String, Vec<FieldChange>> = events
            .into_iter()
            .map(|e| match e {
                WatchEvent::EntityFieldChanged { id, changes, .. } => (id, changes),
                _ => panic!("expected EntityFieldChanged"),
            })
            .collect();

        let c_changes = event_map.get("c").expect("expected event for C");
        let c_by_field: HashMap<&str, &Value> = c_changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        let c_vtags = c_by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(!c_vtags.contains(&json!("BLOCKING")));
        assert_eq!(
            c_by_field.get("blocks"),
            Some(&&json!(Vec::<String>::new())),
            "C no longer blocks anyone"
        );

        let d_changes = event_map.get("d").expect("expected event for D");
        let d_by_field: HashMap<&str, &Value> = d_changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();
        let d_vtags = d_by_field.get("virtual_tags").unwrap().as_array().unwrap();
        assert!(d_vtags.contains(&json!("BLOCKING")));
        assert_eq!(d_by_field.get("blocks"), Some(&&json!(["a"])));
    }

    #[test]
    fn end_to_end_no_spurious_events_when_column_move_does_not_flip_blocked() {
        // Acceptance criterion: no spurious events if re-enrichment produces
        // the same values.
        //
        // B depends on A. A moves between two NON-terminal columns (todo →
        // doing). B's BLOCKED state doesn't change — no event for B.
        let registry = default_virtual_tag_registry();
        let cache = new_enrichment_cache();

        let a_initial = make_task("a", "todo", &[]);
        let b_initial = make_task("b", "todo", &["a"]);
        let initial = vec![a_initial, b_initial];
        for task_id in ["a", "b"] {
            let mut e = initial
                .iter()
                .find(|t| t.id.as_ref() == task_id)
                .unwrap()
                .clone();
            enrich_task_entity(&mut e, &initial, "done", registry);
            record_primary_enrichment(&cache, task_id, &e);
        }

        // Mutation: A moves from "todo" to "doing" — still not terminal.
        let a_final = make_task("a", "doing", &[]);
        let b_final = make_task("b", "todo", &["a"]);
        let post = vec![a_final.clone(), b_final];

        let triggers = HashSet::from(["a".to_string()]);
        let previous = snapshot_previous_depends_on(&triggers, &cache);
        let mut a_enriched = a_final.clone();
        enrich_task_entity(&mut a_enriched, &post, "done", registry);
        record_primary_enrichment(&cache, "a", &a_enriched);

        let targets = compute_fanout_targets(&triggers, &post, &previous);
        let events = fan_out_synthetic_events(&targets, &post, "done", registry, &cache);
        // B's state is unchanged (still BLOCKED by A in "doing"); no event.
        assert!(
            events.is_empty(),
            "expected no events, got {} events",
            events.len()
        );
    }
}
