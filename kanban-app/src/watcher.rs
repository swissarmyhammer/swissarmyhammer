//! Thin bridge between `swissarmyhammer_entity::EntityCache` and the Tauri
//! frontend.
//!
//! This module used to re-implement entity caching, file watching, hash-based
//! dedupe, and field-level diffing — all concerns that now belong to the
//! `swissarmyhammer-entity` crate. After the `entity-cache` migration, the
//! kanban-app owns no entity state and does no filesystem work. It only:
//!
//! 1. Subscribes to `EntityCache::subscribe()`.
//! 2. Enriches each `EntityEvent` with compute-derived fields by re-reading
//!    through `EntityContext::read` (which applies `ComputeEngine.derive_all`)
//!    and, for task entities, running the kanban-layer cross-entity enrichment
//!    (`enrich_task_entity`) so `virtual_tags`/`filter_tags`/`ready`/
//!    `blocked_by`/`blocks` reflect the freshest state.
//! 3. Fans out synthetic `EntityFieldChanged` events to dependent tasks when a
//!    task's `position_column`, `depends_on`, or `completed` field changes —
//!    their computed fields may have shifted even though their own files did
//!    not.
//! 4. Translates each resolved event to the matching Tauri payload shape and
//!    tags it with a `board_path` so the frontend can route it to the correct
//!    window.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use swissarmyhammer_entity::events::EntityEvent;
use swissarmyhammer_entity::{Entity, EntityCache, EntityContext};
use swissarmyhammer_entity_search::EntitySearchIndex;
use swissarmyhammer_kanban::task_helpers::enrich_task_entity;
use swissarmyhammer_kanban::virtual_tags::default_virtual_tag_registry;
use swissarmyhammer_kanban::KanbanContext;
use tauri::Emitter;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Mutex, RwLock};

pub use swissarmyhammer_entity::events::FieldChange;

/// Task fields whose value changing can invalidate computed state on *other*
/// tasks via the dependency graph. A column move flips downstream `BLOCKED`
/// /`READY`; a `depends_on` edit reshuffles which tasks need re-enrichment.
const TASK_FANOUT_TRIGGER_FIELDS: &[&str] = &["position_column", "depends_on"];

/// Task computed fields whose value depends on cross-entity state (the full
/// task list, the terminal column, the virtual-tag registry). These are the
/// fields diffed in the fan-out pass — raw stored fields are owned by the
/// cache's own diff and never touched here.
const TASK_COMPUTED_FIELDS: &[&str] = &[
    "virtual_tags",
    "filter_tags",
    "ready",
    "blocked_by",
    "blocks",
];

/// Events emitted to the frontend when entity state changes.
///
/// **Architecture rule (event-architecture):** Events are thin signals with
/// exactly two granularities:
///
/// 1. **Entity-level** — `EntityCreated` and `EntityRemoved` carry
///    `(entity_type, id)`. For created entities the bridge also provides the
///    full `fields` snapshot (raw + computed) from the compute-enriched read.
///
/// 2. **Field-level** — `EntityFieldChanged` carries a `Vec<FieldChange>`,
///    one entry per changed field, each with the field name and new value.
///    Removals are encoded as `value: Null`.
///
/// The frontend contract:
/// - `entity-created`: add entity to store from payload fields.
/// - `entity-field-changed`: patch individual fields from `changes`. ONE
///   path, no branching, no re-fetch.
/// - `entity-removed`: remove entity from store.
/// - `attachment-changed`: refresh thumbnails/badges for the owning entity.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
#[allow(clippy::enum_variant_names)]
pub enum WatchEvent {
    /// A new entity file appeared.
    #[serde(rename = "entity-created")]
    EntityCreated {
        entity_type: String,
        id: String,
        fields: HashMap<String, serde_json::Value>,
    },
    /// An entity file was deleted.
    #[serde(rename = "entity-removed")]
    EntityRemoved { entity_type: String, id: String },
    /// One or more fields on an entity changed.
    ///
    /// Each `FieldChange` carries the field name and its new value.
    /// The frontend patches individual fields in place.
    #[serde(rename = "entity-field-changed")]
    EntityFieldChanged {
        entity_type: String,
        id: String,
        changes: Vec<FieldChange>,
    },
    /// An attachment file was created, modified, or deleted.
    ///
    /// Emitted when files inside `.attachments/` subdirectories change,
    /// allowing the frontend to update thumbnail previews and badge counts.
    #[serde(rename = "attachment-changed")]
    AttachmentChanged {
        /// The entity type that owns the attachment (e.g. "task").
        entity_type: String,
        /// The stored filename (e.g. "01ABC-screenshot.png").
        filename: String,
        /// Whether the file was removed (true) or created/modified (false).
        removed: bool,
    },
}

/// Wrapper that pairs a `WatchEvent` with the board it belongs to.
///
/// When emitted to the frontend, the JSON includes all fields from the inner
/// event (via `#[serde(flatten)]`) plus a `board_path` string so listeners
/// can filter events for their active board.
#[derive(Debug, Clone, Serialize)]
pub struct BoardWatchEvent {
    #[serde(flatten)]
    pub event: WatchEvent,
    pub board_path: String,
}

