//! In-memory entity cache with content hashing.
//!
//! Provides an `EntityCache` that wraps an `EntityContext` and keeps entities
//! in memory, indexed by `(entity_type, id)`. Each cached entry stores a
//! content hash (computed from serialized YAML of the fields) and a monotonic
//! version counter. Writes delegate to disk through the underlying
//! `EntityContext` and then update the cache.
//!
//! Alongside the primary entity map, the cache maintains two parallel
//! secondary maps keyed on the same `(entity_type, id)` tuple:
//!
//! 1. **Compute-input cache** — the `_changelog` JSON array and the
//!    `_file_created` RFC3339 timestamp. These pseudo-fields are injected
//!    into entities on every `list()` / `read()` call when a computed field
//!    declares a dependency on them; sourcing them from disk per-entity
//!    dominates the cost on large boards.
//! 2. **Derived-output cache** — the already-computed values of every
//!    computed field on the entity (e.g. `created`, `updated`, `tags`,
//!    `status_date` on `task`). With the compute-input cache in place, the
//!    remaining cost on a warm `list_task` pass is the compute-engine
//!    derivation itself — including aggregate derivations that query
//!    other entity types. Memoizing the outputs collapses that per-entity
//!    CPU cost to a single HashMap lookup + clone.
//!
//! Both secondary caches share a single invalidation epoch and are cleared
//! together by every mutation path (`write`, `delete`, `evict`,
//! `refresh_from_disk`, `archive`, `unarchive`) — the inputs and the
//! outputs go stale together, and keeping them in lockstep avoids the
//! correctness hazard of serving a fresh input alongside a stale output
//! (or vice versa).
//!
//! Cross-entity invalidation: aggregate derivations may query other
//! entity types through the `EntityQueryFn`, so their outputs can change
//! even without a mutation on the owning entity. Every mutation path
//! consults [`FieldsContext::entity_types_depending_on`] and also clears
//! the derived-output slots of every entity type whose aggregate fields
//! declare the mutated type in their `depends_on` list. Aggregate
//! derivations whose field definitions omit `depends_on` fall back to
//! entity-local invalidation only, which is the same pre-Option-B
//! behavior as Option A; declaring `depends_on` in the field definition
//! is what opts an aggregate into correct cross-entity invalidation.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use indexmap::IndexMap;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

use crate::context::EntityContext;
use crate::entity::Entity;
use crate::error::Result;
use crate::events::{EntityEvent, FieldChange};
use swissarmyhammer_store::UndoEntryId;

/// A cached entity with its content hash and version stamp.
#[derive(Debug, Clone)]
pub struct CachedEntity {
    /// The entity data.
    pub entity: Entity,
    /// Content hash computed from serialized YAML of `entity.fields`.
    pub hash: u64,
    /// Monotonically increasing version (bumped on each write).
    pub version: u64,
}

/// Cached compute-dependency inputs for a single entity.
///
/// Both fields are filled lazily on first `get_or_load_*` call and cleared
/// whenever the owning `(entity_type, id)` is invalidated. Storing the
/// already-serialized `serde_json::Value` that `inject_compute_dependencies`
/// feeds into `entity.fields` avoids the per-entry `serde_json::to_value`
/// conversion on every list pass.
#[derive(Debug, Clone, Default)]
struct CachedComputeInputs {
    /// The `_changelog` pseudo-field: a `Value::Array` of JSON-serialized
    /// `ChangeEntry` objects.
    changelog: Option<serde_json::Value>,
    /// The `_file_created` pseudo-field: a `Value::String` holding an
    /// RFC3339 timestamp, or `Value::Null` when the file could not be stat'd.
    file_created: Option<serde_json::Value>,
}

/// Cached compute-field outputs for a single entity.
///
/// Populated lazily on the first compute pass that runs against an entity
/// and cleared (not removed) whenever the owning `(entity_type, id)` is
/// invalidated. A `Some(map)` value means the entire set of computed-field
/// outputs for the entity is memoized and can be copied straight into
/// `entity.fields` on the next `derive_compute_fields` call; a `None`
/// value (or a missing map entry) means the derivation must run.
///
/// Both simple-derivation outputs and aggregate outputs live here.
/// Aggregate correctness depends on cross-entity invalidation via
/// [`EntityCache::invalidate_cross_type_derived`] — when an entity of
/// type T changes, every derived-output slot for entity types whose
/// aggregates declare `depends_on: [T]` is cleared alongside the
/// per-entity invalidation.
#[derive(Debug, Clone, Default)]
struct CachedDerived {
    /// Map from computed-field name to its last computed value.
    /// `None` means "no memoization yet" (cold slot); a `Some(map)` is the
    /// authoritative set of computed-field outputs for the entity.
    outputs: Option<HashMap<String, serde_json::Value>>,
}

/// In-memory entity cache backed by an `EntityContext`.
///
/// All reads come from the in-memory map. Writes go through to disk
/// via `EntityContext` first, then update the cache. Content hashing lets
/// callers detect whether an entity actually changed.
/// Default capacity for the broadcast event channel.
const EVENT_CHANNEL_CAPACITY: usize = 256;

pub struct EntityCache {
    inner: Arc<EntityContext>,
    /// Ordered map of cached entities keyed by `(entity_type, id)`.
    ///
    /// `IndexMap` preserves insertion order so `get_all(entity_type)`
    /// returns entities in the same order the cache received them —
    /// either `read_entity_dir` order during `load_all` or chronological
    /// write order for entities added at runtime. This matches the
    /// ordering semantics of the pre-cache disk-backed `list()` and keeps
    /// consumers that rely on stable iteration (like column-order
    /// enumeration in `NextTask`) working.
    cache: RwLock<IndexMap<(String, String), CachedEntity>>,
    /// Secondary map of per-entity compute inputs — the `_changelog` array
    /// and `_file_created` timestamp used by computed fields. Maintained
    /// independently of the primary map so populating an input on a read
    /// path never requires taking the primary-map write lock.
    ///
    /// Entries are cleared (for live entities) or removed (for deleted
    /// entities) by every mutation path that touches the primary map —
    /// `write`, `delete`, `evict`, `archive`, `unarchive`,
    /// `refresh_from_disk` — so stale values never survive past a change
    /// to the entity.
    compute_inputs: RwLock<HashMap<(String, String), CachedComputeInputs>>,
    /// Tertiary map of per-entity computed-field outputs. See
    /// [`CachedDerived`] for what lives here and how aggregate
    /// cross-entity invalidation keeps cached aggregate outputs fresh.
    /// Invalidation tracks the compute-input cache exactly — same
    /// mutation paths, same epoch.
    derived: RwLock<HashMap<(String, String), CachedDerived>>,
    /// Monotonic invalidation epoch for both secondary caches
    /// (`compute_inputs` and `derived`).
    ///
    /// Bumped by every invalidation and purge. Loaders capture the epoch
    /// on entry and refuse to memoize their loaded value if the epoch
    /// changed while they were loading — closing the race where an
    /// invalidation fires between a loader's read-lock drop and its
    /// write-lock acquire. Shared across both secondary caches because
    /// inputs and outputs are always invalidated together — there is no
    /// path that leaves one fresh while staling the other — and a
    /// single epoch keeps the invariant that "any stale loader drops
    /// its value" trivially true for both.
    ///
    /// Trade-off: the epoch is global across all `(type, id)` pairs, not
    /// per-key. A single invalidation on one entity causes every
    /// in-flight loader across the 64-way `buffer_unordered` list
    /// fan-out to abandon its memoization — under mixed writes-during-list
    /// workloads this can briefly thrash the cache as concurrent loaders
    /// each drop their freshly-loaded value on the floor and the next
    /// read has to redo the work. A per-key epoch stored inside
    /// `CachedComputeInputs` / `CachedDerived` would localize the
    /// invalidation and avoid the thrash, at the cost of an extra
    /// `AtomicU64` per cached entry and a sharper map-lookup dance
    /// inside the loader. Left global here deliberately: the cache is
    /// dominated by steady-state list passes with no concurrent writes,
    /// and the thrash window is bounded by a single round-trip per
    /// dropped loader.
    cache_epoch: AtomicU64,
    version_counter: AtomicU64,
    event_sender: broadcast::Sender<EntityEvent>,
}

/// Compute a deterministic content hash for an entity's fields.
///
/// Copies fields into a `BTreeMap` for sorted-key order, then serializes
/// to YAML via `serde_yaml_ng::to_string` and feeds those bytes into a
/// `DefaultHasher`. The sort step eliminates HashMap iteration-order
/// nondeterminism so the hash is stable across calls.
fn hash_entity(entity: &Entity) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::collections::BTreeMap;

    let sorted: BTreeMap<_, _> = entity.fields.iter().collect();
    let yaml = serde_yaml_ng::to_string(&sorted).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    yaml.hash(&mut hasher);
    hasher.finish()
}

/// Compute the field-level diff between an optional old entity and a new one.
///
/// The result lists every field whose value changed:
///
/// - When `old` is `None` (brand-new entity, no prior cache entry) every field
///   in `new.fields` is emitted with its value.
/// - For fields present in `new`: emit `{field, new_value}` when the field is
///   missing from `old` or holds a different value.
/// - For fields only present in `old`: emit `{field, Value::Null}` to signal
///   removal — this matches the frontend's existing patch semantics where
///   `null` at a field position means the field was deleted.
///
/// Fields with identical values on both sides are omitted. The returned order
/// is unspecified (follows `HashMap` iteration order) — callers should not
/// rely on it.
fn diff(old: Option<&Entity>, new: &Entity) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    match old {
        None => {
            // Brand-new entity — every field is a change.
            for (field, value) in &new.fields {
                changes.push(FieldChange {
                    field: field.clone(),
                    value: value.clone(),
                });
            }
        }
        Some(old_entity) => {
            // Additions and modifications.
            for (field, new_value) in &new.fields {
                match old_entity.fields.get(field) {
                    Some(old_value) if old_value == new_value => {}
                    _ => changes.push(FieldChange {
                        field: field.clone(),
                        value: new_value.clone(),
                    }),
                }
            }
            // Removals.
            for field in old_entity.fields.keys() {
                if !new.fields.contains_key(field) {
                    changes.push(FieldChange {
                        field: field.clone(),
                        value: serde_json::Value::Null,
                    });
                }
            }
        }
    }

    changes
}

