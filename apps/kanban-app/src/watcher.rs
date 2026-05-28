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
use swissarmyhammer_perspectives::PerspectiveEvent;
use swissarmyhammer_views::ViewEvent;
use tauri::{Emitter, Manager};
use tokio::sync::broadcast;
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
    perspective_rx: Option<broadcast::Receiver<PerspectiveEvent>>,
    view_rx: Option<broadcast::Receiver<ViewEvent>>,
) {
    // Pre-populate BEFORE subscribing so no event lands between the snapshot
    // and the receiver. A write that lands *before* subscribe will be
    // reflected in the snapshot; the next observed write for that entity
    // (if any) correctly surfaces as a field-change.
    let seen = pre_populate_seen(&cache).await;
    let preloaded_entities = seen.len();
    let state = Arc::new(Mutex::new(BridgeState::new(seen)));
    let mut entity_rx = cache.subscribe();
    let mut perspective_rx = perspective_rx;
    let mut view_rx = view_rx;
    tracing::info!(
        board_path = %board_path,
        preloaded_entities,
        has_perspective_rx = perspective_rx.is_some(),
        has_view_rx = view_rx.is_some(),
        "entity-cache bridge started"
    );

    loop {
        let action = recv_next_event(&mut entity_rx, &mut perspective_rx, &mut view_rx).await;
        match action {
            BridgeAction::Entity(evt) => {
                process_cache_event(evt, &ctx, &state, &app, &board_path, &search_index).await;
            }
            BridgeAction::EntityLagged(n) => {
                tracing::warn!(board_path = %board_path, skipped = n,
                    "bridge lagged — search index and frontend may briefly drift");
            }
            BridgeAction::Perspective(evt) => {
                process_perspective_event(evt, &app, &board_path);
            }
            BridgeAction::PerspectiveLagged(n) => {
                tracing::warn!(board_path = %board_path, skipped = n,
                    "perspective bridge lagged");
            }
            BridgeAction::PerspectiveClosed => {
                tracing::info!(board_path = %board_path, "perspective event channel closed");
                perspective_rx = None;
            }
            BridgeAction::View(evt) => {
                process_view_event(evt, &app, &board_path);
            }
            BridgeAction::ViewLagged(n) => {
                tracing::warn!(board_path = %board_path, skipped = n,
                    "view bridge lagged");
            }
            BridgeAction::ViewClosed => {
                tracing::info!(board_path = %board_path, "view event channel closed");
                view_rx = None;
            }
            BridgeAction::Shutdown => {
                tracing::info!(board_path = %board_path, "entity-cache bridge stopped");
                break;
            }
        }
    }
}

/// Discriminant for the next event received by the bridge loop.
enum BridgeAction {
    Entity(EntityEvent),
    EntityLagged(u64),
    Perspective(PerspectiveEvent),
    PerspectiveLagged(u64),
    PerspectiveClosed,
    View(ViewEvent),
    ViewLagged(u64),
    ViewClosed,
    Shutdown,
}