/// Apply a single `WatchEvent` to an `EntitySearchIndex`.
///
/// Reconstructs an `Entity` from the event fields and calls `update` or
/// `remove` on the index. Called from the bridge subscriber so the search
/// index stays in lockstep with every event emitted to the frontend.
pub fn sync_search_index(idx: &mut EntitySearchIndex, evt: &WatchEvent) {
    match evt {
        WatchEvent::EntityCreated {
            entity_type,
            id,
            fields,
        } => {
            let mut entity = Entity::new(entity_type.as_str(), id.as_str());
            for (k, v) in fields {
                entity.set(k, v.clone());
            }
            idx.update(entity);
        }
        WatchEvent::EntityFieldChanged {
            entity_type,
            id,
            changes,
        } => {
            if !changes.is_empty() {
                let fields_map: HashMap<String, serde_json::Value> = changes
                    .iter()
                    .map(|c| (c.field.clone(), c.value.clone()))
                    .collect();
                idx.merge_fields(entity_type, id, &fields_map);
            }
        }
        WatchEvent::EntityRemoved { id, .. } => {
            idx.remove(id);
        }
        WatchEvent::AttachmentChanged { .. } => {
            // Attachment file changes don't affect the search index.
            // They are forwarded to the frontend for UI updates only.
        }
    }
}

/// Emit one resolved `WatchEvent` to the frontend as the matching Tauri
/// event, wrapped in a `BoardWatchEvent` so receivers can filter by board.
///
/// The Tauri event name matches the `#[serde(rename = ...)]` discriminant
/// on `WatchEvent`, so the frontend listener keys stay stable.
pub fn emit_watch_event<R: tauri::Runtime, E: Emitter<R>>(
    emitter: &E,
    board_path: &str,
    evt: WatchEvent,
) {
    let event_name = match &evt {
        WatchEvent::EntityCreated { .. } => "entity-created",
        WatchEvent::EntityRemoved { .. } => "entity-removed",
        WatchEvent::EntityFieldChanged { .. } => "entity-field-changed",
        WatchEvent::AttachmentChanged { .. } => "attachment-changed",
    };
    let wrapped = BoardWatchEvent {
        event: evt,
        board_path: board_path.to_string(),
    };
    if let Err(e) = emitter.emit(event_name, &wrapped) {
        tracing::warn!(event_name, board_path, error = %e, "failed to emit Tauri event");
    }
}

/// Snapshot of a task's computed-field values captured after enrichment.
///
/// The bridge keeps one of these per task id so the fan-out pass can diff
/// the latest enrichment against the last-emitted baseline and emit
/// synthetic events only for fields whose values actually changed. Storing
/// JSON values (rather than typed `Vec<String>`/`bool`) keeps the diff
/// agnostic to underlying field type and matches the wire format.
#[derive(Clone, Debug, Default)]
struct TaskComputedSnapshot {
    fields: HashMap<String, serde_json::Value>,
}

impl TaskComputedSnapshot {
    /// Extract the five computed task fields from an already-enriched entity.
    fn from_entity(entity: &Entity) -> Self {
        let mut fields = HashMap::with_capacity(TASK_COMPUTED_FIELDS.len());
        for name in TASK_COMPUTED_FIELDS {
            if let Some(v) = entity.get(name) {
                fields.insert((*name).to_string(), v.clone());
            }
        }
        Self { fields }
    }