impl EntityCache {
    /// Create a new cache wrapping the given `EntityContext`.
    ///
    /// Accepts `Arc<EntityContext>` so the same context can be shared between
    /// the cache (which uses it for disk I/O on misses and write-through) and
    /// callers like `KanbanContext` that expose the cache-wired context
    /// directly. The cache never holds exclusive ownership.
    pub fn new(inner: Arc<EntityContext>) -> Self {
        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            inner,
            cache: RwLock::new(IndexMap::new()),
            compute_inputs: RwLock::new(HashMap::new()),
            derived: RwLock::new(HashMap::new()),
            cache_epoch: AtomicU64::new(0),
            version_counter: AtomicU64::new(1),
            event_sender,
        }
    }

    /// Return a reference to the underlying `EntityContext`.
    pub fn inner(&self) -> &EntityContext {
        &self.inner
    }

    /// Subscribe to entity change events.
    ///
    /// Returns a receiver that will get all events emitted after this call.
    /// Missed events (due to slow consumption) result in `RecvError::Lagged`.
    pub fn subscribe(&self) -> broadcast::Receiver<EntityEvent> {
        self.event_sender.subscribe()
    }

    /// Emit an `AttachmentChanged` event on the broadcast channel.
    ///
    /// Attachments are not entities and do not live in the cache map — this
    /// helper exists so the `EntityWatcher` can forward attachment-file events
    /// through the same broadcast channel subscribers already consume.
    ///
    /// `entity_type` is the owner type (e.g. `"task"`), `filename` is the
    /// stored filename including extension, and `removed` should be `true`
    /// when the file no longer exists after the event and `false` for
    /// create/modify of an existing file.
    pub fn send_attachment_event(&self, entity_type: &str, filename: &str, removed: bool) {
        let _ = self.event_sender.send(EntityEvent::AttachmentChanged {
            entity_type: entity_type.to_string(),
            filename: filename.to_string(),
            removed,
        });
    }

    /// Bump the version counter and return the new value.
    fn bump_version(&self) -> u64 {
        self.version_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Load all entities of a given type from disk into the cache.
    ///
    /// Calls `inner.list()` to read every entity file, computes a content hash
    /// for each, and inserts them into the cache. Existing entries of the same
    /// type are overwritten.
    pub async fn load_all(&self, entity_type: &str) -> Result<()> {
        // Read raw on-disk entities — compute fields (especially aggregate
        // ones like `parse-body-tags` that query sibling entity types) must
        // be re-evaluated on every read out of the cache, not frozen at
        // preload time. Storing post-compute entities would turn cross-type
        // writes into silent staleness bombs.
        let entities = self.inner.list_raw_internal(entity_type).await?;
        let mut map = self.cache.write().await;
        for entity in entities {
            let hash = hash_entity(&entity);
            let version = self.bump_version();
            let key = (entity_type.to_string(), entity.id.to_string());
            map.insert(
                key,
                CachedEntity {
                    entity,
                    hash,
                    version,
                },
            );
        }
        Ok(())
    }

    /// Get a clone of a cached entity by type and id.
    ///
    /// Returns `None` if the entity is not in the cache. Never reads from disk.
    pub async fn get(&self, entity_type: &str, id: &str) -> Option<Entity> {
        let map = self.cache.read().await;
        map.get(&(entity_type.to_string(), id.to_string()))
            .map(|ce| ce.entity.clone())
    }

    /// Get the full `CachedEntity` (including hash and version) by type and id.
    pub async fn get_cached(&self, entity_type: &str, id: &str) -> Option<CachedEntity> {
        let map = self.cache.read().await;
        map.get(&(entity_type.to_string(), id.to_string())).cloned()
    }

    /// Get clones of all cached entities of a given type.
    ///
    /// Returns an empty vec if no entities of that type are cached.
    ///
    /// Results are returned in the cache's insertion order — which for
    /// preloaded entities matches `read_entity_dir` order and for
    /// runtime-added entities reflects write chronology.
    pub async fn get_all(&self, entity_type: &str) -> Vec<Entity> {
        let map = self.cache.read().await;
        map.iter()
            .filter(|((t, _), _)| t == entity_type)
            .map(|(_, ce)| ce.entity.clone())
            .collect()
    }

    /// Write an entity to disk and update the cache.
    ///
    /// Delegates to `inner.write()` first. On success, computes the content hash
    /// and inserts/updates the cache entry. Only bumps the version and emits an
    /// `EntityChanged` event if the content hash actually changed (or the entity
    /// is new). The emitted event carries a `changes` vector describing the
    /// field-level diff between the previous cached state and the new on-disk
    /// state (brand-new entities get a full field listing). Returns the
    /// `UndoEntryId` from the underlying write (or `None` if unchanged).
    pub async fn write(&self, entity: &Entity) -> Result<Option<UndoEntryId>> {
        // Capture the pre-write cached entity so we can diff against it after
        // the write completes. Both the hash (for the no-op probe) and the
        // full entity (for the field-level diff) come from the same snapshot
        // under a single read lock.
        let (old_hash, old_entity) = {
            let map = self.cache.read().await;
            map.get(&(entity.entity_type.to_string(), entity.id.to_string()))
                .map(|ce| (Some(ce.hash), Some(ce.entity.clone())))
                .unwrap_or((None, None))
        };

        // Use `write_internal` directly so we don't recurse back through the
        // cache-aware `EntityContext::write` dispatcher (which would call us
        // again). This keeps the cache as the single entry point while
        // letting disk I/O stay on `EntityContext`.
        let change_id = self.inner.write_internal(entity).await?;

        // Read back the raw canonical on-disk form (no compute applied) so
        // the cached hash matches what refresh_from_disk would compute
        // after a round-trip and aggregate computes stay fresh on each
        // read.
        let canonical = self
            .inner
            .read_raw_internal(&entity.entity_type, &entity.id)
            .await
            .unwrap_or_else(|_| entity.clone());

        let new_hash = hash_entity(&canonical);
        let key = (entity.entity_type.to_string(), entity.id.to_string());

        let changed = old_hash != Some(new_hash);

        let version = if changed {
            self.bump_version()
        } else {
            // Content unchanged — reuse existing version.
            let map = self.cache.read().await;
            map.get(&key)
                .map_or_else(|| self.bump_version(), |ce| ce.version)
        };

        // Compute the field-level diff before moving `canonical` into the map.
        let changes = if changed {
            diff(old_entity.as_ref(), &canonical)
        } else {
            Vec::new()
        };

        let mut map = self.cache.write().await;
        map.insert(
            key,
            CachedEntity {
                entity: canonical,
                hash: new_hash,
                version,
            },
        );
        drop(map);

        // A non-idempotent write appends to the changelog and atomically
        // replaces the entity file; either can change what the memoized
        // `_changelog` / `_file_created` entries would return on the next
        // `inject_compute_dependencies` pass. Invalidate unconditionally —
        // a write may touch the entity file's mtime (affecting
        // `_file_created` on btime-less filesystems) and/or append to the
        // changelog. A true no-op write (hash-unchanged) leaves disk
        // untouched and invalidation is strictly conservative in that
        // case; the next reload is cheap and a cleaner contract than
        // trying to gate invalidation on the write's return value.
        self.invalidate_entity_caches(&entity.entity_type, &entity.id)
            .await;

        if changed {
            let _ = self.event_sender.send(EntityEvent::EntityChanged {
                entity_type: entity.entity_type.to_string(),
                id: entity.id.to_string(),
                version,
                changes,
            });
        }

        Ok(change_id)
    }

    /// Delete an entity from disk and remove it from the cache.
    ///
    /// Delegates to `inner.delete()` first, then removes the cache entry and
    /// emits an `EntityDeleted` event. Returns the `ChangeEntryId` from the
    /// underlying delete (or `None`).
    pub async fn delete(&self, entity_type: &str, id: &str) -> Result<Option<UndoEntryId>> {
        // Use `delete_internal` to avoid recursing back through the
        // cache-aware `EntityContext::delete` dispatcher.
        let change_id = self.inner.delete_internal(entity_type, id).await?;

        let mut map = self.cache.write().await;
        map.shift_remove(&(entity_type.to_string(), id.to_string()));
        drop(map);

        // Drop the secondary caches for this entity so a later
        // `unarchive` or re-creation does not see stale memoized values.
        self.purge_entity_caches(entity_type, id).await;

        let _ = self.event_sender.send(EntityEvent::EntityDeleted {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
        });

        Ok(change_id)
    }

    /// Archive an entity — move it out of the live map and to disk.
    ///
    /// Delegates the archive operation to `EntityContext::archive_internal`
    /// (which moves the file to the archive directory on disk), then removes
    /// the entry from the in-memory cache so subsequent `list` calls no
    /// longer surface it. Emits an `EntityDeleted` event because from the
    /// live-list perspective the entity is gone — consumers observe the
    /// archive the same way they observe a delete.
    pub async fn archive(&self, entity_type: &str, id: &str) -> Result<Option<UndoEntryId>> {
        let change_id = self.inner.archive_internal(entity_type, id).await?;

        let mut map = self.cache.write().await;
        map.shift_remove(&(entity_type.to_string(), id.to_string()));
        drop(map);

        self.purge_entity_caches(entity_type, id).await;

        let _ = self.event_sender.send(EntityEvent::EntityDeleted {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
        });

        Ok(change_id)
    }

    /// Unarchive an entity — move it back from archive and re-insert it.
    ///
    /// Delegates the unarchive operation to `EntityContext::unarchive_internal`
    /// (which moves the file back from the archive directory on disk), then
    /// re-reads the restored entity from disk and inserts it into the cache
    /// with a fresh hash/version. Emits an `EntityChanged` event so
    /// consumers see the entity reappear.
    pub async fn unarchive(&self, entity_type: &str, id: &str) -> Result<Option<UndoEntryId>> {
        let change_id = self.inner.unarchive_internal(entity_type, id).await?;

        // Re-read the restored file in raw form so the cache holds
        // canonical disk-shape data with no pre-computed aggregates.
        let entity = self.inner.read_raw_internal(entity_type, id).await?;
        let hash = hash_entity(&entity);
        let version = self.bump_version();
        let changes = diff(None, &entity);

        let key = (entity_type.to_string(), id.to_string());
        let mut map = self.cache.write().await;
        map.insert(
            key,
            CachedEntity {
                entity,
                hash,
                version,
            },
        );
        drop(map);

        // The restored file may have a different btime than any previously
        // cached value (unarchive rewrites the file), and its changelog may
        // differ too. Drop the memoized inputs and derived outputs so the
        // next compute pass re-reads both.
        self.invalidate_entity_caches(entity_type, id).await;

        let _ = self.event_sender.send(EntityEvent::EntityChanged {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            version,
            changes,
        });

        Ok(change_id)
    }

    /// Remove an entity from the cache without touching disk.
    ///
    /// Used by the file watcher when an external process deletes a file.
    /// Emits `EntityDeleted` if the entity was in the cache.
    pub async fn evict(&self, entity_type: &str, id: &str) {
        let key = (entity_type.to_string(), id.to_string());
        let mut map = self.cache.write().await;
        let removed = map.shift_remove(&key).is_some();
        drop(map);

        if removed {
            self.purge_entity_caches(entity_type, id).await;
            let _ = self.event_sender.send(EntityEvent::EntityDeleted {
                entity_type: entity_type.to_string(),
                id: id.to_string(),
            });
        }
    }

    /// Re-read an entity from disk and update the cache if it changed.
    ///
    /// Returns `true` if the on-disk content differs from the cached version
    /// (or if the entity was not previously cached). Returns `false` if the
    /// content hash matches. When an event is emitted, `changes` carries the
    /// field-level diff between the previous cached state and the freshly-read
    /// on-disk state.
    pub async fn refresh_from_disk(&self, entity_type: &str, id: &str) -> Result<bool> {
        // Read the raw on-disk form — we need the canonical on-disk fields
        // (no compute) to detect external edits and keep the cache free of
        // frozen aggregate values.
        let entity = self.inner.read_raw_internal(entity_type, id).await?;
        let new_hash = hash_entity(&entity);
        let key = (entity_type.to_string(), id.to_string());

        let mut map = self.cache.write().await;
        let (changed, old_entity) = match map.get(&key) {
            Some(cached) => (cached.hash != new_hash, Some(cached.entity.clone())),
            None => (true, None),
        };

        if changed {
            let version = self.bump_version();
            let changes = diff(old_entity.as_ref(), &entity);
            map.insert(
                key,
                CachedEntity {
                    entity,
                    hash: new_hash,
                    version,
                },
            );
            drop(map);

            // An external edit may have rewritten the entity file (changing
            // btime) or appended to its changelog. Drop any memoized
            // compute inputs and derived outputs so the next list/read
            // re-reads from disk and re-runs the simple derivations.
            self.invalidate_entity_caches(entity_type, id).await;

            let _ = self.event_sender.send(EntityEvent::EntityChanged {
                entity_type: entity_type.to_string(),
                id: id.to_string(),
                version,
                changes,
            });
        }

        Ok(changed)
    }

    /// Get the cached `_changelog` value for an entity, loading it from disk
    /// on a miss.
    ///
    /// The returned value is the same `Value::Array` of serialized
    /// `ChangeEntry` objects that `EntityContext::inject_compute_dependencies`
    /// feeds into `entity.fields` before running the compute engine. On a hit
    /// this path is an in-memory clone; on a miss we read the `.jsonl`
    /// changelog from disk through the wrapped `EntityContext` and memoize
    /// the serialized array for subsequent calls.
    ///
    /// Returns `Value::Array(vec![])` — not an error — when the entity has
    /// no changelog yet (brand-new entity with no writes, or a type without
    /// one on disk). The caller uses this value verbatim for the pseudo-field
    /// injection.
    ///
    /// Thread-safety: a concurrent miss may cause two or three threads to
    /// race through the disk read. That is benign — the final state converges
    /// because every winner writes the same value (barring a write that
    /// also invalidates this entry, in which case the next read reloads).
    /// Holding the write lock across the disk read would serialize all list
    /// calls, which defeats the purpose of the cache.
    pub async fn get_or_load_changelog(&self, entity_type: &str, id: &str) -> serde_json::Value {
        // Dispatch through the combined loader so the two cache-lookup paths
        // are implemented once. Callers that only need the changelog still
        // pay the single read-lock acquisition, which is the dominant cost.
        self.get_or_load_compute_inputs(entity_type, id, true, false)
            .await
            .0
    }

    /// Get the cached `_file_created` value for an entity, loading it from
    /// disk on a miss.
    ///
    /// `_file_created` is derived from the entity file's `created()` (or
    /// `modified()` fallback) metadata. The value never changes for a live
    /// entity — the btime is fixed at file creation — so we memoize the
    /// first successful load and only re-stat after an invalidation event
    /// (which includes `refresh_from_disk`, in case an external tool
    /// replaced the file).
    ///
    /// Returns `Value::Null` when the file cannot be stat'd — this is a
    /// backstop signal, so a missing file must never surface as an error.
    /// A null result is still memoized so transient errors do not stampede
    /// into repeated stat calls; the natural invalidation hooks (write,
    /// refresh) clear it on any relevant event.
    pub async fn get_or_load_file_created(&self, entity_type: &str, id: &str) -> serde_json::Value {
        self.get_or_load_compute_inputs(entity_type, id, false, true)
            .await
            .1
    }

    /// Load both compute pseudo-field inputs for an entity in a single
    /// cache-lookup round trip.
    ///
    /// The batched path exists so `EntityContext::inject_compute_dependencies`
    /// can acquire the `compute_inputs` read lock exactly once per entity
    /// instead of twice. Under the 64-way concurrent list fan-out that
    /// doubles the effective lock contention — measurable on
    /// `move_task_bench` — for no benefit.
    ///
    /// Each `want_*` flag gates whether we compute/return that specific
    /// pseudo-field. Returned entries are `Value::Null` when the caller did
    /// not request them — callers must inspect the `want_*` flags or the
    /// field they asked for to disambiguate.
    ///
    /// Missing entries are loaded off-lock (the write lock is held only for
    /// the final memoization), so the slow-path disk read never blocks
    /// other entity lookups.
    pub(crate) async fn get_or_load_compute_inputs(
        &self,
        entity_type: &str,
        id: &str,
        want_changelog: bool,
        want_file_created: bool,
    ) -> (serde_json::Value, serde_json::Value) {
        let key = (entity_type.to_string(), id.to_string());

        // Snapshot the invalidation epoch before we start so we can detect
        // a mutation that lands between the read-lock drop and the
        // write-lock acquire. The epoch is bumped by every mutation path
        // (`invalidate_entity_caches` and `purge_entity_caches`) so
        // even a purge-then-reinsert sequence is covered.
        let observed_epoch = self.cache_epoch.load(Ordering::Acquire);

        // Fast path: single read-lock acquisition, clone both cached values
        // when present.
        let (mut changelog, mut file_created) = {
            let map = self.compute_inputs.read().await;
            match map.get(&key) {
                Some(inputs) => (inputs.changelog.clone(), inputs.file_created.clone()),
                None => (None, None),
            }
        };

        // Load any requested values not present in the cache. We do these
        // off-lock so the disk reads overlap freely across concurrent
        // `list` passes.
        let need_changelog = want_changelog && changelog.is_none();
        let need_file_created = want_file_created && file_created.is_none();

        if need_changelog {
            let entries = self
                .inner
                .read_changelog(entity_type, id)
                .await
                .unwrap_or_default();
            let json_entries: Vec<serde_json::Value> = entries
                .iter()
                .filter_map(|e| serde_json::to_value(e).ok())
                .collect();
            changelog = Some(serde_json::Value::Array(json_entries));
        }

        if need_file_created {
            file_created = Some(
                self.inner
                    .compute_file_created_timestamp(entity_type, id)
                    .await,
            );
        }

        // Memoize under the write lock, guarded by the epoch. The guard
        // ensures that a concurrent invalidation which landed between
        // our initial read-lock drop and our write-lock acquire is
        // observed — our loaded data may already be stale in that case
        // and must not overwrite the invalidated slot.
        if need_changelog || need_file_created {
            self.try_memoize_compute_inputs(
                key,
                observed_epoch,
                need_changelog.then(|| changelog.clone()).flatten(),
                need_file_created.then(|| file_created.clone()).flatten(),
            )
            .await;
        }

        (
            changelog.unwrap_or(serde_json::Value::Null),
            file_created.unwrap_or(serde_json::Value::Null),
        )
    }

    /// Memoize the loaded compute-input values into the cache, guarded
    /// by the invalidation epoch.
    ///
    /// Called by `get_or_load_compute_inputs` after it has finished its
    /// off-lock disk reads and is about to write the loaded values back.
    /// `observed_epoch` is the value the loader captured BEFORE its
    /// disk read; this function compares it against the current epoch
    /// and refuses to memoize if the epoch advanced in the meantime —
    /// a stale loader must never overwrite an invalidated slot.
    ///
    /// Extracted to a standalone helper so the race-guard semantics can
    /// be unit-tested directly against a synthetic stale-loader
    /// invocation (see `test_stale_loader_memoization_is_rejected`),
    /// rather than relying on scheduler-dependent interleavings to
    /// exercise the logic.
    ///
    /// Two epoch checks, not one:
    ///
    /// 1. A pre-lock check (`current_epoch == observed_epoch`) that
    ///    short-circuits acquiring the write lock when we already
    ///    know the epoch advanced.
    /// 2. A post-lock check (`still_current`) that re-reads the epoch
    ///    after taking the write lock. An invalidation always bumps
    ///    the epoch BEFORE acquiring its own write lock, so this
    ///    catches the narrow window where the invalidation's epoch
    ///    bump lands between our pre-lock check and our lock
    ///    acquisition.
    async fn try_memoize_compute_inputs(
        &self,
        key: (String, String),
        observed_epoch: u64,
        changelog: Option<serde_json::Value>,
        file_created: Option<serde_json::Value>,
    ) {
        let current_epoch = self.cache_epoch.load(Ordering::Acquire);
        if current_epoch != observed_epoch {
            // Pre-lock fast path: epoch already advanced, don't bother
            // with the write lock — our loaded data is stale.
            return;
        }

        let mut map = self.compute_inputs.write().await;
        // Re-check after taking the write lock — invalidation bumps
        // the epoch before taking the write lock itself, so we could
        // otherwise still race an invalidation that acquires the
        // write lock after our pre-check but before our insert.
        let still_current = self.cache_epoch.load(Ordering::Acquire) == observed_epoch;
        if !still_current {
            return;
        }

        let slot = map.entry(key).or_default();
        if let Some(v) = changelog {
            slot.changelog = Some(v);
        }
        if let Some(v) = file_created {
            slot.file_created = Some(v);
        }
    }

    /// Invalidate any cached compute inputs (`_changelog`, `_file_created`)
    /// and derived outputs for the given entity, plus the derived outputs
    /// of any other entity type whose aggregates depend on this type.
    ///
    /// Called from every mutation path on the cache where the entity
    /// survives the mutation — `write`, `refresh_from_disk`, `unarchive`
    /// — so the next `inject_compute_dependencies` pass observes fresh
    /// inputs and the next `derive_compute_fields` call re-runs the
    /// derivations. Both secondary cache slots for the target entity
    /// stay in their respective maps with their fields cleared, on the
    /// expectation that a subsequent `list()` will repopulate them.
    ///
    /// Cross-entity invalidation: a mutation on an entity of type T must
    /// also invalidate the derived-output cache for every entity whose
    /// type has an aggregate computed field with `depends_on: [T]` —
    /// otherwise cached aggregate outputs on those entities would go
    /// stale. The set of affected types comes from
    /// [`FieldsContext::entity_types_depending_on`]. For each such type
    /// we clear every derived-output slot of that type (a full-type
    /// clear, not per-entity) because every entity of that type owns
    /// the same aggregate dependency.
    ///
    /// Bumps the global invalidation epoch **before** acquiring any
    /// write lock so in-flight loaders (for inputs) and derivers (for
    /// outputs) see the epoch change even if they beat this call to the
    /// write lock.
    ///
    /// Invariant: **invalidate when the entity survives the mutation;
    /// purge when it does not.** `delete`, `evict`, and `archive` go
    /// through [`purge_entity_caches`] instead because the entity is
    /// leaving the live map — there is no future read to repopulate the
    /// slots, so the entries should be removed outright.
    async fn invalidate_entity_caches(&self, entity_type: &str, id: &str) {
        // Bump epoch first, under Release ordering, so any subsequent
        // Acquire-load by a loader or deriver observes it. One epoch
        // bump covers both secondary caches and every cross-type
        // invalidation that follows.
        self.cache_epoch.fetch_add(1, Ordering::Release);

        let key = (entity_type.to_string(), id.to_string());

        // Clear the compute-input slot's fields in place — a subsequent
        // list/read will reload from disk.
        {
            let mut map = self.compute_inputs.write().await;
            if let Some(slot) = map.get_mut(&key) {
                slot.changelog = None;
                slot.file_created = None;
            }
        }

        // Clear the derived-output slot's map in place — a subsequent
        // derive_compute_fields call will re-run the derivations.
        {
            let mut map = self.derived.write().await;
            if let Some(slot) = map.get_mut(&key) {
                slot.outputs = None;
            }
        }

        // Cross-entity invalidation: clear derived outputs on every
        // entity type whose aggregates declare this type in `depends_on`.
        self.invalidate_cross_type_derived(entity_type).await;
    }

    /// Purge the secondary cache slots for an entity that is going away,
    /// plus cross-type invalidation for dependent aggregates.
    ///
    /// Called from `delete`, `evict`, `archive` — paths where the entity
    /// itself has been removed from the primary cache, so no future
    /// compute dependencies or derivations need to be tracked for this
    /// entity. Bumps the global invalidation epoch before removing so
    /// racing loaders or derivers drop any value they would otherwise
    /// insert back in.
    ///
    /// A delete/archive is still a mutation observable to aggregates on
    /// sibling types, so the same cross-type invalidation applies as in
    /// [`invalidate_entity_caches`].
    async fn purge_entity_caches(&self, entity_type: &str, id: &str) {
        self.cache_epoch.fetch_add(1, Ordering::Release);

        let key = (entity_type.to_string(), id.to_string());

        {
            let mut map = self.compute_inputs.write().await;
            map.remove(&key);
        }
        {
            let mut map = self.derived.write().await;
            map.remove(&key);
        }

        self.invalidate_cross_type_derived(entity_type).await;
    }

    /// Clear every derived-output slot for entity types whose aggregates
    /// declare `trigger_type` as a dependency.
    ///
    /// Resolves the dependent types via
    /// [`FieldsContext::entity_types_depending_on`] and, for each one,
    /// wipes the entire derived-output map's entries of that type. The
    /// entries are cleared in-place (`outputs = None`) rather than
    /// removed — the next list pass will repopulate them — matching the
    /// per-entity `invalidate_entity_caches` semantics. We do not touch
    /// the compute-input cache for dependent types: aggregate
    /// derivations read their inputs via the `EntityQueryFn`, not
    /// through the pseudo-field injection path, so `_changelog` /
    /// `_file_created` for a dependent-type entity are unchanged by a
    /// mutation on `trigger_type`.
    ///
    /// The caller must have bumped `cache_epoch` before this function
    /// runs so concurrent derivers observe the invalidation.
    async fn invalidate_cross_type_derived(&self, trigger_type: &str) {
        let dependents: Vec<String> = self
            .inner
            .fields()
            .entity_types_depending_on(trigger_type)
            .into_iter()
            .filter(|t| *t != trigger_type)
            .map(|t| t.to_string())
            .collect();
        if dependents.is_empty() {
            return;
        }

        let mut map = self.derived.write().await;
        for slot_type in dependents {
            for (k, slot) in map.iter_mut() {
                if k.0 == slot_type {
                    slot.outputs = None;
                }
            }
        }
    }

    /// Fetch the memoized computed-field outputs for an entity.
    ///
    /// Returns `(Some(outputs), epoch)` on a warm hit, where `outputs`
    /// is the map of already-computed field values (both simple and
    /// aggregate) and `epoch` is the cache epoch at the moment of the
    /// read. The caller uses `outputs` verbatim — copying values into
    /// `entity.fields` — and passes `epoch` into
    /// [`try_memoize_derived_outputs`] on the cold path so the
    /// memoization write is guarded against any invalidation that
    /// lands mid-compute.
    ///
    /// Returns `(None, epoch)` on a cold miss. The caller then runs the
    /// derivations, collects their outputs, and calls
    /// [`try_memoize_derived_outputs`] with the same `epoch` to land
    /// the value back in the cache.
    pub(crate) async fn get_derived_outputs(
        &self,
        entity_type: &str,
        id: &str,
    ) -> (Option<HashMap<String, serde_json::Value>>, u64) {
        let observed_epoch = self.cache_epoch.load(Ordering::Acquire);
        let key = (entity_type.to_string(), id.to_string());

        let map = self.derived.read().await;
        let outputs = map.get(&key).and_then(|slot| slot.outputs.clone());
        (outputs, observed_epoch)
    }

    /// Memoize a freshly-derived map of computed-field outputs for an
    /// entity, guarded by the invalidation epoch.
    ///
    /// Called by the compute-path after it has run the derivations and
    /// collected their outputs. `observed_epoch` is the value the caller
    /// captured BEFORE the derivation started; this function compares it
    /// against the current epoch and refuses to memoize if the epoch
    /// advanced in the meantime — a stale derivation must never
    /// overwrite an invalidated slot.
    ///
    /// Two epoch checks mirror [`try_memoize_compute_inputs`]: a
    /// pre-lock fast path and a post-lock re-check that catches the
    /// narrow window where an invalidation's epoch bump lands between
    /// the pre-check and the write-lock acquisition.
    pub(crate) async fn try_memoize_derived_outputs(
        &self,
        entity_type: &str,
        id: &str,
        observed_epoch: u64,
        outputs: HashMap<String, serde_json::Value>,
    ) {
        let current_epoch = self.cache_epoch.load(Ordering::Acquire);
        if current_epoch != observed_epoch {
            return;
        }

        let key = (entity_type.to_string(), id.to_string());
        let mut map = self.derived.write().await;
        let still_current = self.cache_epoch.load(Ordering::Acquire) == observed_epoch;
        if !still_current {
            return;
        }

        let slot = map.entry(key).or_default();
        slot.outputs = Some(outputs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_fields_context;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper: build an EntityCache backed by a temp directory.
    async fn setup() -> (TempDir, EntityCache) {
        let fields = test_fields_context();
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        // Create entity directories
        std::fs::create_dir_all(root.join("tags")).unwrap();
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        let ctx = Arc::new(EntityContext::new(&root, fields));
        let cache = EntityCache::new(ctx);
        (temp, cache)
    }

    /// load_all reads entities from disk and populates the cache.
    #[tokio::test]
    async fn load_all_populates_cache() {
        let (_dir, cache) = setup().await;

        // Write entities directly via inner context (bypassing cache)
        let mut t1 = Entity::new("tag", "t1");
        t1.set("tag_name", json!("Bug"));
        t1.set("color", json!("#ff0000"));
        cache.inner().write(&t1).await.unwrap();

        let mut t2 = Entity::new("tag", "t2");
        t2.set("tag_name", json!("Feature"));
        t2.set("color", json!("#00ff00"));
        cache.inner().write(&t2).await.unwrap();

        // Cache should be empty before load_all
        assert!(cache.get("tag", "t1").await.is_none());
        assert!(cache.get("tag", "t2").await.is_none());

        // Load from disk
        cache.load_all("tag").await.unwrap();

        // Verify cache contains both entities
        let cached_t1 = cache.get("tag", "t1").await.unwrap();
        assert_eq!(cached_t1.get_str("tag_name"), Some("Bug"));

        let cached_t2 = cache.get("tag", "t2").await.unwrap();
        assert_eq!(cached_t2.get_str("tag_name"), Some("Feature"));
    }

    /// write updates both disk and cache, and hash/version change on modification.
    #[tokio::test]
    async fn write_updates_cache_and_disk() {
        let (_dir, cache) = setup().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        let cached = cache.get_cached("tag", "t1").await.unwrap();
        let hash1 = cached.hash;
        let ver1 = cached.version;

        // Modify a field and write again
        tag.set("tag_name", json!("Critical Bug"));
        cache.write(&tag).await.unwrap();

        let cached2 = cache.get_cached("tag", "t1").await.unwrap();
        assert_ne!(
            cached2.hash, hash1,
            "hash should change when content changes"
        );
        assert!(cached2.version > ver1, "version should bump on write");
    }

    /// Writing the same content produces the same hash and does not bump version.
    #[tokio::test]
    async fn write_same_content_same_hash() {
        let (_dir, cache) = setup().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        let cached1 = cache.get_cached("tag", "t1").await.unwrap();
        let hash1 = cached1.hash;
        let ver1 = cached1.version;

        // Write the exact same entity again
        cache.write(&tag).await.unwrap();

        let cached2 = cache.get_cached("tag", "t1").await.unwrap();
        assert_eq!(
            hash1, cached2.hash,
            "hash should be identical for same content"
        );
        assert_eq!(
            ver1, cached2.version,
            "version should not bump for same content"
        );
    }

    /// delete removes the entity from both disk and cache.
    #[tokio::test]
    async fn delete_removes_from_cache() {
        let (_dir, cache) = setup().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();
        assert!(cache.get("tag", "t1").await.is_some());

        cache.delete("tag", "t1").await.unwrap();
        assert!(cache.get("tag", "t1").await.is_none());
    }

    /// refresh_from_disk detects when a file was modified outside the cache.
    #[tokio::test]
    async fn refresh_from_disk_detects_change() {
        let (_dir, cache) = setup().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        // Modify the file directly on disk, bypassing the cache
        tag.set("tag_name", json!("Changed On Disk"));
        cache.inner().write(&tag).await.unwrap();

        // Refresh should detect the change
        let changed = cache.refresh_from_disk("tag", "t1").await.unwrap();
        assert!(changed, "refresh should detect disk change");

        // Cache should now reflect the on-disk value
        let refreshed = cache.get("tag", "t1").await.unwrap();
        assert_eq!(refreshed.get_str("tag_name"), Some("Changed On Disk"));
    }

    /// refresh_from_disk returns false when content hasn't changed.
    #[tokio::test]
    async fn refresh_from_disk_no_change() {
        let (_dir, cache) = setup().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        // Refresh without any disk changes
        let changed = cache.refresh_from_disk("tag", "t1").await.unwrap();
        assert!(!changed, "refresh should report no change");
    }

    /// Concurrent reads from cache don't block each other.
    #[tokio::test]
    async fn concurrent_reads_dont_block() {
        let (_dir, cache) = setup().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        let cache = Arc::new(cache);
        let mut handles = Vec::new();

        for _ in 0..10 {
            let cache = Arc::clone(&cache);
            handles.push(tokio::spawn(async move {
                let entity = cache.get("tag", "t1").await;
                assert!(entity.is_some());
                entity.unwrap()
            }));
        }

        for handle in handles {
            let entity = handle.await.unwrap();
            assert_eq!(entity.get_str("tag_name"), Some("Bug"));
        }
    }

    /// Writing an entity emits EntityChanged with correct type, id, version,
    /// and a non-empty changes payload.
    #[tokio::test]
    async fn write_emits_entity_changed_event() {
        let (_dir, cache) = setup().await;
        let mut rx = cache.subscribe();

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            EntityEvent::EntityChanged {
                entity_type,
                id,
                version,
                changes,
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "t1");
                assert!(version > 0);
                assert!(
                    !changes.is_empty(),
                    "a first write should report its fields in `changes`"
                );
            }
            _ => panic!("expected EntityChanged"),
        }
    }

    /// Deleting an entity emits EntityDeleted.
    #[tokio::test]
    async fn delete_emits_entity_deleted_event() {
        let (_dir, cache) = setup().await;
        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let mut rx = cache.subscribe();
        cache.delete("tag", "t1").await.unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            EntityEvent::EntityDeleted { entity_type, id } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "t1");
            }
            _ => panic!("expected EntityDeleted"),
        }
    }

    /// Writing the same content emits no event.
    #[tokio::test]
    async fn write_same_content_no_event() {
        let (_dir, cache) = setup().await;
        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        let mut rx = cache.subscribe();
        // Write exact same content again
        cache.write(&tag).await.unwrap();

        // Should have no event
        assert!(rx.try_recv().is_err());
    }

    /// Version numbers are monotonically increasing across changes.
    #[tokio::test]
    async fn versions_monotonically_increasing() {
        let (_dir, cache) = setup().await;
        let mut rx = cache.subscribe();

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let v1 = match rx.try_recv().unwrap() {
            EntityEvent::EntityChanged { version, .. } => version,
            _ => panic!("expected EntityChanged"),
        };

        tag.set("tag_name", json!("Critical"));
        cache.write(&tag).await.unwrap();

        let v2 = match rx.try_recv().unwrap() {
            EntityEvent::EntityChanged { version, .. } => version,
            _ => panic!("expected EntityChanged"),
        };

        assert!(v2 > v1);
    }

    /// refresh_from_disk emits event when content changed on disk.
    #[tokio::test]
    async fn refresh_from_disk_emits_event_on_change() {
        let (_dir, cache) = setup().await;
        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        // Modify on disk directly
        tag.set("tag_name", json!("Changed"));
        cache.inner().write(&tag).await.unwrap();

        let mut rx = cache.subscribe();
        let changed = cache.refresh_from_disk("tag", "t1").await.unwrap();
        assert!(changed);

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, EntityEvent::EntityChanged { .. }));
    }

    /// evict removes from cache and emits EntityDeleted without touching disk.
    #[tokio::test]
    async fn evict_removes_and_emits_event() {
        let (_dir, cache) = setup().await;
        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let mut rx = cache.subscribe();
        cache.evict("tag", "t1").await;

        assert!(cache.get("tag", "t1").await.is_none());
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, EntityEvent::EntityDeleted { .. }));
    }

    /// evict emits no event when entity is not in cache.
    #[tokio::test]
    async fn evict_no_event_when_not_cached() {
        let (_dir, cache) = setup().await;
        let mut rx = cache.subscribe();
        cache.evict("tag", "nonexistent").await;
        assert!(rx.try_recv().is_err());
    }

    /// refresh_from_disk does NOT emit event when content unchanged.
    #[tokio::test]
    async fn refresh_from_disk_no_event_when_unchanged() {
        let (_dir, cache) = setup().await;
        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        let mut rx = cache.subscribe();
        let changed = cache.refresh_from_disk("tag", "t1").await.unwrap();
        assert!(!changed);
        assert!(rx.try_recv().is_err());
    }

    /// get_all returns all cached entities of a given type.
    #[tokio::test]
    async fn get_all_returns_entities_of_type() {
        let (_dir, cache) = setup().await;

        // Write two tags and one task
        let mut t1 = Entity::new("tag", "t1");
        t1.set("tag_name", json!("Bug"));
        t1.set("color", json!("#ff0000"));
        cache.write(&t1).await.unwrap();

        let mut t2 = Entity::new("tag", "t2");
        t2.set("tag_name", json!("Feature"));
        t2.set("color", json!("#00ff00"));
        cache.write(&t2).await.unwrap();

        let mut task = Entity::new("task", "01ABC");
        task.set("title", json!("Fix bug"));
        task.set("body", json!("Details"));
        cache.write(&task).await.unwrap();

        // get_all for "tag" should return exactly the two tags
        let tags = cache.get_all("tag").await;
        assert_eq!(tags.len(), 2);
        let tag_ids: Vec<&str> = tags.iter().map(|t| t.id.as_str()).collect();
        assert!(tag_ids.contains(&"t1"));
        assert!(tag_ids.contains(&"t2"));

        // get_all for "task" should return exactly one task
        let tasks = cache.get_all("task").await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "01ABC");

        // get_all for unknown type returns empty
        let empty = cache.get_all("column").await;
        assert!(empty.is_empty());
    }

    /// Helper: drain and return the next `EntityChanged` event's `changes`
    /// vector. Panics if the next event is not `EntityChanged`.
    fn take_changes(event: EntityEvent) -> Vec<FieldChange> {
        match event {
            EntityEvent::EntityChanged { changes, .. } => changes,
            other => panic!("expected EntityChanged, got {:?}", other),
        }
    }

    /// Modifying an existing entity reports only the changed and added fields
    /// in `changes` — untouched fields do not appear.
    #[tokio::test]
    async fn test_entity_changed_carries_field_diff() {
        let (_dir, cache) = setup().await;

        // Seed with {a:1, b:2}
        let mut tag = Entity::new("tag", "t1");
        tag.set("a", json!(1));
        tag.set("b", json!(2));
        cache.write(&tag).await.unwrap();

        // Subscribe only now so we observe the next write in isolation.
        let mut rx = cache.subscribe();

        // Write {a:1, b:3, c:4}: `b` changes, `c` is new, `a` is untouched.
        tag.set("b", json!(3));
        tag.set("c", json!(4));
        cache.write(&tag).await.unwrap();

        let changes = take_changes(rx.try_recv().unwrap());
        let by_field: std::collections::HashMap<_, _> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();

        assert_eq!(by_field.len(), 2, "expected exactly two changed fields");
        assert_eq!(by_field.get("b"), Some(&&json!(3)));
        assert_eq!(by_field.get("c"), Some(&&json!(4)));
        assert!(
            !by_field.contains_key("a"),
            "unchanged field `a` must not appear in `changes`"
        );
    }

    /// Removing a field is encoded as `{field, value: Null}` in `changes`.
    #[tokio::test]
    async fn test_entity_changed_encodes_removal_as_null() {
        let (_dir, cache) = setup().await;

        // Seed with {a:1, b:2}
        let mut tag = Entity::new("tag", "t1");
        tag.set("a", json!(1));
        tag.set("b", json!(2));
        cache.write(&tag).await.unwrap();

        let mut rx = cache.subscribe();

        // Remove `b`, keep `a`.
        tag.fields.remove("b");
        cache.write(&tag).await.unwrap();

        let changes = take_changes(rx.try_recv().unwrap());
        assert_eq!(changes.len(), 1, "only the removed field should appear");
        assert_eq!(changes[0].field, "b");
        assert_eq!(changes[0].value, serde_json::Value::Null);
    }

    /// First write of a brand-new entity lists every field in `changes`.
    #[tokio::test]
    async fn test_entity_changed_new_entity_lists_all_fields() {
        let (_dir, cache) = setup().await;

        let mut rx = cache.subscribe();

        let mut tag = Entity::new("tag", "t1");
        tag.set("a", json!(1));
        tag.set("b", json!(2));
        cache.write(&tag).await.unwrap();

        let changes = take_changes(rx.try_recv().unwrap());
        let by_field: std::collections::HashMap<_, _> = changes
            .iter()
            .map(|c| (c.field.as_str(), &c.value))
            .collect();

        assert_eq!(by_field.len(), 2, "both fields should be listed");
        assert_eq!(by_field.get("a"), Some(&&json!(1)));
        assert_eq!(by_field.get("b"), Some(&&json!(2)));
    }

    // =========================================================================
    // Compute-input cache tests (_changelog and _file_created memoization)
    // =========================================================================
    //
    // These tests drive the changelog through direct `append_changelog`
    // calls because `EntityContext::write_internal` only appends a
    // changelog entry when a `StoreHandle` is registered for the entity
    // type, and the test fields context does not register one. The
    // invalidation contract we are verifying is independent of the
    // changelog producer — any change to the .jsonl file between two
    // `get_or_load_changelog` calls must go unobserved unless an
    // invalidation event fired.

    /// Append a one-off changelog entry to the `.jsonl` file backing an
    /// entity. Used by compute-input cache tests to simulate external
    /// appends without depending on the store-handle wiring.
    async fn append_test_changelog(cache: &EntityCache, entity_type: &str, id: &str, op: &str) {
        let path = cache
            .inner()
            .changelog_path(entity_type, id)
            .expect("changelog_path");
        let entry = crate::changelog::ChangeEntry::new(
            entity_type,
            id,
            op,
            vec![(
                "marker".to_string(),
                crate::changelog::FieldChange::Set { value: json!(op) },
            )],
        );
        crate::changelog::append_changelog(&path, &entry)
            .await
            .expect("append_changelog");
    }

    /// `get_or_load_changelog` returns the serialized JSONL entries as a
    /// `Value::Array` on first call, then reuses the memoized value on
    /// repeat calls (even if disk state changes) until invalidation.
    #[tokio::test]
    async fn test_changelog_cache_memoizes_across_calls() {
        let (_dir, cache) = setup().await;

        // Seed the entity and write one changelog entry directly.
        let mut t = Entity::new("task", "01TASK0001");
        t.set("title", json!("Hello"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0001", "create").await;

        // First call loads and memoizes.
        let first = cache.get_or_load_changelog("task", "01TASK0001").await;
        let arr_first = first.as_array().expect("changelog is an array");
        assert_eq!(
            arr_first.len(),
            1,
            "one appended entry should be visible on the first load"
        );

        // Append another entry on disk without going through the cache's
        // invalidation hooks — the memoized value must still be returned.
        append_test_changelog(&cache, "task", "01TASK0001", "update").await;

        let second = cache.get_or_load_changelog("task", "01TASK0001").await;
        assert_eq!(
            second.as_array().unwrap().len(),
            arr_first.len(),
            "memoized value must ignore disk-side appends until invalidated"
        );
    }

    /// `cache.write` invalidates the memoized changelog so the next load
    /// reflects any append that happened alongside the write.
    #[tokio::test]
    async fn test_changelog_cache_invalidates_on_write() {
        let (_dir, cache) = setup().await;

        let mut t = Entity::new("task", "01TASK0002");
        t.set("title", json!("First"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0002", "create").await;

        let before = cache.get_or_load_changelog("task", "01TASK0002").await;
        let before_len = before.as_array().unwrap().len();
        assert_eq!(before_len, 1);

        // Append directly so we know the changelog grew, then write through
        // the cache — the write must invalidate the memoized value even
        // though it was the external append (not the write itself) that
        // produced the new entry. The contract: any `write` invalidates.
        append_test_changelog(&cache, "task", "01TASK0002", "update").await;
        t.set("title", json!("Second"));
        cache.write(&t).await.unwrap();

        let after = cache.get_or_load_changelog("task", "01TASK0002").await;
        let after_len = after.as_array().unwrap().len();
        assert!(
            after_len > before_len,
            "write must invalidate the changelog cache: before={}, after={}",
            before_len,
            after_len
        );
    }

    /// `cache.delete` invalidates memoized compute inputs so a later
    /// re-creation of the same id does not observe stale data.
    #[tokio::test]
    async fn test_changelog_cache_invalidates_on_delete() {
        let (_dir, cache) = setup().await;

        let mut t = Entity::new("task", "01TASK0003");
        t.set("title", json!("to-delete"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0003", "create").await;

        // Prime the cache.
        let before = cache.get_or_load_changelog("task", "01TASK0003").await;
        assert!(!before.as_array().unwrap().is_empty());

        cache.delete("task", "01TASK0003").await.unwrap();

        // After delete, the changelog file is gone (moved to trash); the
        // memoized value must be cleared. The next load reads the empty
        // live path.
        let after = cache.get_or_load_changelog("task", "01TASK0003").await;
        assert!(
            after.as_array().unwrap().is_empty(),
            "delete must drop the memoized changelog so the next read \
             reflects the missing live file"
        );
    }

    /// `cache.evict` invalidates memoized compute inputs alongside the
    /// primary cached entity.
    #[tokio::test]
    async fn test_changelog_cache_invalidates_on_evict() {
        let (_dir, cache) = setup().await;

        let mut t = Entity::new("task", "01TASK0004");
        t.set("title", json!("to-evict"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0004", "create").await;

        // Prime the cache.
        let _primed = cache.get_or_load_changelog("task", "01TASK0004").await;

        // Assert the compute-input entry is present before eviction.
        {
            let map = cache.compute_inputs.read().await;
            assert!(
                map.contains_key(&("task".to_string(), "01TASK0004".to_string())),
                "compute inputs should be cached after get_or_load"
            );
        }

        cache.evict("task", "01TASK0004").await;

        // Compute inputs must be cleared by evict.
        {
            let map = cache.compute_inputs.read().await;
            assert!(
                !map.contains_key(&("task".to_string(), "01TASK0004".to_string())),
                "evict must clear memoized compute inputs"
            );
        }
    }

    /// `cache.refresh_from_disk` invalidates memoized compute inputs when
    /// it detects a disk-side change so the next load sees the current
    /// file state.
    #[tokio::test]
    async fn test_changelog_cache_invalidates_on_refresh_from_disk() {
        let (_dir, cache) = setup().await;

        let mut t = Entity::new("task", "01TASK0005");
        t.set("title", json!("v1"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0005", "create").await;

        let before = cache.get_or_load_changelog("task", "01TASK0005").await;
        let before_len = before.as_array().unwrap().len();
        assert_eq!(before_len, 1);

        // Simulate an external edit: rewrite the entity file and append
        // another changelog entry, then refresh from disk — both inputs
        // must invalidate.
        t.set("title", json!("v2"));
        cache.inner().write_internal(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0005", "update").await;

        let changed = cache.refresh_from_disk("task", "01TASK0005").await.unwrap();
        assert!(changed, "refresh_from_disk should observe the change");

        let after = cache.get_or_load_changelog("task", "01TASK0005").await;
        let after_len = after.as_array().unwrap().len();
        assert!(
            after_len > before_len,
            "refresh_from_disk must drop the memoized changelog"
        );
    }

    /// `get_or_load_file_created` returns an RFC3339 timestamp string for an
    /// existing entity file and null for a missing one, and memoizes both
    /// cases across calls.
    #[tokio::test]
    async fn test_file_created_cache_memoizes() {
        let (_dir, cache) = setup().await;

        let mut t = Entity::new("task", "01TASK0006");
        t.set("title", json!("stamped"));
        cache.write(&t).await.unwrap();

        let first = cache.get_or_load_file_created("task", "01TASK0006").await;
        let second = cache.get_or_load_file_created("task", "01TASK0006").await;
        assert_eq!(first, second, "timestamp must be memoized");
        assert!(
            matches!(&first, serde_json::Value::String(_)),
            "existing entity should produce a timestamp string, got {:?}",
            first
        );

        let missing = cache
            .get_or_load_file_created("task", "does-not-exist")
            .await;
        assert!(matches!(missing, serde_json::Value::Null));
    }

    /// The invalidation epoch protects against a loader that raced with
    /// a concurrent invalidation — specifically the case where a loader
    /// reads an empty slot, drops the read lock, is preempted while the
    /// invalidation runs, and then acquires the write lock with a now-
    /// stale value in hand.
    ///
    /// We drive the race with two tokio tasks on a multi-threaded
    /// runtime. The loader captures `observed_epoch`, starts its disk
    /// read, and yields naturally at `fs::read_to_string.await`. During
    /// that yield the invalidator task mutates the underlying disk
    /// state and bumps the epoch. When the loader resumes it still
    /// holds the pre-mutation data in hand; its memoization attempt
    /// must be rejected by the epoch check so the stale value never
    /// lands in the cache.
    ///
    /// Proof: we mutate the disk state (append a second changelog
    /// entry) between the loader's observed-epoch capture and the
    /// loader's memoize attempt. If the loader's write attempt
    /// succeeded, the cache would contain the 1-entry pre-mutation
    /// changelog. A subsequent non-raced load would then return that
    /// stale 1-entry array instead of the current 2-entry disk state —
    /// the memoization would have "won" against the invalidation. The
    /// test asserts the opposite: after the race, the next load
    /// observes the fresh 2-entry state.
    ///
    /// We iterate the race many times so the scheduler-dependent
    /// interleaving (loader yields on disk read, invalidator runs,
    /// loader resumes) gets exercised on at least one iteration. The
    /// per-iteration assertion catches the regression: any iteration
    /// where a stale 1-entry changelog leaks into the cache would
    /// cause the next fresh load to return that stale value, which
    /// fails the post-race `entries.len() >= expected_min` check.
    ///
    /// The companion `test_stale_loader_memoization_is_rejected`
    /// drives the same guard deterministically by calling
    /// `try_memoize_compute_inputs` directly with a stale
    /// `observed_epoch`, so the regression is caught regardless of
    /// scheduler timing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_invalidation_wins_against_concurrent_loader() {
        let (_dir, cache) = setup().await;
        let cache = Arc::new(cache);

        // Seed an entity with an initial changelog entry.
        let mut t = Entity::new("task", "01TASK0008");
        t.set("title", json!("race"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0008", "create").await;

        let mut saw_fresh_after_race = 0usize;

        // Run the race many times. The iteration count is high enough
        // that a functioning epoch guard will be exercised against a
        // mid-read invalidation on at least one interleaving; a
        // regressed guard will leak a stale 1-entry changelog into the
        // cache on at least one iteration (the assert inside the loop
        // catches it).
        for iter in 0..50 {
            // Reset the cache slot so the race starts from a cold miss.
            // This forces the loader down the disk-read path on every
            // iteration.
            cache.invalidate_entity_caches("task", "01TASK0008").await;

            // The invariant we're about to race:
            // - Loader captures observed_epoch = N.
            // - Loader reads empty slot, drops read lock.
            // - Loader begins fs::read_to_string (yields).
            // - Invalidator (concurrently): append a new changelog
            //   entry to disk, then bump the epoch via
            //   invalidate_entity_caches.
            // - Loader resumes with 1-entry data from pre-mutation
            //   disk state still in hand.
            // - Loader takes write lock, checks epoch (bumped), skips
            //   memoization — the epoch guard wins.

            let loader_cache = Arc::clone(&cache);
            let loader = tokio::spawn(async move {
                loader_cache
                    .get_or_load_changelog("task", "01TASK0008")
                    .await
            });

            let invalidator_cache = Arc::clone(&cache);
            let invalidator = tokio::spawn(async move {
                // Yield to give the loader a chance to start and reach
                // its disk-read await point before we mutate.
                tokio::task::yield_now().await;

                // Mutate the underlying disk state. If the epoch guard
                // is broken, a concurrent loader that already read the
                // pre-mutation state could memoize it and "win" the
                // race, masking the mutation.
                append_test_changelog(
                    &invalidator_cache,
                    "task",
                    "01TASK0008",
                    &format!("update-{}", iter),
                )
                .await;

                // Bump the epoch and clear any in-flight memoization
                // target. Under a correct guard, any loader holding
                // pre-mutation data must refuse to memoize past this
                // point.
                invalidator_cache
                    .invalidate_entity_caches("task", "01TASK0008")
                    .await;
            });

            let (loader_result, invalidator_result) = tokio::join!(loader, invalidator);
            let _ = loader_result.expect("loader task panicked");
            invalidator_result.expect("invalidator task panicked");

            // The invariant: immediately after the race, the cache slot
            // must NOT contain the stale pre-mutation 1-entry
            // changelog. Either it is empty (the loader's write was
            // rejected by the epoch guard), or the loader ran entirely
            // before the invalidator (and whatever it memoized
            // reflects the pre-mutation state, which is fine because
            // the invalidation then cleared it).
            let slot_after_race = {
                let map = cache.compute_inputs.read().await;
                map.get(&("task".to_string(), "01TASK0008".to_string()))
                    .and_then(|s| s.changelog.clone())
            };

            if let Some(v) = &slot_after_race {
                // If the slot is populated, it means the loader's
                // memoization *did* land. That is ONLY safe if the
                // invalidator hadn't bumped the epoch yet when the
                // loader memoized. Either way, the invalidator then
                // cleared it — so the slot being Some here means the
                // loader memoized *after* the invalidator ran. But
                // invalidate_entity_caches sets the slot's fields to
                // None, it doesn't remove the entry. So we could see
                // the entry present with `changelog: None`, which
                // clones to `None` above — that path is safe.
                //
                // If we ever observe `changelog: Some(array)` here,
                // the loader memoized AFTER the invalidator cleared,
                // which means the loader captured a *post-mutation*
                // observed_epoch and did a fresh disk read with the
                // 2-entry changelog. Assert that.
                let arr = v.as_array().expect("memoized value is an array");
                assert!(
                    arr.len() >= 2,
                    "iter {}: cached changelog memoized by a loader that ran \
                     after the invalidator must reflect the post-mutation \
                     2+-entry disk state (got {} entries)",
                    iter,
                    arr.len()
                );
            }

            // Now do a fresh non-raced load and verify it returns the
            // current disk state. If a stale loader had managed to
            // leave a 1-entry changelog cached, this load would return
            // the stale value on a cache hit. With the epoch guard
            // working, the stale value never lands, and this load
            // observes the current 2+-entry disk state.
            let fresh = cache.get_or_load_changelog("task", "01TASK0008").await;
            let entries = fresh.as_array().expect("fresh load returns array");
            let expected_min = 1 + (iter + 1); // initial + one per iteration
            assert!(
                entries.len() >= expected_min,
                "iter {}: post-race fresh load must observe the current \
                 {}+-entry disk state; got {} entries (stale cache?)",
                iter,
                expected_min,
                entries.len()
            );
            saw_fresh_after_race += 1;
        }

        // Sanity: the test actually ran its iterations.
        assert_eq!(
            saw_fresh_after_race, 50,
            "all 50 race rounds should have completed cleanly"
        );
    }

    /// Deterministic counterpart to
    /// `test_invalidation_wins_against_concurrent_loader`: drives the
    /// race-guard logic directly via `try_memoize_compute_inputs` so
    /// the test is not scheduler-dependent.
    ///
    /// The concurrent version proves the guard composes with real
    /// tokio scheduling. This version proves the guard's logic
    /// regardless of scheduling — both epoch checks (the pre-lock
    /// fast path and the post-lock re-check) are exercised by
    /// feeding a stale `observed_epoch` into the helper and verifying
    /// the value never lands in the cache.
    #[tokio::test]
    async fn test_stale_loader_memoization_is_rejected() {
        let (_dir, cache) = setup().await;

        let mut t = Entity::new("task", "01TASK0009");
        t.set("title", json!("stale-loader"));
        cache.write(&t).await.unwrap();
        append_test_changelog(&cache, "task", "01TASK0009", "create").await;

        // Capture the epoch a hypothetical loader would have observed
        // before its disk read started.
        cache.invalidate_entity_caches("task", "01TASK0009").await;
        let observed_epoch = cache.cache_epoch.load(Ordering::Acquire);

        // A stale synthetic value — standing in for the data the loader
        // would have read from disk before the invalidation landed.
        let stale_value = serde_json::json!([{"stale": "placeholder"}]);

        // Simulate the invalidation that fires between the loader's
        // off-lock disk read and its memoize attempt.
        cache.invalidate_entity_caches("task", "01TASK0009").await;
        assert!(
            cache.cache_epoch.load(Ordering::Acquire) > observed_epoch,
            "invalidation must bump the epoch past the stale loader's \
             observed value"
        );

        // Drive the exact memoization path a real loader would take,
        // with the stale observed_epoch. The guard MUST reject it.
        cache
            .try_memoize_compute_inputs(
                ("task".to_string(), "01TASK0009".to_string()),
                observed_epoch,
                Some(stale_value.clone()),
                None,
            )
            .await;

        // Verify the stale value never landed.
        let slot_changelog = {
            let map = cache.compute_inputs.read().await;
            map.get(&("task".to_string(), "01TASK0009".to_string()))
                .and_then(|s| s.changelog.clone())
        };
        assert!(
            slot_changelog.is_none(),
            "stale-loader memoization must be rejected; the slot \
             changelog should remain None, got {:?}",
            slot_changelog
        );

        // A subsequent non-stale load must read fresh data from disk,
        // not return the stale placeholder.
        let fresh = cache.get_or_load_changelog("task", "01TASK0009").await;
        let entries = fresh.as_array().expect("fresh load returns array");
        assert!(!entries.is_empty(), "fresh load must return the disk state");
        assert_ne!(
            entries[0], stale_value[0],
            "post-rejection fresh load must bypass the stale placeholder"
        );
    }

    /// `cache.write` also invalidates the memoized `_file_created` value
    /// so a newly created entity sees a fresh stat on its next read.
    #[tokio::test]
    async fn test_file_created_cache_invalidates_on_write() {
        let (_dir, cache) = setup().await;

        // Pre-populate the `missing-then-created` case: stat the file
        // before it exists (null), then create it, then observe that
        // the cache invalidates so we no longer see null.
        let before = cache.get_or_load_file_created("task", "01TASK0007").await;
        assert!(
            matches!(before, serde_json::Value::Null),
            "non-existent file must produce null"
        );

        let mut t = Entity::new("task", "01TASK0007");
        t.set("title", json!("now-exists"));
        cache.write(&t).await.unwrap();

        let after = cache.get_or_load_file_created("task", "01TASK0007").await;
        assert!(
            matches!(&after, serde_json::Value::String(_)),
            "write must invalidate the cached null so the next stat returns \
             the actual timestamp; got {:?}",
            after
        );
    }

    // =========================================================================
    // Derived-output cache tests (memoized simple-derivation values)
    // =========================================================================
    //
    // These tests drive an `EntityContext` with an attached `ComputeEngine`
    // that has a single `count-changelog` derivation counting entries in the
    // injected `_changelog` pseudo-field. The `task` entity type declares a
    // `change_count` computed field that depends on `_changelog` and uses the
    // `count-changelog` derivation. Because the derivation reads
    // `_changelog` verbatim, the output depends entirely on the changelog
    // size — which is exactly the pattern the derived cache is supposed to
    // memoize.
    //
    // An `Arc<AtomicUsize>` counter registered inside the derive function
    // measures how many times it ran. Under a functioning cache, repeat
    // `list` calls observe the counter incrementing once per entity per
    // mutation boundary, not once per list call.

    use std::sync::atomic::AtomicUsize;
    use swissarmyhammer_fields::{ComputeEngine, FieldsContext};

    /// Build a `FieldsContext` whose `task` entity has a `change_count`
    /// computed field depending on `_changelog`. Mirrors the helper used
    /// by the `_changelog` injection tests in `context.rs` so the same
    /// compute-engine wiring drives the derived cache here.
    fn fields_with_change_count() -> Arc<FieldsContext> {
        let defs = vec![
            (
                "title",
                "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "body",
                "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
            ),
            (
                "change_count",
                "id: 00000000000000000000000CHG\nname: change_count\ntype:\n  kind: computed\n  derive: count-changelog\n  depends_on:\n    - _changelog\n",
            ),
        ];
        let entities = vec![(
            "task",
            "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - change_count\n",
        )];
        let dir = TempDir::new().unwrap();
        Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
    }

    /// Build a `ComputeEngine` whose `count-changelog` derivation reads the
    /// injected `_changelog` array, returns its length, and bumps the
    /// provided counter. The counter lets tests detect whether the
    /// derive function ran or was short-circuited by the derived cache.
    fn compute_engine_counting_runs(counter: Arc<AtomicUsize>) -> Arc<ComputeEngine> {
        let mut engine = ComputeEngine::new();
        engine.register(
            "count-changelog",
            Box::new(move |fields| {
                counter.fetch_add(1, Ordering::Relaxed);
                let count = fields
                    .get("_changelog")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                Box::pin(async move { json!(count) })
            }),
        );
        Arc::new(engine)
    }

    /// Build an `EntityCache` whose inner `EntityContext` has a compute
    /// engine wired in. Returns the cache, the run counter, and the
    /// `TempDir` that owns the on-disk files.
    async fn setup_with_compute() -> (TempDir, Arc<EntityCache>, Arc<AtomicUsize>) {
        let fields = fields_with_change_count();
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("tasks")).unwrap();

        let counter = Arc::new(AtomicUsize::new(0));
        let engine = compute_engine_counting_runs(Arc::clone(&counter));

        let ctx = Arc::new(EntityContext::new(&root, fields).with_compute(engine));
        let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        ctx.attach_cache(&cache);
        (temp, cache, counter)
    }

    /// Helper: write a task and append one changelog entry so the
    /// derived-output cache has something to memoize.
    async fn seed_task(cache: &EntityCache, id: &str) {
        let mut t = Entity::new("task", id);
        t.set("title", json!("seed"));
        cache.write(&t).await.unwrap();
        let path = cache.inner().changelog_path("task", id).unwrap();
        let entry = crate::changelog::ChangeEntry::new(
            "task",
            id,
            "create",
            vec![(
                "title".to_string(),
                crate::changelog::FieldChange::Set {
                    value: json!("seed"),
                },
            )],
        );
        crate::changelog::append_changelog(&path, &entry)
            .await
            .unwrap();
    }

    /// Repeat `list("task")` must not re-run the simple-derivation
    /// compute function on warm-cache passes: the derived-output cache
    /// memoizes the first pass and feeds it back on the second.
    #[tokio::test]
    async fn test_derived_cache_memoizes_across_calls() {
        let (_dir, cache, counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED01").await;

        // First list: cold miss — derive function runs once.
        let first = cache.inner().list("task").await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].fields.get("change_count"),
            Some(&json!(1)),
            "first list should observe the one-entry changelog"
        );
        let after_first = counter.load(Ordering::Relaxed);
        assert_eq!(
            after_first, 1,
            "derive should run exactly once on cold miss"
        );

        // Second list: warm hit — derive function must not run again.
        let second = cache.inner().list("task").await.unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].fields.get("change_count"),
            Some(&json!(1)),
            "warm hit must serve the memoized value"
        );
        assert_eq!(
            counter.load(Ordering::Relaxed),
            after_first,
            "derive must not re-run on warm hit"
        );
    }

    /// After `cache.write`, the derived-output cache must be invalidated
    /// so the next `list` recomputes and observes the post-write state.
    #[tokio::test]
    async fn test_derived_cache_invalidates_on_write() {
        let (_dir, cache, counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED02").await;

        // Prime the cache.
        let primed = cache.inner().list("task").await.unwrap();
        assert_eq!(primed[0].fields.get("change_count"), Some(&json!(1)));
        let after_prime = counter.load(Ordering::Relaxed);

        // Append another changelog entry then write through the cache —
        // the write must invalidate the derived slot so the next list
        // observes the 2-entry changelog.
        let path = cache.inner().changelog_path("task", "01DERIVED02").unwrap();
        let entry = crate::changelog::ChangeEntry::new(
            "task",
            "01DERIVED02",
            "update",
            vec![(
                "title".to_string(),
                crate::changelog::FieldChange::Set { value: json!("v2") },
            )],
        );
        crate::changelog::append_changelog(&path, &entry)
            .await
            .unwrap();

        let mut t = Entity::new("task", "01DERIVED02");
        t.set("title", json!("v2"));
        cache.write(&t).await.unwrap();

        let post = cache.inner().list("task").await.unwrap();
        assert_eq!(
            post[0].fields.get("change_count"),
            Some(&json!(2)),
            "post-write list must observe the new changelog length"
        );
        assert!(
            counter.load(Ordering::Relaxed) > after_prime,
            "write must invalidate the derived cache (re-running derive)"
        );
    }

    /// `cache.delete` must purge the derived-output slot so a later
    /// re-creation of the same id does not see stale memoized values.
    #[tokio::test]
    async fn test_derived_cache_invalidates_on_delete() {
        let (_dir, cache, _counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED03").await;

        // Prime the cache.
        let _ = cache.inner().list("task").await.unwrap();
        {
            let map = cache.derived.read().await;
            assert!(
                map.contains_key(&("task".to_string(), "01DERIVED03".to_string())),
                "derived slot should be populated after a warm list"
            );
        }

        cache.delete("task", "01DERIVED03").await.unwrap();

        // Derived slot must be removed outright (purge, not clear).
        {
            let map = cache.derived.read().await;
            assert!(
                !map.contains_key(&("task".to_string(), "01DERIVED03".to_string())),
                "delete must purge the derived slot"
            );
        }
    }

    /// `cache.evict` must purge the derived-output slot alongside the
    /// primary cached entity.
    #[tokio::test]
    async fn test_derived_cache_invalidates_on_evict() {
        let (_dir, cache, _counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED04").await;

        let _ = cache.inner().list("task").await.unwrap();
        {
            let map = cache.derived.read().await;
            assert!(
                map.contains_key(&("task".to_string(), "01DERIVED04".to_string())),
                "derived slot should be populated after a warm list"
            );
        }

        cache.evict("task", "01DERIVED04").await;

        {
            let map = cache.derived.read().await;
            assert!(
                !map.contains_key(&("task".to_string(), "01DERIVED04".to_string())),
                "evict must purge the derived slot"
            );
        }
    }

    /// `cache.archive` must purge the derived-output slot because the
    /// entity is leaving the live list.
    #[tokio::test]
    async fn test_derived_cache_invalidates_on_archive() {
        let (_dir, cache, _counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED05").await;

        let _ = cache.inner().list("task").await.unwrap();
        {
            let map = cache.derived.read().await;
            assert!(
                map.contains_key(&("task".to_string(), "01DERIVED05".to_string())),
                "derived slot should be populated after a warm list"
            );
        }

        cache.archive("task", "01DERIVED05").await.unwrap();

        {
            let map = cache.derived.read().await;
            assert!(
                !map.contains_key(&("task".to_string(), "01DERIVED05".to_string())),
                "archive must purge the derived slot"
            );
        }
    }

    /// `cache.unarchive` must invalidate the derived-output slot so the
    /// next read re-derives from the restored entity's fresh inputs.
    #[tokio::test]
    async fn test_derived_cache_invalidates_on_unarchive() {
        let (_dir, cache, counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED06").await;

        let _ = cache.inner().list("task").await.unwrap();
        let after_prime = counter.load(Ordering::Relaxed);

        // Round-trip through archive + unarchive. The primary cache
        // removes the entity on archive and reinserts it on unarchive;
        // the derived slot should be cleared at both points so a post-
        // unarchive list re-runs the derive function.
        cache.archive("task", "01DERIVED06").await.unwrap();
        cache.unarchive("task", "01DERIVED06").await.unwrap();

        // A warm post-unarchive list must re-run the derive (the slot
        // was invalidated by unarchive). With a working invalidation,
        // the counter advances; a broken invalidation would serve a
        // stale memoized value silently.
        let _ = cache.inner().list("task").await.unwrap();
        assert!(
            counter.load(Ordering::Relaxed) > after_prime,
            "unarchive must invalidate the derived cache so the next \
             list re-runs the derivation"
        );
    }

    /// `cache.refresh_from_disk` must invalidate the derived-output
    /// slot when it detects a disk-side change so the next read
    /// observes the fresh inputs.
    #[tokio::test]
    async fn test_derived_cache_invalidates_on_refresh_from_disk() {
        let (_dir, cache, counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED07").await;

        let _ = cache.inner().list("task").await.unwrap();
        let after_prime = counter.load(Ordering::Relaxed);

        // Simulate an external edit: rewrite the entity file and append
        // a changelog entry, then refresh from disk — both inputs must
        // invalidate and the derived output should recompute.
        let mut t = Entity::new("task", "01DERIVED07");
        t.set("title", json!("refreshed"));
        cache.inner().write_internal(&t).await.unwrap();

        let path = cache.inner().changelog_path("task", "01DERIVED07").unwrap();
        let entry = crate::changelog::ChangeEntry::new(
            "task",
            "01DERIVED07",
            "update",
            vec![(
                "title".to_string(),
                crate::changelog::FieldChange::Set {
                    value: json!("refreshed"),
                },
            )],
        );
        crate::changelog::append_changelog(&path, &entry)
            .await
            .unwrap();

        let changed = cache
            .refresh_from_disk("task", "01DERIVED07")
            .await
            .unwrap();
        assert!(changed, "refresh_from_disk should observe the change");

        let post = cache.inner().list("task").await.unwrap();
        assert_eq!(
            post[0].fields.get("change_count"),
            Some(&json!(2)),
            "refresh_from_disk must invalidate the derived cache"
        );
        assert!(
            counter.load(Ordering::Relaxed) > after_prime,
            "refresh_from_disk should force a re-derive"
        );
    }

    /// Cross-entity invalidation: a mutation on an entity of type T
    /// must clear the derived-output slot of every entity whose type
    /// has an aggregate field declaring `depends_on: [T]`.
    ///
    /// Builds a two-type registry where `task` has an aggregate field
    /// `count_tags` with `depends_on: [tag]`. The aggregate returns the
    /// number of existing tag entities (queried via the EntityQueryFn).
    /// After a warm task list, writing a new tag must invalidate the
    /// task's derived-output slot so the next list reflects the larger
    /// tag count.
    #[tokio::test]
    async fn test_derived_cache_cross_type_invalidation_on_write() {
        let defs = vec![
            (
                "title",
                "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "body",
                "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
            ),
            (
                "tag_name",
                "id: 00000000000000000000000TNM\nname: tag_name\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "count_tags",
                "id: 00000000000000000000000CNT\nname: count_tags\ntype:\n  kind: computed\n  derive: count-tag-entities\n  depends_on:\n    - tag\n",
            ),
        ];
        let entities = vec![
            (
                "task",
                "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - count_tags\n",
            ),
            ("tag", "name: tag\nfields:\n  - tag_name\n"),
        ];
        let fields_dir = TempDir::new().unwrap();
        let fields = Arc::new(
            FieldsContext::from_yaml_sources(fields_dir.path(), &defs, &entities).unwrap(),
        );

        // Aggregate: count the number of tag entities via the query fn.
        let mut engine = ComputeEngine::new();
        engine.register_aggregate(
            "count-tag-entities",
            Box::new(|_fields, query| {
                Box::pin(async move {
                    let tags = query("tag").await;
                    json!(tags.len())
                })
            }),
        );

        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        std::fs::create_dir_all(root.join("tags")).unwrap();

        let ctx = Arc::new(EntityContext::new(&root, fields).with_compute(Arc::new(engine)));
        let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        ctx.attach_cache(&cache);

        // Create a task; initially there are zero tags.
        let mut t = Entity::new("task", "01CROSS01");
        t.set("title", json!("hello"));
        cache.write(&t).await.unwrap();

        // Warm the derived cache for the task.
        let before = cache.inner().list("task").await.unwrap();
        assert_eq!(
            before[0].fields.get("count_tags"),
            Some(&json!(0)),
            "no tags exist yet, count should be 0"
        );

        // Now write a tag — since task.count_tags declares
        // `depends_on: [tag]`, writing a tag must invalidate the task's
        // cached aggregate output.
        let mut tag = Entity::new("tag", "01TAG01");
        tag.set("tag_name", json!("bug"));
        cache.write(&tag).await.unwrap();

        let after = cache.inner().list("task").await.unwrap();
        assert_eq!(
            after[0].fields.get("count_tags"),
            Some(&json!(1)),
            "cross-type invalidation must force the aggregate to recompute \
             after a mutation on a dependency type; got {:?}",
            after[0].fields.get("count_tags")
        );
    }

    /// Cross-entity invalidation on **delete**: deleting an entity of
    /// type T must clear the derived-output slot of every entity whose
    /// type has an aggregate field declaring `depends_on: [T]`.
    ///
    /// Companion to `test_derived_cache_cross_type_invalidation_on_write`.
    /// The code path is shared through `purge_entity_caches` →
    /// `invalidate_cross_type_derived`, but making the delete-side
    /// matrix entry explicit guards against a future regression that
    /// forgets to wire `purge_entity_caches` into a new delete-like
    /// mutation path (e.g. a future `hard_evict`).
    #[tokio::test]
    async fn test_derived_cache_cross_type_invalidation_on_delete() {
        let defs = vec![
            (
                "title",
                "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "body",
                "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
            ),
            (
                "tag_name",
                "id: 00000000000000000000000TNM\nname: tag_name\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "count_tags",
                "id: 00000000000000000000000CNT\nname: count_tags\ntype:\n  kind: computed\n  derive: count-tag-entities\n  depends_on:\n    - tag\n",
            ),
        ];
        let entities = vec![
            (
                "task",
                "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - count_tags\n",
            ),
            ("tag", "name: tag\nfields:\n  - tag_name\n"),
        ];
        let fields_dir = TempDir::new().unwrap();
        let fields = Arc::new(
            FieldsContext::from_yaml_sources(fields_dir.path(), &defs, &entities).unwrap(),
        );

        // Aggregate: count the number of tag entities via the query fn.
        let mut engine = ComputeEngine::new();
        engine.register_aggregate(
            "count-tag-entities",
            Box::new(|_fields, query| {
                Box::pin(async move {
                    let tags = query("tag").await;
                    json!(tags.len())
                })
            }),
        );

        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        std::fs::create_dir_all(root.join("tags")).unwrap();

        let ctx = Arc::new(EntityContext::new(&root, fields).with_compute(Arc::new(engine)));
        let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        ctx.attach_cache(&cache);

        // Seed a task and two tags so the aggregate starts at 2.
        let mut t = Entity::new("task", "01CROSSDEL01");
        t.set("title", json!("hello"));
        cache.write(&t).await.unwrap();

        let mut tag_a = Entity::new("tag", "01TAGDEL01");
        tag_a.set("tag_name", json!("bug"));
        cache.write(&tag_a).await.unwrap();

        let mut tag_b = Entity::new("tag", "01TAGDEL02");
        tag_b.set("tag_name", json!("feature"));
        cache.write(&tag_b).await.unwrap();

        // Warm the derived cache for the task — expect count_tags == 2.
        let before = cache.inner().list("task").await.unwrap();
        assert_eq!(
            before[0].fields.get("count_tags"),
            Some(&json!(2)),
            "pre-delete state should see both tags"
        );

        // Delete one of the tags. Since task.count_tags declares
        // `depends_on: [tag]`, deleting a tag must invalidate the
        // task's cached aggregate output through the
        // `purge_entity_caches → invalidate_cross_type_derived` path.
        cache.delete("tag", "01TAGDEL01").await.unwrap();

        let after = cache.inner().list("task").await.unwrap();
        assert_eq!(
            after[0].fields.get("count_tags"),
            Some(&json!(1)),
            "cross-type invalidation on delete must force the aggregate \
             to recompute after a dependency-type entity is deleted; got {:?}",
            after[0].fields.get("count_tags")
        );
    }

    /// Directly driving `try_memoize_derived_outputs` with a stale
    /// `observed_epoch` must be rejected by the epoch guard so a
    /// derivation that raced with an invalidation cannot land stale
    /// outputs.
    #[tokio::test]
    async fn test_stale_derived_memoization_is_rejected() {
        let (_dir, cache, _counter) = setup_with_compute().await;
        seed_task(&cache, "01DERIVED08").await;

        // Capture the epoch a hypothetical deriver would have observed
        // before its compute pass started, then force an invalidation
        // so the epoch advances past it.
        let (_, observed_epoch) = cache.get_derived_outputs("task", "01DERIVED08").await;
        cache.invalidate_entity_caches("task", "01DERIVED08").await;
        assert!(
            cache.cache_epoch.load(Ordering::Acquire) > observed_epoch,
            "invalidation must bump the epoch past the deriver's observed value"
        );

        // Drive the memoization path with the stale epoch — the guard
        // must reject it.
        let mut stale_outputs = HashMap::new();
        stale_outputs.insert("change_count".to_string(), json!(42));
        cache
            .try_memoize_derived_outputs("task", "01DERIVED08", observed_epoch, stale_outputs)
            .await;

        let (slot_outputs, _) = cache.get_derived_outputs("task", "01DERIVED08").await;
        assert!(
            slot_outputs.is_none(),
            "stale-deriver memoization must be rejected; got {:?}",
            slot_outputs
        );
    }
}