/// Wait for the next event from any of the entity, perspective, or view channels.
///
/// Uses `tokio::select!` to multiplex all receivers. The perspective and view
/// channels are optional — when `None`, that arm pends forever so only the
/// remaining streams are processed.
async fn recv_next_event(
    entity_rx: &mut broadcast::Receiver<EntityEvent>,
    perspective_rx: &mut Option<broadcast::Receiver<PerspectiveEvent>>,
    view_rx: &mut Option<broadcast::Receiver<ViewEvent>>,
) -> BridgeAction {
    tokio::select! {
        entity_result = entity_rx.recv() => {
            match entity_result {
                Ok(evt) => BridgeAction::Entity(evt),
                Err(RecvError::Lagged(n)) => BridgeAction::EntityLagged(n),
                Err(RecvError::Closed) => BridgeAction::Shutdown,
            }
        }
        perspective_result = async {
            match perspective_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending().await,
            }
        } => {
            match perspective_result {
                Ok(evt) => BridgeAction::Perspective(evt),
                Err(RecvError::Lagged(n)) => BridgeAction::PerspectiveLagged(n),
                Err(RecvError::Closed) => BridgeAction::PerspectiveClosed,
            }
        }
        view_result = async {
            match view_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending().await,
            }
        } => {
            match view_result {
                Ok(evt) => BridgeAction::View(evt),
                Err(RecvError::Lagged(n)) => BridgeAction::ViewLagged(n),
                Err(RecvError::Closed) => BridgeAction::ViewClosed,
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
    let touched_task = cache_event_touches_task(&resolved);
    for watch_event in resolved {
        emit_watch_event(app, board_path, watch_event);
    }

    // A task create/remove/field-change can move tasks into or out of a
    // filtered window's perspective. `filtered_task_ids` is a snapshot taken
    // at `perspective.switch` time and nothing else refreshes it — so recompute
    // it here, server-side, and push a `ui-state-changed` event per window.
    if touched_task {
        recompute_and_emit_perspective_filters(ctx, app).await;
    }
}

/// Whether a resolved batch of `WatchEvent`s touches at least one `task`
/// entity — i.e. whether `process_cache_event` should fire a perspective
/// recompute.
///
/// A single cache event resolves to zero-or-more `WatchEvent`s (the primary
/// event plus synthetic dependency fan-out events). The recompute is the
/// expensive part, so the gate runs once over the whole batch.
fn cache_event_touches_task(resolved: &[WatchEvent]) -> bool {
    resolved.iter().any(watch_event_touches_task)
}

/// Whether a resolved `WatchEvent` concerns a `task` entity.
///
/// `AttachmentChanged` carries the owning entity type too; a task attachment
/// change does not move the task in or out of a filter, so only the three
/// entity-shaped events count.
fn watch_event_touches_task(evt: &WatchEvent) -> bool {
    matches!(
        evt,
        WatchEvent::EntityCreated { entity_type, .. }
        | WatchEvent::EntityRemoved { entity_type, .. }
        | WatchEvent::EntityFieldChanged { entity_type, .. }
            if entity_type == "task"
    )
}

/// Recompute every filtered window's perspective filter and emit a
/// `ui-state-changed` event for each window whose id list actually moved.
///
/// Reaches `UIState` through the app's managed [`AppState`] and mirrors the
/// `ui-state-changed` payload shape produced by `emit_ui_state_change_if_needed`
/// in `commands.rs` — `{ "kind": "perspective_switch", "state": <UIState> }`.
async fn recompute_and_emit_perspective_filters(ctx: &Arc<KanbanContext>, app: &tauri::AppHandle) {
    let app_state = app.state::<crate::state::AppState>();
    let ui_state = &app_state.ui_state;
    let changed_windows = recompute_perspective_filters(ctx.as_ref(), ui_state).await;
    if changed_windows.is_empty() {
        return;
    }
    let snapshot = serde_json::json!({
        "kind": "perspective_switch",
        "state": ui_state.to_json(),
    });
    for window_label in changed_windows {
        if let Some(window) = app.get_webview_window(&window_label) {
            if let Err(e) = window.emit("ui-state-changed", &snapshot) {
                tracing::warn!(
                    window_label, error = %e,
                    "failed to emit ui-state-changed after perspective recompute"
                );
            }
        }
    }
}

/// Re-evaluate the perspective filter for every window whose `filtered_task_ids`
/// is already populated (`Some`) and write the fresh id list back via
/// [`UIState::switch_perspective`].
///
/// Returns the label of every window whose id list actually changed —
/// `switch_perspective` is idempotent and yields `None` when the recompute
/// produces the same set, so unchanged windows are absent. The caller emits a
/// full `UIState` snapshot per changed window (mirroring
/// `emit_ui_state_change_if_needed`), so only the labels are needed here.
///
/// Windows whose `filtered_task_ids` is `None` (never switched perspective)
/// are skipped: `None` means "no filter, show all", and populating it would
/// change that window's behavior.
///
/// Windows that share an `active_perspective_id` share one filter, so each
/// distinct perspective is evaluated exactly once. The shared DSL evaluator
/// [`evaluate_perspective_filter`] keeps this in lockstep with
/// `perspective.switch`.
pub async fn recompute_perspective_filters(
    ctx: &KanbanContext,
    ui_state: &swissarmyhammer_ui_state::UIState,
) -> Vec<String> {
    use swissarmyhammer_kanban::commands::perspective_commands::evaluate_perspective_filter;

    // Collect the windows that hold a filter snapshot, grouped by perspective.
    let mut windows_by_perspective: HashMap<String, Vec<String>> = HashMap::new();
    for (label, window) in ui_state.all_windows() {
        if window.filtered_task_ids.is_none() {
            continue;
        }
        windows_by_perspective
            .entry(window.active_perspective_id.clone())
            .or_default()
            .push(label);
    }
    if windows_by_perspective.is_empty() {
        return Vec::new();
    }

    // Look up each distinct perspective's filter once.
    let perspective_filters = match load_perspective_filters(ctx, &windows_by_perspective).await {
        Some(filters) => filters,
        None => return Vec::new(),
    };

    let mut changed_windows = Vec::new();
    for (perspective_id, window_labels) in windows_by_perspective {
        let Some(filter) = perspective_filters.get(&perspective_id) else {
            // Perspective vanished (deleted concurrently) — leave its windows
            // untouched rather than forcing an empty filter on them.
            continue;
        };
        let new_ids = match evaluate_perspective_filter(ctx, filter).await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(
                    perspective_id, error = %e,
                    "perspective filter recompute failed — leaving snapshot stale"
                );
                continue;
            }
        };
        for label in window_labels {
            // `switch_perspective` takes the id list by value; the `clone()`
            // is required because multiple windows on the same perspective
            // each consume their own copy.
            if ui_state
                .switch_perspective(&label, &perspective_id, new_ids.clone())
                .is_some()
            {
                changed_windows.push(label);
            }
        }
    }
    changed_windows
}