    /// Return the field changes that differ between this snapshot and `other`.
    fn diff_to(&self, other: &Self) -> Vec<FieldChange> {
        let mut changes = Vec::new();
        for name in TASK_COMPUTED_FIELDS {
            let Some(new_val) = other.fields.get(*name) else {
                continue;
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

/// Per-board bridge state that outlives individual events: the "seen"
/// (entity_type, id) set used to tell first-observation from modification,
/// and the cached computed-field snapshot per task used by the fan-out diff.
struct BridgeState {
    seen: HashSet<(String, String)>,
    task_snapshots: HashMap<String, TaskComputedSnapshot>,
}

impl BridgeState {
    fn new(seen: HashSet<(String, String)>) -> Self {
        Self {
            seen,
            task_snapshots: HashMap::new(),
        }
    }
}

/// Pre-populate the "seen" set from every entity currently in the cache.
///
/// `EntityCache::load_all` fills the cache without emitting events, so the
/// bridge needs a starting snapshot to tell "already loaded" apart from
/// "newly created after subscribe". We walk every cached entity type that
/// has entities and record `(entity_type, id)` pairs.
///
/// Runs BEFORE the bridge subscribes to the cache, so no event is queued
/// against a partially-built snapshot.
async fn pre_populate_seen(cache: &EntityCache) -> HashSet<(String, String)> {
    let mut seen = HashSet::new();
    let fields_ctx = cache.inner().fields();
    for entity_def in fields_ctx.all_entities() {
        let entity_type = entity_def.name.as_str();
        for entity in cache.get_all(entity_type).await {
            seen.insert((entity_type.to_string(), entity.id.to_string()));
        }
    }
    seen
}

/// Read `entity_type`/`id` through `EntityContext::read` and, for tasks, apply
/// the kanban-layer cross-entity enrichment on top of ComputeEngine-derived
/// values. Returns `None` if the entity could not be read.
async fn read_enriched(ectx: &EntityContext, entity_type: &str, id: &str) -> Option<Entity> {
    let mut entity = match ectx.read(entity_type, id).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                entity_type = %entity_type,
                id = %id,
                error = %e,
                "bridge: failed to read entity for enrichment"
            );
            return None;
        }
    };
    if entity_type == "task" {
        apply_task_enrichment(ectx, &mut entity).await;
    }
    Some(entity)
}

/// Read the current full task list plus the terminal column id (the column
/// with the highest `order`, fallback `"done"`). Both are needed for every
/// task-enrichment call, so pairing them keeps the two `list` calls in one
/// place.
async fn load_task_enrichment_inputs(ectx: &EntityContext) -> (Vec<Entity>, String) {
    let all_tasks = ectx.list("task").await.unwrap_or_default();
    let mut columns = ectx.list("column").await.unwrap_or_default();
    columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
    let terminal_id = columns
        .last()
        .map(|c| c.id.to_string())
        .unwrap_or_else(|| "done".to_string());
    (all_tasks, terminal_id)
}

/// Run `enrich_task_entity` against a single task, sourcing the full task list
/// and terminal column id from `ectx`. No-op if the task list or columns
/// cannot be read — the entity is returned with whatever ComputeEngine
/// produced, which is the same behaviour as before this card.
async fn apply_task_enrichment(ectx: &EntityContext, entity: &mut Entity) {
    let (all_tasks, terminal_id) = load_task_enrichment_inputs(ectx).await;
    enrich_task_entity(
        entity,
        &all_tasks,
        &terminal_id,
        default_virtual_tag_registry(),
    );
}

/// Return `true` when `changes` touches any field whose mutation can flip
/// another task's computed state via the dependency graph.
fn touches_fanout_field(changes: &[FieldChange]) -> bool {
    changes
        .iter()
        .any(|c| TASK_FANOUT_TRIGGER_FIELDS.contains(&c.field.as_str()))
}

/// Build synthetic `EntityFieldChanged` events for every task whose computed
/// fields changed as a consequence of a trigger task event.
///
/// Walks the full task list, re-enriches every task other than the trigger,
/// and diffs against the bridge's per-task snapshot cache. Only tasks with
/// non-empty diffs emit an event. The cache is updated with the fresh
/// snapshot for every task visited so subsequent fan-outs diff against the
/// most recent state even when no event fires.
async fn fan_out_task_dependents(
    ectx: &EntityContext,
    trigger_id: &str,
    snapshots: &mut HashMap<String, TaskComputedSnapshot>,
) -> Vec<WatchEvent> {
    let (mut all_tasks, terminal_id) = load_task_enrichment_inputs(ectx).await;
    let registry = default_virtual_tag_registry();
    // Enrichment mutates each entity in place; clone the list up front so we
    // have both the stable `&[Entity]` input and a mutable owner.
    let reference = all_tasks.clone();
    let mut events = Vec::new();
    for entity in all_tasks.iter_mut() {
        let id = entity.id.to_string();
        enrich_task_entity(entity, &reference, &terminal_id, registry);
        let new_snapshot = TaskComputedSnapshot::from_entity(entity);
        let prev = snapshots.get(&id).cloned().unwrap_or_default();
        let changes = prev.diff_to(&new_snapshot);
        snapshots.insert(id.clone(), new_snapshot);
        // The trigger's own event was already emitted by the caller with the
        // enriched fields merged in. Skip it here to avoid a duplicate.
        if id == trigger_id || changes.is_empty() {
            continue;
        }
        events.push(WatchEvent::EntityFieldChanged {
            entity_type: "task".to_string(),
            id,
            changes,
        });
    }
    events
}

/// Build the fields map for an `EntityCreated` payload.
///
/// Includes every non-null field on the enriched entity — both raw on-disk
/// fields (which `changes` already covered) and ComputeEngine-derived fields
/// (`progress`, `tags`, `virtual_tags`, `filter_tags`, etc.). Null values
/// are dropped so the frontend's `Object.keys(fields).length > 0` fast-path
/// detects an empty payload as "no data" rather than "explicitly null".
fn fields_map_from_enriched(entity: &Entity) -> HashMap<String, serde_json::Value> {
    entity
        .fields
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Augment an `EntityFieldChanged` change list with computed-field values.
///
/// The entity crate's `EntityCache::write` diffs raw canonical fields only.
/// After re-reading through `ectx.read` the entity carries compute-derived
/// fields on top; this helper appends only those (per
/// `fields_ctx.fields_for_entity(entity_type)` with `FieldType::Computed`)
/// unless the field name is already present from the raw diff.
///
/// Raw on-disk fields are intentionally NOT appended here — they would
/// pollute the diff and make fan-out heuristics like `touches_fanout_field`
/// misfire (e.g. every write would look like a `position_column` change).
fn append_computed_changes(
    ectx: &EntityContext,
    entity_type: &str,
    entity: &Entity,
    changes: &mut Vec<FieldChange>,
) {
    let existing: HashSet<String> = changes.iter().map(|c| c.field.clone()).collect();
    let mut computed_names: Vec<String> = ectx
        .fields()
        .fields_for_entity(entity_type)
        .into_iter()
        .filter(|fd| matches!(fd.type_, swissarmyhammer_fields::FieldType::Computed { .. }))
        .map(|fd| fd.name.to_string())
        .collect();
    // Also include the kanban-layer enrichment fields, which are set directly
    // by `enrich_task_entity` (not via ComputeEngine registrations) and
    // therefore don't appear as `FieldType::Computed` in the registry.
    for extra in TASK_COMPUTED_FIELDS {
        if entity_type == "task" && !computed_names.iter().any(|n| n == extra) {
            computed_names.push((*extra).to_string());
        }
    }
    // Stable emission order for tests.
    computed_names.sort();
    for name in computed_names {
        if existing.contains(name.as_str()) {
            continue;
        }
        if let Some(value) = entity.fields.get(&name) {
            if value.is_null() {
                continue;
            }
            changes.push(FieldChange {
                field: name.clone(),
                value: value.clone(),
            });
        }
    }
}

/// Translate an `EntityEvent` into one or more resolved `WatchEvent`s,
/// updating bridge state in the process.
///
/// Returns a `Vec` so a single cache event can produce synthetic fan-out
/// events for dependent tasks alongside the primary event.
async fn resolve_event(
    evt: EntityEvent,
    ctx: &KanbanContext,
    state: &mut BridgeState,
) -> Vec<WatchEvent> {
    match evt {
        EntityEvent::EntityChanged {
            entity_type,
            id,
            changes,
            ..
        } => resolve_entity_changed(ctx, state, entity_type, id, changes).await,
        EntityEvent::EntityDeleted { entity_type, id } => {
            resolve_entity_deleted(ctx, state, entity_type, id).await
        }
        EntityEvent::AttachmentChanged {
            entity_type,
            filename,
            removed,
        } => vec![WatchEvent::AttachmentChanged {
            entity_type,
            filename,
            removed,
        }],
    }
}

/// Handle an `EntityChanged` cache event: enrich, classify as create vs
/// field-change, and append task fan-out events when graph-relevant fields
/// flipped.
async fn resolve_entity_changed(
    ctx: &KanbanContext,
    state: &mut BridgeState,
    entity_type: String,
    id: String,
    changes: Vec<FieldChange>,
) -> Vec<WatchEvent> {
    let Ok(ectx) = ctx.entity_context().await else {
        tracing::warn!("bridge: entity context unavailable — emitting raw event");
        return vec![raw_changed_event(
            &entity_type,
            &id,
            changes,
            &mut state.seen,
        )];
    };
    let enriched = read_enriched(&ectx, &entity_type, &id).await;

    let key = (entity_type.clone(), id.clone());
    let is_new = state.seen.insert(key);

    let primary = if is_new {
        build_created_event(&entity_type, &id, enriched.as_ref(), changes)
    } else {
        build_field_changed_event(&ectx, &entity_type, &id, enriched.as_ref(), changes)
    };

    let mut out = vec![primary];
    if entity_type == "task" {
        apply_task_fanout(&ectx, state, &id, enriched.as_ref(), &mut out).await;
    }
    out
}

/// Build the `EntityCreated` payload for a first-observation of an entity.
///
/// When enrichment succeeded the fields map carries the full raw + computed
/// snapshot so the frontend can render immediately. Otherwise it falls back
/// to the raw `changes` list so the entity is still visible even when
/// enrichment was unavailable.
fn build_created_event(
    entity_type: &str,
    id: &str,
    enriched: Option<&Entity>,
    changes: Vec<FieldChange>,
) -> WatchEvent {
    let fields = match enriched {
        Some(e) => fields_map_from_enriched(e),
        None => changes.into_iter().map(|c| (c.field, c.value)).collect(),
    };
    WatchEvent::EntityCreated {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        fields,
    }
}

/// Build the `EntityFieldChanged` payload for a repeat observation, merging
/// compute-derived fields from the enriched read on top of the cache's raw
/// field diff.
fn build_field_changed_event(
    ectx: &EntityContext,
    entity_type: &str,
    id: &str,
    enriched: Option<&Entity>,
    mut changes: Vec<FieldChange>,
) -> WatchEvent {
    if let Some(e) = enriched {
        append_computed_changes(ectx, entity_type, e, &mut changes);
    }
    WatchEvent::EntityFieldChanged {
        entity_type: entity_type.to_string(),
        id: id.to_string(),
        changes,
    }
}

/// Apply task-specific fan-out logic: when the primary event flips a
/// graph-dependent field, emit synthetic events for dependents; otherwise
/// simply refresh the trigger task's snapshot so future fan-outs diff
/// against the freshest baseline.
async fn apply_task_fanout(
    ectx: &EntityContext,
    state: &mut BridgeState,
    id: &str,
    enriched: Option<&Entity>,
    out: &mut Vec<WatchEvent>,
) {
    let touches_graph = matches!(
        out[0],
        WatchEvent::EntityCreated { .. } | WatchEvent::EntityRemoved { .. }
    ) || match &out[0] {
        WatchEvent::EntityFieldChanged { changes, .. } => touches_fanout_field(changes),
        _ => false,
    };
    if touches_graph {
        let synthetic = fan_out_task_dependents(ectx, id, &mut state.task_snapshots).await;
        out.extend(synthetic);
    } else if let Some(e) = enriched {
        state
            .task_snapshots
            .insert(id.to_string(), TaskComputedSnapshot::from_entity(e));
    }
}

/// Handle an `EntityDeleted` cache event: drop the entity from the "seen"
/// set, emit the `EntityRemoved` event, and (for tasks) fan out synthetic
/// events for dependents whose computed fields may have shifted.
async fn resolve_entity_deleted(
    ctx: &KanbanContext,
    state: &mut BridgeState,
    entity_type: String,
    id: String,
) -> Vec<WatchEvent> {
    state.seen.remove(&(entity_type.clone(), id.clone()));
    let mut out = vec![WatchEvent::EntityRemoved {
        entity_type: entity_type.clone(),
        id: id.clone(),
    }];
    if entity_type == "task" {
        state.task_snapshots.remove(&id);
        if let Ok(ectx) = ctx.entity_context().await {
            let synthetic = fan_out_task_dependents(&ectx, &id, &mut state.task_snapshots).await;
            out.extend(synthetic);
        }
    }
    out
}

/// Fallback event mapping when `EntityContext::read` is unavailable.
///
/// Preserves the created-vs-modified distinction using the `seen` set but
/// does not attempt computed-field enrichment — the frontend's existing
/// fast-path handles raw fields gracefully. Only used when `KanbanContext`
/// is not yet initialized; in the normal flow `resolve_event` always has
/// access to an `EntityContext`.
fn raw_changed_event(
    entity_type: &str,
    id: &str,
    changes: Vec<FieldChange>,
    seen: &mut HashSet<(String, String)>,
) -> WatchEvent {
    let key = (entity_type.to_string(), id.to_string());
    if seen.insert(key) {
        let fields: HashMap<String, serde_json::Value> =
            changes.into_iter().map(|c| (c.field, c.value)).collect();
        WatchEvent::EntityCreated {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            fields,
        }
    } else {
        WatchEvent::EntityFieldChanged {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            changes,
        }
    }
}

/// Subscribe to an `EntityCache` and forward every event to Tauri, scoped
/// to a specific board.
///
/// Runs until the broadcast channel closes — which happens when the cache
/// (and therefore the surrounding `KanbanContext`) is dropped. The bridge:
///
/// - Pre-populates a "seen" set from the cache BEFORE subscribing so the
///   first `EntityChanged` for an unknown `(entity_type, id)` surfaces as
///   `entity-created` and repeat changes surface as `entity-field-changed`,
///   matching the frontend's long-standing contract. The ordering is
///   deliberate: subscribing first would leave a narrow window in which a
///   write could land between `subscribe()` and the snapshot, causing the
///   bridge to see the new entity in the cache snapshot AND on the
///   receiver, then mis-classify it as a field-change because
///   `seen.insert(...)` returns `false`.
/// - Re-reads each changed entity through `EntityContext::read` and, for
///   tasks, runs the kanban-layer enrichment so ComputeEngine-derived
///   fields (`progress`, `tags`) and kanban graph fields (`virtual_tags`,
///   `filter_tags`, `ready`, `blocked_by`, `blocks`) are merged into the
///   emitted payload.
/// - Fans out synthetic `EntityFieldChanged` events to dependent tasks when
///   a task write touches `position_column` or `depends_on`.
/// - Updates the shared `EntitySearchIndex` in lockstep with every emission
///   so full-text search results don't drift behind the frontend store.
/// - Logs and continues on `Lagged` — dropping events keeps the bridge
///   alive at the cost of a momentary index/store drift, which self-heals
///   on the next write.
///
/// Failure modes are logged but do not propagate: the bridge is a
/// fire-and-forget observer, not a control plane.
pub async fn run_bridge(
    ctx: Arc<KanbanContext>,
    cache: Arc<EntityCache>,
    app: tauri::AppHandle,
    board_path: String,
    search_index: Arc<RwLock<EntitySearchIndex>>,
) {
    // Pre-populate BEFORE subscribing so no event lands between the snapshot
    // and the receiver. A write that lands *before* subscribe will be
    // reflected in the snapshot; the next observed write for that entity
    // (if any) correctly surfaces as a field-change.
    let seen = pre_populate_seen(&cache).await;
    let preloaded_entities = seen.len();
    let state = Arc::new(Mutex::new(BridgeState::new(seen)));
    let mut rx = cache.subscribe();
    tracing::info!(
        board_path = %board_path,
        preloaded_entities,
        "entity-cache bridge started"
    );

    loop {
        match rx.recv().await {
            Ok(evt) => {
                process_cache_event(evt, &ctx, &state, &app, &board_path, &search_index).await;
            }
            Err(RecvError::Lagged(n)) => {
                tracing::warn!(
                    board_path = %board_path,
                    skipped = n,
                    "bridge lagged — search index and frontend may briefly drift"
                );
            }
            Err(RecvError::Closed) => {
                tracing::info!(board_path = %board_path, "entity-cache bridge stopped");
                break;
            }
        }
    }
}

/// Resolve a single cache event to zero-or-more `WatchEvent`s, synchronise
/// the search index, and emit the resulting events to the frontend.
///
/// Extracted from `run_bridge` so the outer loop only deals with receive
/// error classification.
async fn process_cache_event(
    evt: EntityEvent,
    ctx: &Arc<KanbanContext>,
    state: &Arc<Mutex<BridgeState>>,
    app: &tauri::AppHandle,
    board_path: &str,
    search_index: &Arc<RwLock<EntitySearchIndex>>,
) {
    let resolved = {
        let mut state_guard = state.lock().await;
        resolve_event(evt, ctx.as_ref(), &mut state_guard).await
    };
    {
        let mut idx = search_index.write().await;
        for watch_event in &resolved {
            sync_search_index(&mut idx, watch_event);
        }
    }
    for watch_event in resolved {
        emit_watch_event(app, board_path, watch_event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use swissarmyhammer_entity::events::FieldChange as EntityFieldChange;
    use swissarmyhammer_kanban::{
        board::InitBoard,
        task::{AddTask, MoveTask, UpdateTask},
        Execute, KanbanContext, TaskId,
    };
    use tempfile::TempDir;

    /// Minimal fields context + temp-dir scaffolding for `map_event`-style tests.
    /// Returns (temp dir, cache).
    async fn setup_cache() -> (TempDir, Arc<EntityCache>) {
        let fields = swissarmyhammer_entity::test_utils::test_fields_context();
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("tags")).unwrap();
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        let ctx = Arc::new(EntityContext::new(&root, fields));
        let cache = Arc::new(EntityCache::new(ctx));
        (temp, cache)
    }

    /// Set up a `.kanban/` board on disk with default columns (todo/doing/done)
    /// and return both the context and the raw `Arc<EntityCache>` the bridge
    /// would subscribe to.
    async fn setup_kanban_with_board() -> (TempDir, Arc<KanbanContext>, Arc<EntityCache>) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        // After InitBoard runs, entity_context() is primed and entity_cache()
        // returns the shared cache.
        let _ = ctx.entity_context().await.unwrap();
        let cache = ctx.entity_cache().expect("cache must be primed");
        (temp, Arc::new(ctx), cache)
    }

    fn new_test_seen_from(entries: &[(&str, &str)]) -> HashSet<(String, String)> {
        entries
            .iter()
            .map(|(t, i)| (t.to_string(), i.to_string()))
            .collect()
    }

    #[test]
    fn raw_changed_event_first_time_is_entity_created() {
        let mut seen = HashSet::new();
        let changes = vec![
            FieldChange {
                field: "tag_name".into(),
                value: json!("Bug"),
            },
            FieldChange {
                field: "color".into(),
                value: json!("#ff0000"),
            },
        ];
        let out = raw_changed_event("tag", "bug", changes, &mut seen);
        match out {
            WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "bug");
                assert_eq!(fields.get("tag_name").unwrap(), &json!("Bug"));
                assert_eq!(fields.get("color").unwrap(), &json!("#ff0000"));
            }
            other => panic!("expected EntityCreated, got {other:?}"),
        }
        assert!(seen.contains(&("tag".to_string(), "bug".to_string())));
    }

