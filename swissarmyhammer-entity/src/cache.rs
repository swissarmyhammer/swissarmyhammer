//! In-memory entity cache with content hashing.
//!
//! Provides an `EntityCache` that wraps an `EntityContext` and keeps entities
//! in memory, indexed by `(entity_type, id)`. Each cached entry stores a
//! content hash (computed from serialized YAML of the fields) and a monotonic
//! version counter. Writes delegate to disk through the underlying
//! `EntityContext` and then update the cache.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::broadcast;
use tokio::sync::RwLock;

use crate::context::EntityContext;
use crate::entity::Entity;
use crate::error::Result;
use crate::events::EntityEvent;
use crate::id_types::ChangeEntryId;

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

/// In-memory entity cache backed by an `EntityContext`.
///
/// All reads come from the in-memory map. Writes go through to disk
/// via `EntityContext` first, then update the cache. Content hashing lets
/// callers detect whether an entity actually changed.
/// Default capacity for the broadcast event channel.
const EVENT_CHANNEL_CAPACITY: usize = 256;

pub struct EntityCache {
    inner: EntityContext,
    cache: RwLock<HashMap<(String, String), CachedEntity>>,
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

impl EntityCache {
    /// Create a new cache wrapping the given `EntityContext`.
    pub fn new(inner: EntityContext) -> Self {
        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            inner,
            cache: RwLock::new(HashMap::new()),
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
        let entities = self.inner.list(entity_type).await?;
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
    /// is new). Returns the `ChangeEntryId` from the underlying write (or `None`
    /// if unchanged).
    pub async fn write(&self, entity: &Entity) -> Result<Option<ChangeEntryId>> {
        // Grab the old hash before writing, so we can detect no-op writes.
        let old_hash = {
            let map = self.cache.read().await;
            map.get(&(entity.entity_type.to_string(), entity.id.to_string()))
                .map(|ce| ce.hash)
        };

        let change_id = self.inner.write(entity).await?;

        // Read back the canonical on-disk form so the cached hash matches
        // what refresh_from_disk would compute after a round-trip.
        let canonical = self
            .inner
            .read(&entity.entity_type, &entity.id)
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

        let mut map = self.cache.write().await;
        map.insert(
            key,
            CachedEntity {
                entity: canonical,
                hash: new_hash,
                version,
            },
        );

        if changed {
            let _ = self.event_sender.send(EntityEvent::EntityChanged {
                entity_type: entity.entity_type.to_string(),
                id: entity.id.to_string(),
                version,
            });
        }

        Ok(change_id)
    }

    /// Delete an entity from disk and remove it from the cache.
    ///
    /// Delegates to `inner.delete()` first, then removes the cache entry and
    /// emits an `EntityDeleted` event. Returns the `ChangeEntryId` from the
    /// underlying delete (or `None`).
    pub async fn delete(&self, entity_type: &str, id: &str) -> Result<Option<ChangeEntryId>> {
        let change_id = self.inner.delete(entity_type, id).await?;

        let mut map = self.cache.write().await;
        map.remove(&(entity_type.to_string(), id.to_string()));

        let _ = self.event_sender.send(EntityEvent::EntityDeleted {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
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
        if map.remove(&key).is_some() {
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
    /// content hash matches.
    pub async fn refresh_from_disk(&self, entity_type: &str, id: &str) -> Result<bool> {
        let entity = self.inner.read(entity_type, id).await?;
        let new_hash = hash_entity(&entity);
        let key = (entity_type.to_string(), id.to_string());

        let mut map = self.cache.write().await;
        let changed = match map.get(&key) {
            Some(cached) => cached.hash != new_hash,
            None => true,
        };

        if changed {
            let version = self.bump_version();
            map.insert(
                key,
                CachedEntity {
                    entity,
                    hash: new_hash,
                    version,
                },
            );

            let _ = self.event_sender.send(EntityEvent::EntityChanged {
                entity_type: entity_type.to_string(),
                id: id.to_string(),
                version,
            });
        }

        Ok(changed)
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
        let ctx = EntityContext::new(&root, fields);
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

    /// Writing an entity emits EntityChanged with correct type, id, version.
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
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "t1");
                assert!(version > 0);
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
}