/// Resolve the filter DSL string for each distinct perspective id referenced
/// by a filtered window.
///
/// Returns `None` when the perspective context is unavailable (the recompute
/// is then skipped entirely). A perspective id with no matching perspective is
/// simply absent from the returned map.
async fn load_perspective_filters(
    ctx: &KanbanContext,
    windows_by_perspective: &HashMap<String, Vec<String>>,
) -> Option<HashMap<String, String>> {
    let pctx = match ctx.perspective_context().await {
        Ok(pctx) => pctx,
        Err(e) => {
            tracing::warn!(error = %e, "perspective context unavailable — skipping recompute");
            return None;
        }
    };
    let pctx = pctx.read().await;
    let mut filters = HashMap::new();
    for perspective_id in windows_by_perspective.keys() {
        if let Some(perspective) = pctx.get_by_id(perspective_id) {
            filters.insert(
                perspective_id.clone(),
                perspective.filter.clone().unwrap_or_default(),
            );
        }
    }
    Some(filters)
}

/// Translate a `PerspectiveEvent` into a Tauri event and emit it.
///
/// Perspectives are not entities, but the frontend's `PerspectiveProvider`
/// already listens for `entity-field-changed` / `entity-created` /
/// `entity-removed` with `entity_type === "perspective"`. We map perspective
/// events into the same shape so the existing frontend refresh logic fires
/// without any React changes.
///
/// Creates emit `entity-created`, updates emit `entity-field-changed`, and
/// deletes emit `entity-removed` — matching the entity bridge's contract.
fn process_perspective_event(evt: PerspectiveEvent, app: &tauri::AppHandle, board_path: &str) {
    let watch_event = match evt {
        PerspectiveEvent::PerspectiveChanged {
            id,
            changed_fields,
            is_create,
        } => {
            if is_create {
                // The frontend only needs the event signal to trigger a
                // perspective list refresh — field values are re-fetched
                // from the backend via `perspective.list`.
                let fields = changed_fields
                    .into_iter()
                    .map(|field| (field, serde_json::Value::Null))
                    .collect();
                WatchEvent::EntityCreated {
                    entity_type: "perspective".to_string(),
                    id,
                    fields,
                }
            } else {
                let changes = changed_fields
                    .into_iter()
                    .map(|field| FieldChange {
                        field,
                        // The frontend only needs to know *which* fields changed
                        // to trigger a perspective list refresh — the actual value
                        // is re-fetched from the backend via `perspective.list`.
                        value: serde_json::Value::Null,
                    })
                    .collect();
                WatchEvent::EntityFieldChanged {
                    entity_type: "perspective".to_string(),
                    id,
                    changes,
                }
            }
        }
        PerspectiveEvent::PerspectiveDeleted { id } => WatchEvent::EntityRemoved {
            entity_type: "perspective".to_string(),
            id,
        },
    };
    emit_watch_event(app, board_path, watch_event);
}