    #[test]
    fn raw_changed_event_second_time_is_entity_field_changed() {
        let mut seen = new_test_seen_from(&[("tag", "bug")]);
        let changes = vec![FieldChange {
            field: "color".into(),
            value: json!("#00ff00"),
        }];
        let out = raw_changed_event("tag", "bug", changes, &mut seen);
        match out {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "bug");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].field, "color");
                assert_eq!(changes[0].value, json!("#00ff00"));
            }
            other => panic!("expected EntityFieldChanged, got {other:?}"),
        }
    }

    #[test]
    fn raw_changed_event_deleted_then_recreated_emits_entity_created_again() {
        let mut seen = new_test_seen_from(&[("tag", "bug")]);
        // Simulate delete: drop the key from seen.
        seen.remove(&("tag".to_string(), "bug".to_string()));

        let out = raw_changed_event(
            "tag",
            "bug",
            vec![FieldChange {
                field: "tag_name".into(),
                value: json!("Bug"),
            }],
            &mut seen,
        );
        assert!(matches!(out, WatchEvent::EntityCreated { .. }));
    }

    #[test]
    fn sync_search_index_upserts_on_entity_created() {
        let mut idx = EntitySearchIndex::default();
        let evt = WatchEvent::EntityCreated {
            entity_type: "tag".into(),
            id: "bug".into(),
            fields: {
                let mut m = HashMap::new();
                m.insert("tag_name".into(), json!("Bug"));
                m.insert("color".into(), json!("#ff0000"));
                m
            },
        };
        sync_search_index(&mut idx, &evt);
        assert!(idx.get("bug").is_some());
    }

    #[test]
    fn sync_search_index_patches_on_field_changed() {
        let mut idx = EntitySearchIndex::default();
        let mut e = Entity::new("tag", "bug");
        e.set("tag_name", json!("Bug"));
        e.set("color", json!("#ff0000"));
        idx.update(e);

        let evt = WatchEvent::EntityFieldChanged {
            entity_type: "tag".into(),
            id: "bug".into(),
            changes: vec![FieldChange {
                field: "color".into(),
                value: json!("#00ff00"),
            }],
        };
        sync_search_index(&mut idx, &evt);

        let found = idx.get("bug").unwrap();
        assert_eq!(found.get_str("color"), Some("#00ff00"));
        assert_eq!(found.get_str("tag_name"), Some("Bug"));
    }

    #[test]
    fn sync_search_index_removes_on_entity_removed() {
        let mut idx = EntitySearchIndex::default();
        let mut e = Entity::new("tag", "bug");
        e.set("tag_name", json!("Bug"));
        idx.update(e);
        assert!(idx.get("bug").is_some());

        let evt = WatchEvent::EntityRemoved {
            entity_type: "tag".into(),
            id: "bug".into(),
        };
        sync_search_index(&mut idx, &evt);
        assert!(idx.get("bug").is_none());
    }

    #[test]
    fn sync_search_index_attachment_is_noop() {
        let mut idx = EntitySearchIndex::default();
        let evt = WatchEvent::AttachmentChanged {
            entity_type: "task".into(),
            filename: "foo.png".into(),
            removed: false,
        };
        sync_search_index(&mut idx, &evt);
        assert!(idx.get("foo.png").is_none());
    }

    #[tokio::test]
    async fn pre_populate_seen_captures_cached_entities() {
        let (_dir, cache) = setup_cache().await;

        let mut t = Entity::new("tag", "bug");
        t.set("tag_name", json!("Bug"));
        cache.write(&t).await.unwrap();

        let mut t2 = Entity::new("tag", "feat");
        t2.set("tag_name", json!("Feature"));
        cache.write(&t2).await.unwrap();

        let seen = pre_populate_seen(&cache).await;
        assert!(seen.contains(&("tag".to_string(), "bug".to_string())));
        assert!(seen.contains(&("tag".to_string(), "feat".to_string())));
    }

    #[tokio::test]
    async fn append_computed_changes_fills_in_missing_computed_fields() {
        // Uses a real KanbanContext so the fields registry has task's
        // computed fields registered — the helper intentionally only
        // appends computed fields, not raw stored ones.
        let (_temp, ctx, _cache) = setup_kanban_with_board().await;
        let ectx = ctx.entity_context().await.unwrap();

        let mut e = Entity::new("task", "t1");
        e.set("title", json!("T"));
        e.set("position_column", json!("todo"));
        e.set(
            "progress",
            json!({"total": 2, "completed": 1, "percent": 50}),
        );
        e.set("virtual_tags", json!(["READY"]));

        let mut changes = vec![FieldChange {
            field: "title".into(),
            value: json!("T"),
        }];
        append_computed_changes(&ectx, "task", &e, &mut changes);

        let by_field: HashMap<String, serde_json::Value> =
            changes.into_iter().map(|c| (c.field, c.value)).collect();
        // Raw field preserved (was already in the vec).
        assert_eq!(by_field.get("title"), Some(&json!("T")));
        // Computed field appended.
        assert_eq!(
            by_field.get("progress"),
            Some(&json!({"total": 2, "completed": 1, "percent": 50}))
        );
        assert_eq!(by_field.get("virtual_tags"), Some(&json!(["READY"])));
        // Raw stored field NOT appended — it would pollute the diff and
        // make fan-out heuristics like `touches_fanout_field` misfire.
        assert!(!by_field.contains_key("position_column"));
    }

    #[test]
    fn task_computed_snapshot_diff_to_detects_changes() {
        let mut old = Entity::new("task", "t1");
        old.set("virtual_tags", json!([]));
        old.set("ready", json!(false));
        let old_snap = TaskComputedSnapshot::from_entity(&old);

        let mut new = Entity::new("task", "t1");
        new.set("virtual_tags", json!(["READY"]));
        new.set("ready", json!(true));
        let new_snap = TaskComputedSnapshot::from_entity(&new);

        let changes = old_snap.diff_to(&new_snap);
        let by_field: HashMap<String, serde_json::Value> =
            changes.into_iter().map(|c| (c.field, c.value)).collect();
        assert_eq!(by_field.get("virtual_tags"), Some(&json!(["READY"])));
        assert_eq!(by_field.get("ready"), Some(&json!(true)));
    }

    #[test]
    fn task_computed_snapshot_diff_to_empty_when_unchanged() {
        let mut e = Entity::new("task", "t1");
        e.set("virtual_tags", json!(["READY"]));
        e.set("ready", json!(true));
        let snap_a = TaskComputedSnapshot::from_entity(&e);
        let snap_b = TaskComputedSnapshot::from_entity(&e);

        assert!(snap_a.diff_to(&snap_b).is_empty());
    }

    #[test]
    fn touches_fanout_field_detects_position_column_change() {
        let changes = vec![FieldChange {
            field: "position_column".into(),
            value: json!("done"),
        }];
        assert!(touches_fanout_field(&changes));
    }

    #[test]
    fn touches_fanout_field_detects_depends_on_change() {
        let changes = vec![FieldChange {
            field: "depends_on".into(),
            value: json!(["t1"]),
        }];
        assert!(touches_fanout_field(&changes));
    }

    #[test]
    fn touches_fanout_field_false_for_title_change() {
        let changes = vec![FieldChange {
            field: "title".into(),
            value: json!("new title"),
        }];
        assert!(!touches_fanout_field(&changes));
    }

    #[test]
    fn fields_map_from_enriched_drops_null_values() {
        let mut e = Entity::new("task", "t1");
        e.set("title", json!("T"));
        e.set("empty_field", json!(null));
        let map = fields_map_from_enriched(&e);
        assert!(map.contains_key("title"));
        assert!(!map.contains_key("empty_field"));
    }

    #[tokio::test]
    async fn bridge_end_to_end_second_write_emits_field_changed_payload() {
        // End-to-end integration: real EntityCache, subscribe, assert the
        // bridge's raw-path output shape for a field change.
        let (_dir, cache) = setup_cache().await;

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        let mut seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();

        let mut updated = Entity::new("tag", "bug");
        updated.set("tag_name", json!("Bug"));
        updated.set("color", json!("#00ff00"));
        cache.write(&updated).await.unwrap();

        let evt = rx.recv().await.unwrap();
        let EntityEvent::EntityChanged { changes, .. } = evt else {
            panic!("expected EntityChanged");
        };
        let out = raw_changed_event("tag", "bug", changes, &mut seen);
        match out {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "bug");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].field, "color");
                assert_eq!(changes[0].value, json!("#00ff00"));
            }
            other => panic!("expected EntityFieldChanged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bridge_end_to_end_attachment_emits_attachment_changed() {
        let (_dir, cache) = setup_cache().await;
        let _seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();

        cache.send_attachment_event("task", "01ABC-photo.png", false);

        let evt = rx.recv().await.unwrap();
        match evt {
            EntityEvent::AttachmentChanged {
                entity_type,
                filename,
                removed,
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(filename, "01ABC-photo.png");
                assert!(!removed);
            }
            other => panic!("expected AttachmentChanged, got {other:?}"),
        }
    }

    // ------------------------------------------------------------------------
    // Integration tests against a real KanbanContext — exercise enrichment
    // and fan-out end to end. These build a .kanban/ board on disk, run
    // `resolve_event` against real cache events, and assert the emitted
    // payloads carry enriched fields.
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn resolve_event_entity_created_for_task_includes_computed_fields() {
        // Regression guard for blocker 2: the `entity-created` payload must
        // carry computed fields (progress, virtual_tags, filter_tags, ready,
        // blocked_by, blocks) so the frontend renders the card correctly
        // without a follow-up refresh.
        let (_temp, ctx, cache) = setup_kanban_with_board().await;
        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);

        let add = AddTask::new("Hello")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let id = add["id"].as_str().unwrap().to_string();

        let evt = rx.recv().await.unwrap();
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;

        let primary = resolved
            .iter()
            .find(|e| matches!(e, WatchEvent::EntityCreated { id: eid, .. } if eid == &id))
            .expect("expected EntityCreated for the new task");
        let WatchEvent::EntityCreated { fields, .. } = primary else {
            unreachable!()
        };
        assert!(fields.contains_key("progress"), "progress must be enriched");
        assert!(
            fields.contains_key("virtual_tags"),
            "virtual_tags must be enriched"
        );
        assert!(
            fields.contains_key("filter_tags"),
            "filter_tags must be enriched"
        );
        assert!(fields.contains_key("ready"), "ready must be enriched");
    }

    #[tokio::test]
    async fn resolve_event_move_task_fans_out_to_dependent_blocked_by() {
        // Regression guard for blocker 1: moving task A into the terminal
        // column must emit a synthetic EntityFieldChanged for task B that
        // depends on A, so B's BLOCKED/READY badge refreshes.
        let (_temp, ctx, cache) = setup_kanban_with_board().await;

        // Add two tasks: B depends on A.
        let a = AddTask::new("A")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let a_id = a["id"].as_str().unwrap().to_string();
        let b = AddTask::new("B")
            .with_depends_on(vec![TaskId::from(a_id.as_str())])
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let b_id = b["id"].as_str().unwrap().to_string();

        // Prime bridge state: drain the two creation events so `seen`
        // reflects the real post-creation snapshot.
        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);
        // Emit a no-op update on B so the bridge's snapshot cache for B
        // records its *current* computed state (BLOCKED, ready=false).
        // fan_out_task_dependents primes the snapshot for every task it
        // visits, so any task event will do.
        UpdateTask::new(b_id.as_str())
            .with_title("B")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        while let Ok(evt) = rx.try_recv() {
            resolve_event(evt, ctx.as_ref(), &mut state).await;
        }

        // Move A to "done" — this should fan out an EntityFieldChanged for B.
        MoveTask::to_column(a_id.as_str(), "done")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();

        let evt = rx.recv().await.unwrap();
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;

        // There must be an event for B among the resolved outputs.
        let b_event = resolved.iter().find(|e| match e {
            WatchEvent::EntityFieldChanged { id, .. } => id == &b_id,
            _ => false,
        });
        assert!(
            b_event.is_some(),
            "expected fan-out EntityFieldChanged for dependent task B, got: {resolved:#?}"
        );
        let WatchEvent::EntityFieldChanged { changes, .. } = b_event.unwrap() else {
            unreachable!()
        };
        // B transitioned from blocked → ready. The synthetic event must
        // carry at minimum a fresh `ready` value.
        let change_fields: HashSet<&str> = changes.iter().map(|c| c.field.as_str()).collect();
        assert!(
            change_fields.contains("ready"),
            "fan-out must include `ready` in its changes; got: {change_fields:?}"
        );
    }

    #[tokio::test]
    async fn resolve_event_non_graph_change_skips_fanout() {
        // Editing a title doesn't change any dependent's computed state —
        // no synthetic fan-out events should be produced for other tasks.
        let (_temp, ctx, cache) = setup_kanban_with_board().await;

        let a = AddTask::new("A")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let a_id = a["id"].as_str().unwrap().to_string();
        let b = AddTask::new("B")
            .with_depends_on(vec![TaskId::from(a_id.as_str())])
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let b_id = b["id"].as_str().unwrap().to_string();

        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);
        while let Ok(evt) = rx.try_recv() {
            resolve_event(evt, ctx.as_ref(), &mut state).await;
        }

        // Edit A's title — nothing downstream should change.
        UpdateTask::new(a_id.as_str())
            .with_title("A renamed")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();

        let evt = rx.recv().await.unwrap();
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;
        // The primary event is for A. No synthetic event should target B.
        let b_event = resolved.iter().find(|e| match e {
            WatchEvent::EntityFieldChanged { id, .. } => id == &b_id,
            _ => false,
        });
        assert!(
            b_event.is_none(),
            "title edit must not fan out to dependents; got: {resolved:#?}"
        );
    }

    #[test]
    fn entity_field_change_converts_to_tauri_payload_field_change() {
        // The Tauri `FieldChange` is a re-export of the entity-layer struct
        // (same serde fields, same types) so the shim is gone. Guard against
        // accidental divergence.
        let entity_fc = EntityFieldChange {
            field: "title".into(),
            value: json!("T"),
        };
        // Same fields.
        assert_eq!(entity_fc.field, "title");
        assert_eq!(entity_fc.value, json!("T"));
        // `FieldChange` IS `EntityFieldChange` via pub use — no conversion step.
        let tauri_fc: FieldChange = entity_fc;
        assert_eq!(tauri_fc.field, "title");
    }
}