/// Translate a `ViewEvent` into a Tauri event and emit it.
///
/// Views are not entities, but the frontend listens for
/// `entity-created` / `entity-field-changed` / `entity-removed` with
/// `entity_type === "view"` to refresh the view list. We map view events
/// into the same shape so the existing frontend refresh logic fires without
/// any React changes.
///
/// Creates emit `entity-created`, updates emit `entity-field-changed`, and
/// deletes emit `entity-removed` — matching the entity bridge's contract.
/// `is_create` on `ViewChanged` selects between the create / update cases;
/// `reload_from_disk` always emits `is_create: false`, so undo-of-delete
/// surfaces as `entity-field-changed` and the frontend re-fetches the view list.
fn process_view_event(evt: ViewEvent, app: &tauri::AppHandle, board_path: &str) {
    let watch_event = view_event_to_watch_event(evt);
    emit_watch_event(app, board_path, watch_event);
}

/// Pure mapping from a `ViewEvent` to the matching `WatchEvent` payload.
///
/// Extracted from [`process_view_event`] so unit tests can verify the
/// `is_create` → `EntityCreated` vs `EntityFieldChanged` branch without
/// constructing a real Tauri `AppHandle`. The bridge contract is verified
/// here: undo-of-delete (where `reload_from_disk` emits `is_create: false`)
/// must map to `EntityFieldChanged`, not `EntityCreated`.
fn view_event_to_watch_event(evt: ViewEvent) -> WatchEvent {
    match evt {
        ViewEvent::ViewChanged {
            id,
            changed_fields,
            is_create,
        } => {
            if is_create {
                // The frontend only needs the event signal to trigger a view
                // list refresh — field values are re-fetched from the backend.
                let fields = changed_fields
                    .into_iter()
                    .map(|field| (field, serde_json::Value::Null))
                    .collect();
                WatchEvent::EntityCreated {
                    entity_type: "view".to_string(),
                    id,
                    fields,
                }
            } else {
                let changes = changed_fields
                    .into_iter()
                    .map(|field| FieldChange {
                        field,
                        // The frontend only needs to know *which* fields
                        // changed to trigger a refresh — the actual value is
                        // re-fetched from the backend.
                        value: serde_json::Value::Null,
                    })
                    .collect();
                WatchEvent::EntityFieldChanged {
                    entity_type: "view".to_string(),
                    id,
                    changes,
                }
            }
        }
        ViewEvent::ViewDeleted { id } => WatchEvent::EntityRemoved {
            entity_type: "view".to_string(),
            id,
        },
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

    // -----------------------------------------------------------------------
    // View event → WatchEvent mapping tests (the bridge contract for views).
    //
    // These exercise `view_event_to_watch_event` directly so we don't need a
    // real `tauri::AppHandle`. They pin the contract from the task description:
    // ViewChanged{is_create=true} → EntityCreated, ViewChanged{is_create=false}
    // → EntityFieldChanged, ViewDeleted → EntityRemoved. All carry
    // entity_type="view".
    // -----------------------------------------------------------------------

    #[test]
    fn view_event_to_watch_event_create_maps_to_entity_created() {
        let evt = ViewEvent::ViewChanged {
            id: "01VIEW".into(),
            changed_fields: vec!["name".into(), "kind".into()],
            is_create: true,
        };
        match view_event_to_watch_event(evt) {
            WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            } => {
                assert_eq!(entity_type, "view");
                assert_eq!(id, "01VIEW");
                assert!(fields.contains_key("name"));
                assert!(fields.contains_key("kind"));
            }
            other => panic!("expected EntityCreated, got {other:?}"),
        }
    }

    #[test]
    fn view_event_to_watch_event_update_maps_to_entity_field_changed() {
        let evt = ViewEvent::ViewChanged {
            id: "01VIEW".into(),
            changed_fields: vec!["kind".into()],
            is_create: false,
        };
        match view_event_to_watch_event(evt) {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
            } => {
                assert_eq!(entity_type, "view");
                assert_eq!(id, "01VIEW");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].field, "kind");
            }
            other => panic!("expected EntityFieldChanged, got {other:?}"),
        }
    }

    #[test]
    fn view_event_to_watch_event_delete_maps_to_entity_removed() {
        let evt = ViewEvent::ViewDeleted {
            id: "01VIEW".into(),
        };
        match view_event_to_watch_event(evt) {
            WatchEvent::EntityRemoved { entity_type, id } => {
                assert_eq!(entity_type, "view");
                assert_eq!(id, "01VIEW");
            }
            other => panic!("expected EntityRemoved, got {other:?}"),
        }
    }

    /// End-to-end mapping for the full create → delete → undo-of-delete cycle
    /// the views crate emits. Pins the bridge contract documented in the task:
    /// the third event (undo-of-delete) is `EntityFieldChanged`, not
    /// `EntityCreated`, because `reload_from_disk` always emits
    /// `is_create: false`.
    #[test]
    fn bridge_routes_view_undo_of_delete() {
        // Step 1: create — bridge must emit EntityCreated.
        let create = view_event_to_watch_event(ViewEvent::ViewChanged {
            id: "01VIEW".into(),
            changed_fields: vec!["name".into()],
            is_create: true,
        });
        assert!(matches!(
            create,
            WatchEvent::EntityCreated { ref entity_type, .. } if entity_type == "view"
        ));

        // Step 2: delete — bridge must emit EntityRemoved.
        let delete = view_event_to_watch_event(ViewEvent::ViewDeleted {
            id: "01VIEW".into(),
        });
        assert!(matches!(
            delete,
            WatchEvent::EntityRemoved { ref entity_type, .. } if entity_type == "view"
        ));

        // Step 3: undo-of-delete — `reload_from_disk` emits
        // ViewChanged{is_create=false}, which must map to EntityFieldChanged
        // (NOT EntityCreated). This is the key bridge contract for views.
        let undo = view_event_to_watch_event(ViewEvent::ViewChanged {
            id: "01VIEW".into(),
            changed_fields: vec![],
            is_create: false,
        });
        assert!(
            matches!(
                undo,
                WatchEvent::EntityFieldChanged { ref entity_type, .. } if entity_type == "view"
            ),
            "undo-of-delete must surface as EntityFieldChanged, got {undo:?}"
        );
    }

    /// End-to-end through the bridge: write → delete → undo-of-delete. The
    /// bridge must emit `entity-created`, `entity-removed`, `entity-created`
    /// in order. The third event is `entity-created` (NOT
    /// `entity-field-changed`) because deletion clears the bridge's `seen`
    /// set, so when undo restores the file the post-undo `EntityChanged`
    /// finds the key absent and classifies it as a fresh create. This
    /// pins the cache-bridge contract that single-changelog (card
    /// 01KQ5FJ0VXEQZVKHZBN49Q5GFS) relies on for the delete-undo
    /// round-trip.
    #[tokio::test]
    async fn bridge_routes_undo_of_delete_to_entity_created() {
        use swissarmyhammer_entity::EntityTypeStore;
        use swissarmyhammer_store::{StoreContext, StoreHandle};

        // Full production-like wiring: kanban context, store handles for
        // every entity type, shared StoreContext, attached EntityCache.
        let (_temp, ctx, cache) = setup_kanban_with_board().await;
        let ectx = ctx.entity_context().await.unwrap();

        let store_context = Arc::new(StoreContext::new(ctx.root().to_path_buf()));
        ectx.set_store_context(Arc::clone(&store_context));

        let fields_ctx = ectx.fields();
        for entity_def in fields_ctx.all_entities() {
            let entity_type = entity_def.name.as_str();
            let field_defs: Vec<_> = fields_ctx
                .fields_for_entity(entity_type)
                .into_iter()
                .cloned()
                .collect();
            let store = EntityTypeStore::new(
                ectx.entity_dir(entity_type),
                entity_type,
                Arc::new(entity_def.clone()),
                Arc::new(field_defs),
            );
            let handle = Arc::new(StoreHandle::new(Arc::new(store)));
            ectx.register_store(entity_type, Arc::clone(&handle)).await;
            store_context.register(handle).await;
        }

        // Pre-populate seen from the board's existing entities (columns,
        // board) so the bridge has the correct baseline before our write.
        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);

        // Step 1: create a tag. The bridge must classify this as
        // `entity-created`.
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let evt = rx.recv().await.expect("write must emit");
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;
        let primary = resolved
            .first()
            .expect("resolve_event must return at least one event");
        assert!(
            matches!(primary, WatchEvent::EntityCreated { id, .. } if id == "bug"),
            "step 1 (write) must surface as EntityCreated, got {primary:?}"
        );

        // Step 2: delete the tag. The bridge must classify this as
        // `entity-removed` and clear `seen`.
        ectx.delete("tag", "bug").await.unwrap();
        let evt = rx.recv().await.expect("delete must emit");
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;
        let primary = resolved
            .first()
            .expect("resolve_event must return at least one event");
        assert!(
            matches!(primary, WatchEvent::EntityRemoved { id, .. } if id == "bug"),
            "step 2 (delete) must surface as EntityRemoved, got {primary:?}"
        );

        // Step 3: undo the delete. The store layer restores the file and
        // the command-layer glue (mirrored here) calls
        // `sync_entity_cache_from_disk`, which fires an `EntityChanged`
        // broadcast. Because step 2 cleared the bridge's seen-set, this
        // event must surface as `entity-created` again, NOT as
        // `entity-field-changed`.
        let outcome = store_context.undo().await.expect("undo must succeed");
        ectx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
            .await;

        let evt = rx.recv().await.expect("undo must emit");
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;
        let primary = resolved
            .first()
            .expect("resolve_event must return at least one event");
        assert!(
            matches!(primary, WatchEvent::EntityCreated { id, .. } if id == "bug"),
            "step 3 (undo of delete) must surface as EntityCreated (not \
             EntityFieldChanged) — the bridge's seen-set was cleared on \
             delete and the post-undo EntityChanged finds the key absent; \
             got {primary:?}"
        );
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

    // ------------------------------------------------------------------------
    // Inspector-edit propagation tests — regression guards for the bug where
    // editing a non-hardcoded field in the inspector failed to refresh the
    // card view. The fix lives downstream of the cache/bridge but these
    // tests confirm the bridge always carries the raw field change through
    // to `WatchEvent::EntityFieldChanged.changes` so the frontend never has
    // to refetch.
    // ------------------------------------------------------------------------

    /// Editing a task field via `UpdateEntityField` must emit a
    /// `WatchEvent::EntityFieldChanged` whose `changes` vector contains the
    /// raw field that was edited. Bisects whether the bug is upstream of the
    /// frontend store: if this test fails, the bridge layer is dropping the
    /// raw change.
    #[tokio::test]
    async fn update_field_emits_raw_change_for_task_title() {
        use swissarmyhammer_kanban::entity::UpdateEntityField;

        let (_temp, ctx, cache) = setup_kanban_with_board().await;
        let add = AddTask::new("Original")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add["id"].as_str().unwrap().to_string();

        // Prime bridge state and drain any pre-existing events.
        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);
        while let Ok(evt) = rx.try_recv() {
            resolve_event(evt, ctx.as_ref(), &mut state).await;
        }

        // Dispatch the same command the inspector uses to commit a field edit.
        UpdateEntityField::new("task", &task_id, "title", json!("Renamed"))
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();

        let evt = rx.recv().await.unwrap();
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;

        let primary = resolved
            .iter()
            .find(|e| matches!(e, WatchEvent::EntityFieldChanged { id, .. } if id == &task_id))
            .expect("expected EntityFieldChanged for the edited task");
        let WatchEvent::EntityFieldChanged { changes, .. } = primary else {
            unreachable!()
        };
        let title_change = changes.iter().find(|c| c.field == "title");
        assert!(
            title_change.is_some(),
            "EntityFieldChanged.changes must include the edited `title` field; got: {changes:#?}"
        );
        assert_eq!(title_change.unwrap().value, json!("Renamed"));
    }

    /// Editing a non-task entity field via `UpdateEntityField` must also
    /// emit the raw change. Confirms the propagation works for tag-type
    /// entities (no task fan-out, no enrichment).
    #[tokio::test]
    async fn update_field_emits_raw_change_for_tag_color() {
        use swissarmyhammer_kanban::entity::UpdateEntityField;
        use swissarmyhammer_kanban::tag::AddTag;

        let (_temp, ctx, cache) = setup_kanban_with_board().await;

        // Seed the tag via the real `AddTag` command path — same fidelity
        // as the task variant which uses `AddTask`. AddTag generates a ULID
        // for the tag's stable id; capture it for the field-update step.
        let add = AddTag::new("bug")
            .with_color("ff0000")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let tag_id = add["id"].as_str().unwrap().to_string();

        // Prime bridge state and drain any pre-existing events.
        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);
        while let Ok(evt) = rx.try_recv() {
            resolve_event(evt, ctx.as_ref(), &mut state).await;
        }

        UpdateEntityField::new("tag", &tag_id, "color", json!("#00ff00"))
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();

        let evt = rx.recv().await.unwrap();
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;

        let primary = resolved
            .iter()
            .find(|e| matches!(e, WatchEvent::EntityFieldChanged { id, .. } if id == &tag_id))
            .expect("expected EntityFieldChanged for the edited tag");
        let WatchEvent::EntityFieldChanged { changes, .. } = primary else {
            unreachable!()
        };
        let color_change = changes.iter().find(|c| c.field == "color");
        assert!(
            color_change.is_some(),
            "EntityFieldChanged.changes must include the edited `color` field; got: {changes:#?}"
        );
        assert_eq!(color_change.unwrap().value, json!("#00ff00"));
    }

    // ------------------------------------------------------------------------
    // Perspective-filter recompute tests — regression guards for the bug where
    // a task created or changed by an external process never appeared on the
    // board because the window's `filtered_task_ids` snapshot went stale.
    //
    // `recompute_perspective_filters` is the bridge-side hook: given the
    // current `KanbanContext` and `UIState`, it re-evaluates each filtered
    // window's perspective filter and returns the windows whose id list
    // actually moved.
    // ------------------------------------------------------------------------

    use swissarmyhammer_ui_state::UIState;

    /// Add a `#bug` perspective to the board and return its id.
    async fn add_bug_perspective(ctx: &Arc<KanbanContext>) -> String {
        use swissarmyhammer_kanban::perspective::AddPerspective;
        let result = AddPerspective::new("Bugs", "board")
            .with_filter("#bug")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    /// Add a task whose description carries `#bug` so the enrichment pipeline
    /// lifts it into `filter_tags` (what the `#bug` DSL predicate reads).
    async fn add_bug_task(ctx: &Arc<KanbanContext>, title: &str) -> String {
        let task = AddTask::new(title)
            .with_description("#bug")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        task["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn recompute_perspective_filters_picks_up_new_matching_task() {
        // A window switched to a `#bug` perspective holds a snapshot of the
        // matching ids. After an external process adds a new `#bug` task, the
        // bridge recompute must refresh that snapshot and surface a
        // PerspectiveSwitch change for the window.
        let (_temp, ctx, _cache) = setup_kanban_with_board().await;
        let pid = add_bug_perspective(&ctx).await;
        let t1 = add_bug_task(&ctx, "First bug").await;

        let ui = UIState::new();
        // Window starts on the perspective with only the first task in scope.
        ui.switch_perspective("main", &pid, vec![t1.clone()]);

        // External process adds another `#bug` task.
        let t2 = add_bug_task(&ctx, "Second bug").await;

        let changed_windows = recompute_perspective_filters(ctx.as_ref(), &ui).await;

        assert_eq!(
            changed_windows,
            vec!["main".to_string()],
            "exactly the `main` window should change"
        );
        // UIState itself must carry the refreshed list.
        let stored = ui.filtered_task_ids("main");
        assert!(stored.contains(&t1));
        assert!(
            stored.contains(&t2),
            "the newly created task must be in the refreshed id list"
        );
    }

    #[tokio::test]
    async fn recompute_perspective_filters_leaves_never_switched_window_alone() {
        // A window that has never switched perspective holds
        // `filtered_task_ids == None` ("no filter, show all"). The bridge must
        // not populate it — that would change its behavior.
        let (_temp, ctx, _cache) = setup_kanban_with_board().await;
        let _pid = add_bug_perspective(&ctx).await;
        let _t1 = add_bug_task(&ctx, "A bug").await;

        let ui = UIState::new();
        // No switch_perspective call — the `main` window's filtered_task_ids
        // stays `None`.

        let changed_windows = recompute_perspective_filters(ctx.as_ref(), &ui).await;

        assert!(
            changed_windows.is_empty(),
            "a never-switched window must not be recomputed"
        );
        assert!(
            ui.get_window_state("main")
                .and_then(|ws| ws.filtered_task_ids)
                .is_none(),
            "filtered_task_ids must remain None for a never-switched window"
        );
    }

    #[tokio::test]
    async fn recompute_perspective_filters_dedupes_shared_perspective() {
        // Two windows on the SAME perspective share one filter. The recompute
        // evaluates that filter once and applies the fresh id list to both.
        let (_temp, ctx, _cache) = setup_kanban_with_board().await;
        let pid = add_bug_perspective(&ctx).await;
        let t1 = add_bug_task(&ctx, "First bug").await;

        let ui = UIState::new();
        ui.switch_perspective("main", &pid, vec![t1.clone()]);
        ui.switch_perspective("secondary", &pid, vec![t1.clone()]);

        let t2 = add_bug_task(&ctx, "Second bug").await;

        let mut changed_windows = recompute_perspective_filters(ctx.as_ref(), &ui).await;
        changed_windows.sort();

        assert_eq!(
            changed_windows,
            vec!["main".to_string(), "secondary".to_string()],
            "both windows must be refreshed"
        );
        for label in ["main", "secondary"] {
            let stored = ui.filtered_task_ids(label);
            assert!(
                stored.contains(&t2),
                "window `{label}` must see the new task"
            );
        }
    }

    #[tokio::test]
    async fn recompute_perspective_filters_drops_removed_task() {
        // When a task that was in scope is deleted, the recompute must remove
        // it from the window's filtered id list.
        let (_temp, ctx, _cache) = setup_kanban_with_board().await;
        let pid = add_bug_perspective(&ctx).await;
        let t1 = add_bug_task(&ctx, "First bug").await;
        let t2 = add_bug_task(&ctx, "Second bug").await;

        let ui = UIState::new();
        ui.switch_perspective("main", &pid, vec![t1.clone(), t2.clone()]);

        // External process deletes the second task.
        let ectx = ctx.entity_context().await.unwrap();
        ectx.delete("task", &t2).await.unwrap();

        let changed_windows = recompute_perspective_filters(ctx.as_ref(), &ui).await;

        assert_eq!(changed_windows, vec!["main".to_string()]);
        let stored = ui.filtered_task_ids("main");
        assert!(stored.contains(&t1));
        assert!(
            !stored.contains(&t2),
            "the removed task must drop out of the filtered id list"
        );
    }

    #[tokio::test]
    async fn cache_event_gate_fires_for_task_events_only() {
        // `process_cache_event` only fires the perspective recompute when its
        // `cache_event_touches_task` gate says the resolved batch touched a
        // `task`. That gate runs on the *resolved* `WatchEvent`s produced by
        // `resolve_event`. Drive real cache events for a task and for a
        // non-task (`tag`) entity end to end and assert the gate verdict on
        // each — a regression in `watch_event_touches_task` (e.g. matching
        // `tag`, or dropping the `task` guard) would flip one of these.
        let (_temp, ctx, cache) = setup_kanban_with_board().await;

        let seen = pre_populate_seen(&cache).await;
        let mut rx = cache.subscribe();
        let mut state = BridgeState::new(seen);

        // A real task creation by the kanban layer.
        let task = AddTask::new("A task")
            .execute(ctx.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        let evt = rx.recv().await.expect("task write must emit");
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;
        assert!(
            resolved.iter().any(
                |e| matches!(e, WatchEvent::EntityCreated { entity_type, id, .. }
                    if entity_type == "task" && id == &task_id)
            ),
            "resolve_event must surface the new task as a task EntityCreated"
        );
        assert!(
            cache_event_touches_task(&resolved),
            "a resolved task cache event must trip the recompute gate"
        );

        // A real tag creation — a non-task entity. The gate must NOT trip.
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let evt = rx.recv().await.expect("tag write must emit");
        let resolved = resolve_event(evt, ctx.as_ref(), &mut state).await;
        assert!(
            !resolved.is_empty(),
            "resolve_event must surface the tag event"
        );
        assert!(
            resolved.iter().all(|e| !watch_event_touches_task(e)),
            "no resolved tag event may be classified as touching a task"
        );
        assert!(
            !cache_event_touches_task(&resolved),
            "a resolved non-task cache event must not trip the recompute gate"
        );
    }
}
