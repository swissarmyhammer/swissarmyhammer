//! EntityContext — root-aware I/O coordinator for dynamic entities.
//!
//! Given a storage root and a FieldsContext, this handles all directory
//! resolution, file I/O, and changelog management. Consumers (like kanban)
//! create an EntityContext and delegate all entity I/O to it.
//!
//! # Computed fields and pseudo-field dependencies
//!
//! Computed fields (YAML `kind: computed`) can declare `depends_on` entries
//! that name reserved `_`-prefixed pseudo-fields. The entity layer injects
//! these into `entity.fields` before derivation and strips them afterward so
//! they are never persisted or surfaced to callers.
//!
//! Supported pseudo-fields:
//!
//! - **`_changelog`** — the entity's JSONL changelog as a `Value::Array` of
//!   serialized `ChangeEntry` objects. Empty array on missing/unreadable file.
//! - **`_file_created`** — RFC 3339 timestamp from `Metadata::created()`,
//!   falling back to `Metadata::modified()`. `Value::Null` on stat failure.
//!
//! Injection is lazy: a pseudo-field is loaded only when at least one computed
//! field for the entity type declares it in `depends_on`. See
//! [`EntityContext::inject_compute_dependencies`] for the injection logic and
//! [`EntityContext::derive_compute_fields`] for the strip block.
//!
//! To add a new pseudo-field, see the "Computed Fields and Pseudo-Field
//! Dependencies" section in `ARCHITECTURE.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use swissarmyhammer_fields::{
    ComputeEngine, EntityDef, FieldType, FieldsContext, ValidationEngine,
};
use swissarmyhammer_store::{StoreContext, StoreHandle, StoredItemId};
use tokio::sync::RwLock;

use crate::changelog::{self, ChangeEntry, FieldChange};
use crate::entity::{Entity, EntityLocation};
use crate::error::{EntityError, Result};
use crate::id_types::EntityId;
use crate::io;
use crate::store::EntityTypeStore;

/// Root-aware I/O coordinator for dynamic entities.
///
/// Maps entity types to storage directories under a root path,
/// handles read/write/delete/list, and manages per-entity changelogs.
pub struct EntityContext {
    root: PathBuf,
    fields: Arc<FieldsContext>,
    validation: Option<Arc<ValidationEngine>>,
    compute: Option<Arc<ComputeEngine>>,
    /// Optional store handles for entity types.
    /// When present, `write()` and `delete()` delegate file I/O to the store handle
    /// instead of using the legacy `io::write_entity` / `io::trash_entity_files` path.
    store_handles: RwLock<HashMap<String, Arc<StoreHandle<EntityTypeStore>>>>,
    /// Optional shared StoreContext for undo/redo stack management.
    /// When set, write/delete operations automatically push onto the undo stack.
    /// Uses `OnceLock` so it can be set after construction through a shared reference.
    store_context: OnceLock<Arc<StoreContext>>,
    /// Optional in-memory cache. When attached, `read()`, `list()`, and
    /// `write()` delegate to this cache so repeated reads do not hit disk and
    /// writes emit `EntityChanged` events on the cache's broadcast channel.
    /// `OnceLock` guarantees the cache is attached at most once — the cache
    /// and the context form a fixed pairing.
    ///
    /// We store a `Weak` reference to break the Arc cycle: the cache holds
    /// an `Arc<EntityContext>`, and the context holds a reference back to
    /// the cache. Using `Weak` here means dropping the cache drops the cycle.
    cache: OnceLock<std::sync::Weak<crate::cache::EntityCache>>,
}

impl EntityContext {
    /// Create a new EntityContext.
    ///
    /// - `root`: the storage root (e.g. `.kanban/`)
    /// - `fields`: the field registry containing EntityDefs
    pub fn new(root: impl Into<PathBuf>, fields: Arc<FieldsContext>) -> Self {
        Self {
            root: root.into(),
            fields,
            validation: None,
            compute: None,
            store_handles: RwLock::new(HashMap::new()),
            store_context: OnceLock::new(),
            cache: OnceLock::new(),
        }
    }

    /// Attach an `EntityCache` so that `read`, `list`, and `write` delegate
    /// to it instead of hitting disk on every call.
    ///
    /// Takes `&Self` (not `self`) because the cache and context form an
    /// `Arc` cycle — the cache owns an `Arc<EntityContext>`, and this method
    /// installs a `Weak` reference to that same cache on the context. Callers
    /// construct the cache from the context's `Arc`, then attach it back
    /// through this method.
    ///
    /// Uses `OnceLock`: only the first call wins. Subsequent calls are no-ops.
    /// Panics if called after the cache has already been set (which would
    /// indicate a programming error in the wiring layer).
    pub fn attach_cache(&self, cache: &Arc<crate::cache::EntityCache>) {
        let weak = Arc::downgrade(cache);
        self.cache
            .set(weak)
            .expect("EntityContext::attach_cache called more than once");
    }

    /// Return the attached cache, if any. `None` when no cache has been
    /// installed or the cache has been dropped.
    fn attached_cache(&self) -> Option<Arc<crate::cache::EntityCache>> {
        self.cache.get().and_then(|w| w.upgrade())
    }

    /// Set the StoreContext for shared undo/redo stack management.
    ///
    /// When set, `write()` and `delete()` automatically push successful
    /// operations onto the shared undo stack. Can be called through a
    /// shared reference since it uses `OnceLock` internally. Subsequent
    /// calls are no-ops (first write wins).
    pub fn set_store_context(&self, ctx: Arc<StoreContext>) {
        let _ = self.store_context.set(ctx);
    }

    /// Attach a validation engine. Enables field validation on write.
    pub fn with_validation(mut self, engine: Arc<ValidationEngine>) -> Self {
        self.validation = Some(engine);
        self
    }

    /// Attach a compute engine. Enables computed field derivation on read.
    pub fn with_compute(mut self, engine: Arc<ComputeEngine>) -> Self {
        self.compute = Some(engine);
        self
    }

    /// Register a `StoreHandle` for an entity type.
    ///
    /// When registered, `write()` and `delete()` delegate file I/O to the store
    /// handle instead of using the legacy `io::write_entity` / `io::trash_entity_files`
    /// path. The old per-entity changelog continues to be written for activity history.
    pub async fn register_store(
        &self,
        entity_type: &str,
        handle: Arc<StoreHandle<EntityTypeStore>>,
    ) {
        self.store_handles
            .write()
            .await
            .insert(entity_type.to_string(), handle);
    }

    /// Get the storage root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the FieldsContext.
    pub fn fields(&self) -> &FieldsContext {
        &self.fields
    }

    /// Look up the EntityDef for an entity type.
    pub fn entity_def(&self, entity_type: impl AsRef<str>) -> Result<&EntityDef> {
        let entity_type = entity_type.as_ref();
        self.fields
            .get_entity(entity_type)
            .ok_or_else(|| EntityError::UnknownEntityType {
                entity_type: entity_type.into(),
            })
    }

    /// Get the storage directory for an entity type.
    ///
    /// Maps entity type → `{root}/{type}s/` (e.g. "task" → "tasks/",
    /// "board" → "boards/").
    pub fn entity_dir(&self, entity_type: impl AsRef<str>) -> PathBuf {
        self.root.join(format!("{}s", entity_type.as_ref()))
    }

    /// Get the file path for a specific entity.
    ///
    /// Includes the correct extension (.md or .yaml) based on the EntityDef.
    pub fn entity_path(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<PathBuf> {
        let entity_type = entity_type.as_ref();
        let def = self.entity_def(entity_type)?;
        Ok(io::entity_file_path(&self.entity_dir(entity_type), id, def))
    }

    /// Get the changelog path for a specific entity.
    pub fn changelog_path(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<PathBuf> {
        let path = self.entity_path(entity_type, id)?;
        Ok(path.with_extension("jsonl"))
    }

    /// Get the trash directory for an entity type.
    ///
    /// Maps entity type → `{root}/{type}s/.trash/` (e.g. "task" → "tasks/.trash/").
    ///
    /// Each entity type's directory is self-contained: live, trashed, and archived
    /// files all live under the same parent (`{type}s/`).
    pub fn trash_dir(&self, entity_type: impl AsRef<str>) -> PathBuf {
        self.entity_dir(entity_type).join(".trash")
    }

    /// Get the archive directory for an entity type.
    ///
    /// Maps entity type → `{root}/{type}s/.archive/` (e.g. "task" → "tasks/.archive/").
    /// Archived entities are excluded from `list()` but remain accessible via
    /// `list_archived()` and `read_archived()`.
    pub fn archive_dir(&self, entity_type: impl AsRef<str>) -> PathBuf {
        self.entity_dir(entity_type).join(".archive")
    }

    /// Read a single entity by type and ID.
    ///
    /// When a cache is attached via [`attach_cache`], pulls the raw
    /// cached entity and applies compute fresh. Cache misses fall through
    /// to disk — misses are rare in practice because `KanbanContext`
    /// preloads every registered entity type on startup, but they are
    /// still possible for lazily-added types or files that appeared after
    /// `load_all`.
    ///
    /// If a `ComputeEngine` is attached, computed fields are derived after reading.
    pub async fn read(&self, entity_type: impl AsRef<str>, id: impl AsRef<str>) -> Result<Entity> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        if let Some(cache) = self.attached_cache() {
            if let Some(mut entity) = cache.get(entity_type, id).await {
                self.apply_compute(entity_type, &mut entity).await?;
                return Ok(entity);
            }
        }
        self.read_internal(entity_type, id).await
    }

    /// Read a single entity directly from disk, bypassing any attached cache.
    ///
    /// Used by `read()` as the fall-through path on a cache miss. Always
    /// applies the attached `ComputeEngine` so callers get the same shape
    /// whether they hit cache or disk.
    pub(crate) async fn read_internal(&self, entity_type: &str, id: &str) -> Result<Entity> {
        let mut entity = self.read_raw_internal(entity_type, id).await?;
        self.apply_compute(entity_type, &mut entity).await?;
        Ok(entity)
    }

    /// Read a single entity from disk without applying any compute.
    ///
    /// Used by the cache to store canonical disk-form entities. Aggregate
    /// compute fields (like `parse-body-tags` whose output depends on
    /// sibling entity types) must be re-evaluated on every read out of the
    /// cache to stay correct under cross-type writes — caching their
    /// output would mean stale data whenever a sibling entity changes.
    pub(crate) async fn read_raw_internal(&self, entity_type: &str, id: &str) -> Result<Entity> {
        let def = self.entity_def(entity_type)?;
        let path = io::entity_file_path(&self.entity_dir(entity_type), id, def);
        io::read_entity(&path, entity_type, id, def).await
    }

    /// Write an entity, routing through the cache when one is attached.
    ///
    /// When a cache is attached via [`attach_cache`], this method delegates
    /// to [`EntityCache::write`], which handles hashing, versioning, and event
    /// emission on top of the underlying disk write. Without a cache it falls
    /// through to [`write_internal`] directly.
    ///
    /// Returns `Ok(Some(ulid))` when changes were logged, or `Ok(None)` when
    /// no changes were detected (idempotent write).
    pub async fn write(
        &self,
        entity: &Entity,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        if let Some(cache) = self.attached_cache() {
            return cache.write(entity).await;
        }
        self.write_internal(entity).await
    }

    /// Write an entity directly to disk, bypassing any attached cache.
    ///
    /// This is the pure disk-write path: validation, attachment handling,
    /// store-handle delegation, changelog append, and undo-stack push. It is
    /// the fallback called by `write()` when no cache is attached, and the
    /// method the cache itself calls to avoid recursing back through its own
    /// write path.
    ///
    /// If a `ValidationEngine` is attached, fields are validated/transformed
    /// before writing. Computed fields are stripped (they are derived on read).
    /// If a previous version exists, diffs against it and appends a changelog
    /// entry. On creation (no previous version), all fields are logged as `Set`.
    ///
    /// Returns `Ok(Some(ulid))` when changes were logged, or `Ok(None)` when
    /// no changes were detected (idempotent write).
    pub(crate) async fn write_internal(
        &self,
        entity: &Entity,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let def = self.entity_def(&entity.entity_type)?;
        let entity = self.validate_for_write(entity).await?;

        let dir = self.entity_dir(&entity.entity_type);
        let path = io::entity_file_path(&dir, &entity.id, def);

        // Trash attachment files that were removed during update
        let previous = io::read_entity(&path, &entity.entity_type, &entity.id, def)
            .await
            .ok();
        if let Some(ref old) = previous {
            self.trash_removed_attachments(&entity.entity_type, old, &entity)
                .await?;
        }

        // Write — delegate to StoreHandle when available, otherwise
        // fall back to the legacy io::write_entity path.
        let store_handle = self
            .store_handles
            .read()
            .await
            .get(entity.entity_type.as_str())
            .cloned();

        let Some(sh) = store_handle else {
            // Fallback for tests or entity types without a registered store
            io::write_entity(&path, &entity, def).await?;
            return Ok(None);
        };

        let entry_id = sh.write(&entity).await?;

        // Append a legacy field-level changelog entry so that the activity
        // log (which reads per-entity JSONL) continues to work even when
        // I/O is delegated to a StoreHandle.
        if entry_id.is_some() {
            self.append_write_changelog(&entity, previous.as_ref(), &path)
                .await?;
        }

        // Push onto the shared undo stack if a StoreContext is available
        if let (Some(sc), Some(eid)) = (self.store_context.get(), &entry_id) {
            let is_create = previous.is_none();
            let op = if is_create { "create" } else { "update" };
            let label = format!("{} {} {}", op, entity.entity_type, entity.id);
            let item_id = StoredItemId::from(entity.id.as_str());
            sc.push(*eid, label, item_id).await;
        }
        Ok(entry_id)
    }

    /// Append a field-level changelog entry for a write operation.
    ///
    /// Computes the diff between the previous entity state and the current
    /// one (or treats all fields as `Set` for creates) and appends the
    /// resulting `ChangeEntry` to the entity's JSONL changelog.
    async fn append_write_changelog(
        &self,
        entity: &Entity,
        previous: Option<&Entity>,
        path: &Path,
    ) -> Result<()> {
        let is_create = previous.is_none();
        let op = if is_create { "create" } else { "update" };
        let changes = if let Some(old) = previous {
            changelog::diff_entities(old, entity)
        } else {
            entity
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
                .collect()
        };
        if changes.is_empty() {
            return Ok(());
        }
        let entry = ChangeEntry::new(entity.entity_type.as_str(), entity.id.as_str(), op, changes);
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &entry).await?;
        Ok(())
    }

    /// Delete an entity by type and ID.
    ///
    /// When a cache is attached via [`attach_cache`], this delegates to
    /// [`EntityCache::delete`] which updates the cache map and emits an
    /// `EntityDeleted` event on top of the disk trash operation.
    ///
    /// Moves the data file to the trash directory (`{root}/{type}s/.trash/`).
    /// The entity is no longer listed or readable, but its files are
    /// preserved for recovery.
    ///
    /// Returns `Ok(Some(entry_id))` when a store handle processes the
    /// delete, or `Ok(None)` for the legacy fallback path.
    pub async fn delete(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        if let Some(cache) = self.attached_cache() {
            return cache.delete(entity_type, id).await;
        }
        self.delete_internal(entity_type, id).await
    }

    /// Delete an entity directly from disk, bypassing any attached cache.
    ///
    /// This is the pure disk-delete path used as the fallback in `delete()`
    /// and called by the cache itself to avoid recursing through its own
    /// delete path.
    pub(crate) async fn delete_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);

        // Read existing entity before deletion (used for attachment cleanup
        // and legacy changelog).
        let previous = io::read_entity(&path, entity_type, id, def).await.ok();
        if let Some(ref old) = previous {
            self.trash_entity_attachments(entity_type, old).await?;
        }

        // Delete — delegate to StoreHandle when available, otherwise
        // fall back to the legacy io::trash_entity_files path.
        let store_handle = self.store_handles.read().await.get(entity_type).cloned();
        if let Some(sh) = store_handle {
            // Append a legacy field-level changelog entry BEFORE the store
            // handle trashes the file, so the entry gets included in the
            // trashed changelog and activity history remains intact.
            if let Some(ref old) = previous {
                let mut changes: Vec<_> = old
                    .fields
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            FieldChange::Removed {
                                old_value: v.clone(),
                            },
                        )
                    })
                    .collect();
                changes.sort_by(|a, b| a.0.cmp(&b.0));
                if !changes.is_empty() {
                    let entry = ChangeEntry::new(entity_type, id, "delete", changes);
                    let log_path = path.with_extension("jsonl");
                    changelog::append_changelog(&log_path, &entry).await?;
                }
            }

            let entity_id = EntityId::from(id);
            let entry_id = sh.delete(&entity_id).await?;

            // Push onto the shared undo stack if a StoreContext is available
            if let Some(sc) = self.store_context.get() {
                let label = format!("delete {} {}", entity_type, id);
                let item_id = StoredItemId::from(id);
                sc.push(entry_id, label, item_id).await;
            }
            Ok(Some(entry_id))
        } else {
            // Fallback for tests or entity types without a registered store
            let trash = self.trash_dir(entity_type);
            io::trash_entity_files(&path, &trash).await?;
            Ok(None)
        }
    }

    /// Restore an entity from trash back to live storage.
    ///
    /// Moves the entity data file and changelog from the trash directory
    /// (`{root}/{type}s/.trash/`) back to the live storage directory.
    /// This is the inverse of the trash operation performed by `delete()`.
    pub async fn restore_from_trash(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<()> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        self.restore_from_trash_internal(entity_type, id).await?;
        // Refresh the cache so the restored entity shows up in `list()`.
        if let Some(cache) = self.attached_cache() {
            let _ = cache.refresh_from_disk(entity_type, id).await;
        }
        Ok(())
    }

    /// Pure disk restore-from-trash, bypassing any attached cache.
    pub(crate) async fn restore_from_trash_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<()> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);
        let trash = self.trash_dir(entity_type);
        io::restore_entity_files(&path, &trash).await
    }

    /// Restore an entity from the archive back to live storage.
    ///
    /// Moves the entity data file and changelog from the archive directory
    /// (`{root}/{type}s/.archive/`) back to the live storage directory.
    /// This is the inverse of the archive operation performed by `archive()`.
    pub async fn restore_from_archive(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<()> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        self.restore_from_archive_internal(entity_type, id).await?;
        // Refresh the cache so the restored entity shows up in `list()`.
        if let Some(cache) = self.attached_cache() {
            let _ = cache.refresh_from_disk(entity_type, id).await;
        }
        Ok(())
    }

    /// Pure disk restore-from-archive, bypassing any attached cache.
    pub(crate) async fn restore_from_archive_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<()> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);
        let archive = self.archive_dir(entity_type);
        io::restore_entity_files(&path, &archive).await
    }

    /// Archive an entity by type and ID.
    ///
    /// When a cache is attached via [`attach_cache`], this routes through
    /// [`EntityCache::archive`], which removes the archived entity from the
    /// in-memory map so `list()` no longer surfaces it.
    ///
    /// When a StoreHandle is registered for the entity type, delegates to
    /// `StoreHandle::archive()` which records an undoable changelog entry and
    /// moves files to `.archive/` with versioned filenames.
    ///
    /// Falls back to legacy behavior (activity-only changelog + plain file move)
    /// when no StoreHandle is available.
    ///
    /// Returns `Ok(Some(entry_id))` when a store handle processes the
    /// archive, or `Ok(None)` for the legacy fallback path.
    pub async fn archive(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        if let Some(cache) = self.attached_cache() {
            return cache.archive(entity_type, id).await;
        }
        self.archive_internal(entity_type, id).await
    }

    /// Archive an entity directly on disk, bypassing any attached cache.
    ///
    /// This is the pure archive path — the cache itself calls it, and
    /// `archive()` falls through to it when no cache is attached.
    pub(crate) async fn archive_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);

        // Archive — delegate to StoreHandle when available, otherwise
        // fall back to the legacy io::trash_entity_files path.
        let store_handle = self.store_handles.read().await.get(entity_type).cloned();
        if let Some(sh) = store_handle {
            let entity_id = EntityId::from(id);
            let entry_id = sh.archive(&entity_id).await?;
            // Push onto the shared undo stack if a StoreContext is available
            if let Some(sc) = self.store_context.get() {
                let label = format!("archive {} {}", entity_type, id);
                let item_id = StoredItemId::from(id);
                sc.push(entry_id, label, item_id).await;
            }
            Ok(Some(entry_id))
        } else {
            // Fallback: append an "archive" changelog entry for activity history
            if let Ok(old) = io::read_entity(&path, entity_type, id, def).await {
                let mut changes: Vec<_> = old
                    .fields
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            FieldChange::Removed {
                                old_value: v.clone(),
                            },
                        )
                    })
                    .collect();
                changes.sort_by(|a, b| a.0.cmp(&b.0));

                if !changes.is_empty() {
                    let entry = ChangeEntry::new(entity_type, id, "archive", changes);
                    let log_path = path.with_extension("jsonl");
                    changelog::append_changelog(&log_path, &entry).await?;
                }
            }

            let archive = self.archive_dir(entity_type);
            io::trash_entity_files(&path, &archive).await?;
            Ok(None)
        }
    }

    /// Restore an entity from the archive back to live storage.
    ///
    /// When a cache is attached via [`attach_cache`], this routes through
    /// [`EntityCache::unarchive`], which re-reads the restored entity from
    /// disk and inserts it back into the in-memory map.
    ///
    /// When a StoreHandle is registered for the entity type, delegates to
    /// `StoreHandle::unarchive_latest()` which finds the most recently
    /// archived version, restores it, and records an undoable changelog entry.
    ///
    /// Falls back to legacy behavior (plain file move + activity changelog)
    /// when no StoreHandle is available.
    ///
    /// Returns `Ok(Some(entry_id))` when a store handle processes the
    /// unarchive, or `Ok(None)` for the legacy fallback path.
    pub async fn unarchive(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        if let Some(cache) = self.attached_cache() {
            return cache.unarchive(entity_type, id).await;
        }
        self.unarchive_internal(entity_type, id).await
    }

    /// Unarchive an entity directly on disk, bypassing any attached cache.
    pub(crate) async fn unarchive_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Option<swissarmyhammer_store::UndoEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);

        // Unarchive — delegate to StoreHandle when available
        let store_handle = self.store_handles.read().await.get(entity_type).cloned();
        if let Some(sh) = store_handle {
            let entity_id = EntityId::from(id);
            let (_item, entry_id) = sh.unarchive_latest(&entity_id).await?;
            // Push onto the shared undo stack if a StoreContext is available
            if let Some(sc) = self.store_context.get() {
                let label = format!("unarchive {} {}", entity_type, id);
                let item_id = StoredItemId::from(id);
                sc.push(entry_id, label, item_id).await;
            }
            Ok(Some(entry_id))
        } else {
            // Fallback: legacy file move + activity changelog
            let archive = self.archive_dir(entity_type);
            io::restore_entity_files(&path, &archive).await?;

            let entity = io::read_entity(&path, entity_type, id, def).await?;
            let mut changes: Vec<_> = entity
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
                .collect();
            changes.sort_by(|a, b| a.0.cmp(&b.0));

            if !changes.is_empty() {
                let entry = ChangeEntry::new(entity_type, id, "unarchive", changes);
                let log_path = path.with_extension("jsonl");
                changelog::append_changelog(&log_path, &entry).await?;
            }

            Ok(None)
        }
    }

    /// List all archived entities of a given type.
    ///
    /// Reads from the archive directory (`{root}/{type}s/.archive/`).
    /// If a `ComputeEngine` is attached, computed fields are derived for each entity.
    pub async fn list_archived(&self, entity_type: impl AsRef<str>) -> Result<Vec<Entity>> {
        let entity_type = entity_type.as_ref();
        let def = self.entity_def(entity_type)?;
        let dir = self.archive_dir(entity_type);
        let mut entities = io::read_entity_dir(&dir, entity_type, def).await?;
        for entity in &mut entities {
            entity.location = EntityLocation::Archive;
        }
        if self.compute.is_some() {
            let query_fn = self.build_entity_query_fn();
            for entity in &mut entities {
                self.apply_compute_with_query(entity_type, entity, &query_fn)
                    .await?;
            }
        }
        Ok(entities)
    }

    /// Read a single archived entity by type and ID.
    ///
    /// Reads from the archive directory (`{root}/{type}s/.archive/`).
    /// If a `ComputeEngine` is attached, computed fields are derived after reading.
    pub async fn read_archived(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Entity> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        let def = self.entity_def(entity_type)?;
        let path = io::entity_file_path(&self.archive_dir(entity_type), id, def);
        let mut entity = io::read_entity(&path, entity_type, id, def).await?;
        entity.location = EntityLocation::Archive;
        self.apply_compute(entity_type, &mut entity).await?;
        Ok(entity)
    }

    /// Migrate old trash layout to the new layout.
    ///
    /// Old layout: `{root}/.trash/{type}s/` (e.g. `.kanban/.trash/tasks/`)
    /// New layout: `{root}/{type}s/.trash/` (e.g. `.kanban/tasks/.trash/`)
    ///
    /// If the old layout exists for a given entity type, moves all files from the
    /// old directory to the new directory. Removes the old directory when empty.
    /// This is idempotent — if the old layout doesn't exist, this is a no-op.
    pub async fn migrate_trash_layout(&self, entity_type: impl AsRef<str>) -> Result<()> {
        let entity_type = entity_type.as_ref();
        let old_trash = self.root.join(".trash").join(format!("{}s", entity_type));
        let new_trash = self.trash_dir(entity_type);

        if !old_trash.exists() {
            return Ok(());
        }

        tokio::fs::create_dir_all(&new_trash).await?;

        let mut entries = tokio::fs::read_dir(&old_trash).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src = entry.path();
            let filename = entry.file_name();
            let dest = new_trash.join(&filename);
            // Move file; skip if destination already exists
            match tokio::fs::rename(&src, &dest).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(crate::error::EntityError::Io(e)),
            }
        }

        // Remove old directory if now empty
        let _ = tokio::fs::remove_dir(&old_trash).await;

        // Try to remove the parent `.trash/` if empty
        let old_trash_root = self.root.join(".trash");
        let _ = tokio::fs::remove_dir(&old_trash_root).await;

        Ok(())
    }

    /// List all entities of a given type.
    ///
    /// When a cache is attached via [`attach_cache`], pulls raw entities
    /// from the in-memory map and applies compute fresh on the way out —
    /// no `read_entity_dir` call, no disk parsing, but aggregate computed
    /// fields (like body-tag parsing that queries sibling entity types)
    /// still reflect the current state of every entity type.
    ///
    /// Per-entity compute runs concurrently (bounded fan-out) so a large
    /// board doesn't serialize the per-task `_changelog` / `_file_created`
    /// disk reads that `apply_compute_with_query` injects.
    ///
    /// Without a cache, falls through to [`list_internal`].
    pub async fn list(&self, entity_type: impl AsRef<str>) -> Result<Vec<Entity>> {
        let entity_type = entity_type.as_ref();
        if let Some(cache) = self.attached_cache() {
            // Validate the type exists — the cache silently returns an
            // empty vec for unknown types, which would hide real bugs.
            let _ = self.entity_def(entity_type)?;
            let mut entities = cache.get_all(entity_type).await;
            if self.compute.is_some() {
                self.apply_compute_batch(entity_type, &mut entities).await?;
            }
            return Ok(entities);
        }
        self.list_internal(entity_type).await
    }

    /// List all entities of a type by reading them from disk, bypassing any
    /// attached cache.
    ///
    /// Used by `list()` as the fallback when no cache is attached. Applies
    /// the attached `ComputeEngine` to each entity concurrently (bounded
    /// fan-out) so the per-entity `_changelog` disk reads don't serialize.
    pub(crate) async fn list_internal(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let mut entities = self.list_raw_internal(entity_type).await?;
        if self.compute.is_some() {
            self.apply_compute_batch(entity_type, &mut entities).await?;
        }
        Ok(entities)
    }

    /// List all entities of a type from disk without applying any compute.
    ///
    /// Used by the cache's `load_all` to seed itself with canonical
    /// disk-form entities. See [`read_raw_internal`] for why the cache
    /// stores pre-compute data.
    pub(crate) async fn list_raw_internal(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        io::read_entity_dir(&dir, entity_type, def).await
    }

    /// List entities of a type, filtered by a predicate with access to context.
    ///
    /// Loads all entities first (with computed fields derived), builds an
    /// `EntityFilterContext` via the `build_ctx` callback, then keeps only
    /// entities where `predicate` returns `true`.
    ///
    /// The `build_ctx` callback receives the loaded entity slice and returns
    /// a populated `EntityFilterContext` — this is where callers inject
    /// domain-specific extras (tag registries, column IDs, etc.) without the
    /// entity layer knowing about those types.
    pub async fn list_where<F>(
        &self,
        entity_type: impl AsRef<str>,
        build_ctx: impl FnOnce(&[Entity]) -> crate::filter::EntityFilterContext<'_>,
        predicate: F,
    ) -> Result<Vec<Entity>>
    where
        F: Fn(&Entity, &crate::filter::EntityFilterContext) -> bool,
    {
        let mut entities = self.list(entity_type).await?;
        let ctx = build_ctx(&entities);
        // Collect passing indices while ctx borrows entities, then drop ctx
        // before draining. This satisfies the borrow checker without cloning.
        let keep: Vec<bool> = entities.iter().map(|e| predicate(e, &ctx)).collect();
        drop(ctx);
        let mut i = 0;
        entities.retain(|_| {
            let pass = keep[i];
            i += 1;
            pass
        });
        Ok(entities)
    }

    /// Read the changelog for an entity.
    pub async fn read_changelog(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Vec<ChangeEntry>> {
        let log_path = self.changelog_path(entity_type, id)?;
        changelog::read_changelog(&log_path).await
    }

    /// Read the changelog for an entity, falling back to the trash directory
    /// if the live changelog does not exist (e.g. the entity was deleted),
    /// and further falling back to the archive directory if neither the live
    /// nor trash changelog exists (e.g. the entity was archived).
    pub async fn read_changelog_with_trash_fallback(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Vec<ChangeEntry>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        let live_path = self.changelog_path(entity_type, id)?;
        let def = self.entity_def(entity_type)?;
        let file_stem = io::entity_file_path(&self.entity_dir(entity_type), id, def)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(id)
            .to_string();

        let trash_dir = self.trash_dir(entity_type);
        let trash_path = trash_dir.join(format!("{file_stem}.jsonl"));

        // Try live first, then trash, then archive
        let entries = changelog::read_changelog_with_fallback(&live_path, &trash_path).await?;
        if entries.is_empty() && !live_path.exists() && !trash_path.exists() {
            let archive_dir = self.archive_dir(entity_type);
            let archive_path = archive_dir.join(format!("{file_stem}.jsonl"));
            return changelog::read_changelog(&archive_path).await;
        }
        Ok(entries)
    }

    // =========================================================================
    // Internal: validation and computation
    // =========================================================================

    /// Validate fields on write and strip computed fields.
    ///
    /// For each field defined on the entity type:
    /// Validate and prepare an entity for writing to disk.
    ///
    /// This is the domain-level validation layer that runs before storage.
    /// It clones the entity and returns a cleaned version ready for persistence:
    ///
    /// 1. Strips computed fields (they are derived on read, never persisted).
    /// 2. Applies field defaults for missing non-computed fields.
    /// 3. Runs field-level validation via the ValidationEngine (if present).
    /// 4. Runs entity-level cross-field validation (if present).
    ///
    /// Callers can use this independently of `write()` to validate an entity
    /// before passing it to a `StoreHandle`.
    pub async fn validate_for_write(&self, entity: &Entity) -> Result<Entity> {
        let mut entity = entity.clone();
        let entity_type = entity.entity_type.to_string();
        let field_defs = self.fields.fields_for_entity(&entity_type);
        if field_defs.is_empty() {
            return Ok(entity);
        }

        // Strip computed fields — they must never be persisted.
        for fd in &field_defs {
            if matches!(&fd.type_, FieldType::Computed { .. }) {
                entity.fields.remove(fd.name.as_str());
            }
        }

        // Apply defaults for missing non-computed fields
        for fd in &field_defs {
            if matches!(&fd.type_, FieldType::Computed { .. }) {
                continue;
            }
            if entity.fields.contains_key(fd.name.as_str()) {
                continue;
            }
            let Some(ref default) = fd.default else {
                continue;
            };
            entity.set(fd.name.to_string(), default.clone());
        }

        // Process attachment fields — copy source files, validate sizes.
        let entity_type_dir = self.entity_dir(&entity_type);
        for fd in &field_defs {
            let FieldType::Attachment {
                max_bytes,
                multiple,
            } = &fd.type_
            else {
                continue;
            };
            self.process_attachment_field(
                &mut entity,
                fd.name.as_str(),
                *max_bytes,
                *multiple,
                &entity_type_dir,
            )
            .await?;
        }

        // Validate fields
        let Some(ref engine) = self.validation else {
            return Ok(entity);
        };

        // Collect field names to validate (avoid borrowing entity.fields while mutating)
        let names_to_validate: Vec<String> = field_defs
            .iter()
            .filter(|fd| !matches!(&fd.type_, FieldType::Computed { .. }))
            .filter(|fd| entity.fields.contains_key(fd.name.as_str()))
            .map(|fd| fd.name.to_string())
            .collect();

        // Snapshot sibling fields once before the loop — validation functions
        // see a consistent view of the entity, not partially-validated state.
        let siblings = entity.fields.clone();

        for name in &names_to_validate {
            let fd = field_defs.iter().find(|f| f.name == name.as_str()).unwrap();
            let value = entity.fields.get(name).cloned().unwrap();
            let validated = engine.validate(fd, value, &siblings).await.map_err(|e| {
                EntityError::ValidationFailed {
                    field: name.clone(),
                    message: e.to_string(),
                }
            })?;
            entity.set(name.clone(), validated);
        }

        // Entity-level cross-field validation (runs after all field validations)
        let entity_def = self.entity_def(&entity_type)?;
        engine
            .validate_entity(entity_def, &mut entity.fields)
            .await
            .map_err(|e| EntityError::ValidationFailed {
                field: format!("entity:{}", entity_type),
                message: e.to_string(),
            })?;

        Ok(entity)
    }

    /// Process a single attachment field during validation.
    ///
    /// For each value in the field:
    /// - If the value is a path to an existing file on disk, copy it into
    ///   `.attachments/` and replace the value with the stored filename.
    /// - If the value already names a file in `.attachments/`, leave it alone.
    /// - For `multiple: true`, the value is an array of strings.
    async fn process_attachment_field(
        &self,
        entity: &mut Entity,
        field_name: &str,
        max_bytes: u64,
        multiple: bool,
        entity_type_dir: &Path,
    ) -> Result<()> {
        use serde_json::Value;

        let Some(value) = entity.fields.get(field_name).cloned() else {
            return Ok(());
        };

        if !multiple {
            let resolved = self
                .resolve_single_attachment(value, field_name, max_bytes, entity_type_dir)
                .await?;
            if let Some(stored) = resolved {
                entity.set(field_name, Value::String(stored));
            }
            return Ok(());
        }

        // Array of attachment values
        let values = match value {
            Value::Array(arr) => arr,
            Value::Null => return Ok(()),
            other => vec![other],
        };
        let mut result = Vec::new();
        for v in values {
            let resolved = self
                .resolve_single_attachment(v, field_name, max_bytes, entity_type_dir)
                .await?;
            if let Some(stored) = resolved {
                result.push(Value::String(stored));
            }
        }
        entity.set(field_name, Value::Array(result));

        Ok(())
    }

    /// Resolve a single attachment value of any shape to its stored filename.
    ///
    /// Handles three cases:
    /// - `Value::String` — delegates to [`resolve_attachment_value`] (copy or
    ///   verify existing).
    /// - `Value::Object` — enriched metadata round-trip; reconstructs the
    ///   `{id}-{name}` filename and verifies it in `.attachments/`.
    /// - Anything else — logs a warning and returns `None`.
    async fn resolve_single_attachment(
        &self,
        value: serde_json::Value,
        field_name: &str,
        max_bytes: u64,
        entity_type_dir: &Path,
    ) -> Result<Option<String>> {
        match value {
            serde_json::Value::String(s) => {
                let stored = self
                    .resolve_attachment_value(&s, field_name, max_bytes, entity_type_dir)
                    .await?;
                Ok(Some(stored))
            }
            serde_json::Value::Object(ref obj) => {
                self.resolve_enriched_attachment(obj, field_name, entity_type_dir)
                    .await
            }
            serde_json::Value::Null => Ok(None),
            other => {
                tracing::warn!(
                    field = field_name,
                    value = ?other,
                    "skipping non-string/non-object attachment value"
                );
                Ok(None)
            }
        }
    }

    /// Reconstruct a stored filename from an enriched metadata object and
    /// verify it still exists in `.attachments/`.
    ///
    /// Returns `Ok(Some(filename))` when valid, `Ok(None)` when the object
    /// lacks the required `id`/`name` keys, or an error when the file is
    /// missing from disk.
    async fn resolve_enriched_attachment(
        &self,
        obj: &serde_json::Map<String, serde_json::Value>,
        field_name: &str,
        entity_type_dir: &Path,
    ) -> Result<Option<String>> {
        let (Some(id), Some(name)) = (
            obj.get("id").and_then(|v| v.as_str()),
            obj.get("name").and_then(|v| v.as_str()),
        ) else {
            return Ok(None);
        };
        // Reject path separators to prevent directory traversal via
        // crafted enriched metadata objects.
        if id.contains('/') || id.contains('\\') || name.contains('/') || name.contains('\\') {
            return Err(EntityError::AttachmentNotFound {
                field: field_name.to_string(),
                filename: format!("{}-{}", id, name),
            });
        }
        let stored = format!("{}-{}", id, name);
        let att_dir = crate::io::attachments_dir(entity_type_dir);
        let path = att_dir.join(&stored);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(Some(stored));
        }
        Err(EntityError::AttachmentNotFound {
            field: field_name.to_string(),
            filename: stored,
        })
    }

    /// Resolve a single attachment value: either an existing stored filename
    /// or a source file path to copy.
    ///
    /// Returns the stored filename to persist in the YAML.
    async fn resolve_attachment_value(
        &self,
        value: &str,
        field_name: &str,
        max_bytes: u64,
        entity_type_dir: &Path,
    ) -> Result<String> {
        // Only check .attachments/ for bare filenames (no path separators).
        // Values containing '/' or '\' are always treated as source file paths
        // to copy, preventing PathBuf::join from replacing the base when given
        // an absolute path.
        if !value.contains('/') && !value.contains('\\') {
            let att_dir = io::attachments_dir(entity_type_dir);
            let existing = att_dir.join(value);
            if tokio::fs::try_exists(&existing).await.unwrap_or(false) {
                return Ok(value.to_string());
            }
        }

        // Treat as a source file path to copy
        let source = Path::new(value);
        io::copy_attachment(source, entity_type_dir, field_name, max_bytes).await
    }

    /// Build a read-only entity query function for aggregate computed fields.
    ///
    /// The query returns raw entities (without applying compute) to avoid
    /// infinite recursion. When an `EntityCache` is attached, queries serve
    /// from the in-memory map; otherwise they fall through to disk.
    fn build_entity_query_fn(&self) -> std::sync::Arc<swissarmyhammer_fields::EntityQueryFn> {
        let root = self.root.clone();
        let fields_ctx = Arc::clone(&self.fields);
        let cache_weak = self.cache.get().cloned();
        std::sync::Arc::new(Box::new(move |et: &str| {
            let root = root.clone();
            let fields_ctx = Arc::clone(&fields_ctx);
            let cache_weak = cache_weak.clone();
            let et = et.to_string();
            Box::pin(async move {
                if let Some(cache) = cache_weak.as_ref().and_then(|w| w.upgrade()) {
                    return cache
                        .get_all(&et)
                        .await
                        .into_iter()
                        .map(|e| e.fields)
                        .collect();
                }
                let Some(def) = fields_ctx.get_entity(&et) else {
                    return vec![];
                };
                let dir = root.join(format!("{}s", et));
                let entities = io::read_entity_dir(&dir, &et, def)
                    .await
                    .unwrap_or_default();
                entities.into_iter().map(|e| e.fields).collect()
            })
        }))
    }

    /// Apply compute-engine derivation to every entity in `entities`
    /// concurrently with bounded fan-out, preserving input order.
    ///
    /// Serial iteration through a 2000-entity list multiplies per-task
    /// disk I/O inside `apply_compute_with_query` (each task reads its
    /// own `.jsonl` changelog and stats its data file for
    /// `_file_created`). Fanning the compute pass out across many tokio
    /// tasks lets those reads overlap.
    ///
    /// Concurrency is bounded to keep memory footprint and FD pressure
    /// predictable — the exact fan-out is an internal detail tuned by
    /// benchmark.
    async fn apply_compute_batch(
        &self,
        entity_type: &str,
        entities: &mut Vec<Entity>,
    ) -> Result<()> {
        use futures::stream::{self, StreamExt};

        // Per-entity compute is a mix of cheap in-memory work (compute
        // engine derivation) and per-task disk I/O (`_changelog` JSONL
        // read + `_file_created` stat). 64 balances overlap of the I/O
        // path against scheduling overhead on the tokio runtime.
        // Benchmarked on `move_task_bench` — raising to 256 costs more
        // than it saves under contention.
        const COMPUTE_CONCURRENCY: usize = 64;

        let query_fn = self.build_entity_query_fn();
        // Hoist the field-def Vec out of the per-entity loop. The set of
        // computed fields for an entity type is fixed for the duration of
        // the batch, so cloning the definitions once and sharing via
        // `Arc` avoids N clones of a non-trivial `FieldDef` Vec.
        let owned_defs: std::sync::Arc<Vec<swissarmyhammer_fields::FieldDef>> = std::sync::Arc::new(
            self.fields
                .fields_for_entity(entity_type)
                .into_iter()
                .cloned()
                .collect(),
        );
        // Pre-compute the subset of attachment fields once per batch. The
        // per-entity `enrich_attachment_fields` call otherwise calls
        // `fields_for_entity` and walks every field def looking for the
        // `Attachment` variant — 2000× that traversal is measurable on
        // `list_task`. When no attachment fields are declared the batch
        // can skip enrichment entirely for entities that don't have any
        // attachment values set.
        let has_attachment_fields = owned_defs
            .iter()
            .any(|fd| matches!(&fd.type_, FieldType::Attachment { .. }));
        let entity_type_dir = if has_attachment_fields {
            Some(std::sync::Arc::new(self.entity_dir(entity_type)))
        } else {
            None
        };
        // Drain the caller's Vec into owned entities so each compute
        // task can mutate its own value concurrently without needing a
        // mutable borrow into a shared slice.
        let taken: Vec<Entity> = std::mem::take(entities);
        // Tag each entity with its input index so we can reassemble the
        // output Vec in the original order, independent of the
        // `buffer_unordered` completion order.
        let mut indexed: Vec<(usize, Result<Entity>)> = stream::iter(taken.into_iter().enumerate())
            .map(|(idx, mut entity)| {
                let query_fn = std::sync::Arc::clone(&query_fn);
                let owned_defs = std::sync::Arc::clone(&owned_defs);
                let entity_type_dir = entity_type_dir.clone();
                async move {
                    let res = async {
                        if let Some(dir) = entity_type_dir.as_deref() {
                            self.enrich_attachment_fields_with_defs(&mut entity, &owned_defs, dir)
                                .await?;
                        }
                        self.derive_compute_fields(entity_type, &mut entity, &query_fn, &owned_defs)
                            .await
                    }
                    .await;
                    (idx, res.map(|_| entity))
                }
            })
            .buffer_unordered(COMPUTE_CONCURRENCY)
            .collect()
            .await;

        // Restore input order by sorting on the captured indices.
        indexed.sort_by_key(|(idx, _)| *idx);
        entities.reserve(indexed.len());
        for (_, res) in indexed {
            entities.push(res?);
        }
        Ok(())
    }

    /// Derive computed fields after reading.
    ///
    /// Attachment enrichment is handled inside `apply_compute_with_query` so
    /// it only runs once, regardless of which entry point is used.
    async fn apply_compute(&self, entity_type: &str, entity: &mut Entity) -> Result<()> {
        if self.compute.is_none() {
            // No compute engine — just enrich attachment fields
            self.enrich_attachment_fields(entity_type, entity).await?;
            return Ok(());
        }
        let query_fn = self.build_entity_query_fn();
        self.apply_compute_with_query(entity_type, entity, &query_fn)
            .await
    }

    /// Derive computed fields using a pre-built query function.
    ///
    /// This avoids reconstructing the query closure per entity in batch
    /// operations like `list()`.
    ///
    /// When any computed field declares a dependency on a reserved pseudo-field
    /// (name starting with `_`), it is lazily sourced and injected into
    /// `entity.fields` before derivation, then stripped after derivation so it
    /// is never persisted or returned to callers.
    ///
    /// Supported injected dependencies:
    /// - `_changelog`: the entity's JSONL changelog as a JSON array.
    /// - `_file_created`: an RFC 3339 timestamp derived from the entity file's
    ///   `created()` metadata (falling back to `modified()` on platforms/filesystems
    ///   that don't support btime). Resolves to `Value::Null` when the file is
    ///   missing or cannot be stat'd — this is always a backstop signal, never
    ///   the primary one.
    async fn apply_compute_with_query(
        &self,
        entity_type: &str,
        entity: &mut Entity,
        query_fn: &std::sync::Arc<swissarmyhammer_fields::EntityQueryFn>,
    ) -> Result<()> {
        self.enrich_attachment_fields(entity_type, entity).await?;

        if self.compute.is_none() {
            return Ok(());
        }
        let owned_defs: Vec<_> = self
            .fields
            .fields_for_entity(entity_type)
            .into_iter()
            .cloned()
            .collect();

        self.derive_compute_fields(entity_type, entity, query_fn, &owned_defs)
            .await
    }

    /// Run the compute engine against `entity` using a pre-built
    /// `owned_defs` slice.
    ///
    /// Separated from [`apply_compute_with_query`] so the batch path
    /// ([`apply_compute_batch`]) can clone the type's field definitions
    /// exactly once, wrap them in an `Arc`, and share that `Arc` across
    /// every concurrent compute task. Without this split, each of the
    /// 2000 per-entity compute calls in `list("task")` would reclone the
    /// full `FieldDef` Vec, which is non-trivial for entity types with
    /// many fields.
    ///
    /// Callers must have already run `enrich_attachment_fields` on
    /// `entity` when that enrichment is relevant — this helper only
    /// handles compute-engine derivation and pseudo-field injection.
    ///
    /// When an [`EntityCache`] is attached, the outputs of every
    /// computed field — simple derivations and aggregates alike — are
    /// memoized per-entity in the cache's derived-output map. On a warm
    /// hit the cached values are copied straight into `entity.fields`
    /// without running the compute engine at all — and without even
    /// injecting the `_changelog` / `_file_created` pseudo-fields, since
    /// the derivations that consume them don't run on the warm path.
    ///
    /// Aggregate outputs (those produced by derivations that query other
    /// entity types via the `EntityQueryFn`) are kept fresh by
    /// cross-entity invalidation: the cache consults
    /// [`FieldsContext::entity_types_depending_on`] and, when any entity
    /// of a dependency type changes, bulk-invalidates the derived-output
    /// slots for every entity type whose aggregates declare that
    /// dependency. Aggregate fields that do not declare `depends_on` in
    /// their FieldDef are still cached — their outputs become stale only
    /// when the aggregate's hidden inputs change, which is a correctness
    /// bug in the field definition (fix by declaring `depends_on`).
    async fn derive_compute_fields(
        &self,
        entity_type: &str,
        entity: &mut Entity,
        query_fn: &std::sync::Arc<swissarmyhammer_fields::EntityQueryFn>,
        owned_defs: &[swissarmyhammer_fields::FieldDef],
    ) -> Result<()> {
        let Some(ref engine) = self.compute else {
            return Ok(());
        };

        // Try the derived-output cache. `cached_outputs` carries the
        // memoized per-entity outputs (a `Some(map)` is a warm hit; `None`
        // is a cold miss). The observed epoch is captured at read time so a
        // post-compute memoization attempt can be guarded against any
        // invalidation that lands mid-derivation.
        let (cached_outputs, observed_epoch) = if let Some(cache) = self.attached_cache() {
            cache
                .get_derived_outputs(entity_type, entity.id.as_str())
                .await
        } else {
            (None, 0)
        };
        let has_warm_cache = cached_outputs.is_some();

        // Inject pseudo-field inputs only on the cold path. On a warm hit
        // every computed field (simple AND aggregate) gets its value from
        // the cached output, so `_changelog` / `_file_created` are never
        // read. Cross-entity invalidation
        // ([`EntityCache::invalidate_cross_type_derived`]) keeps aggregate
        // cached outputs fresh when sibling entity types change, so
        // skipping injection here is safe.
        if !has_warm_cache {
            self.inject_compute_dependencies(entity_type, entity, owned_defs)
                .await;
        }

        // Collect freshly-computed outputs on the cold path so we can
        // memoize them after the derivation finishes.
        let mut fresh_outputs: Option<HashMap<String, serde_json::Value>> = if has_warm_cache {
            None
        } else {
            Some(HashMap::new())
        };

        // Iterate fields in declaration order so aggregate derivations that
        // read simple-derivation outputs see them already populated — the
        // same ordering contract `ComputeEngine::derive_all` documents.
        for field in owned_defs {
            let FieldType::Computed { .. } = field.type_ else {
                continue;
            };

            // Warm path: use the cached output when available. A field
            // missing from the cached map falls through to compute — this
            // happens when a computed field is added after the cache was
            // populated; the next invalidation closes the gap.
            if let Some(value) = cached_outputs
                .as_ref()
                .and_then(|c| c.get(field.name.as_str()))
            {
                entity.fields.insert(field.name.to_string(), value.clone());
                continue;
            }

            let value = engine
                .derive(field, &entity.fields, Some(query_fn))
                .await
                .map_err(map_compute_error)?;

            if let Some(ref mut fresh) = fresh_outputs {
                fresh.insert(field.name.to_string(), value.clone());
            }

            entity.fields.insert(field.name.to_string(), value);
        }

        // Strip injected pseudo-fields so they are never persisted or
        // surfaced to callers. Only the cold path inserts them, but the
        // `remove` is cheap so it is unconditional.
        entity.fields.remove("_changelog");
        entity.fields.remove("_file_created");

        // Memoize the freshly-computed outputs so the next
        // derive_compute_fields call for this entity can skip the engine.
        // Guarded by `observed_epoch` so any invalidation that landed
        // between the cache read and here causes the memoization to be
        // dropped.
        self.try_memoize_fresh_outputs(entity_type, entity, observed_epoch, fresh_outputs)
            .await;

        Ok(())
    }

    /// Store freshly-computed field outputs in the entity cache so the next
    /// derive pass can skip the compute engine.
    ///
    /// No-ops when there is no cache, nothing was freshly computed, or the
    /// cache epoch has advanced since the derivation began (meaning an
    /// invalidation landed mid-derivation and the outputs are stale).
    async fn try_memoize_fresh_outputs(
        &self,
        entity_type: &str,
        entity: &Entity,
        observed_epoch: u64,
        fresh_outputs: Option<HashMap<String, serde_json::Value>>,
    ) {
        let Some(fresh) = fresh_outputs else {
            return;
        };
        if fresh.is_empty() {
            return;
        }
        let Some(cache) = self.attached_cache() else {
            return;
        };
        cache
            .try_memoize_derived_outputs(entity_type, entity.id.as_str(), observed_epoch, fresh)
            .await;
    }

    /// Lazily source reserved pseudo-fields and insert them into `entity.fields`
    /// when at least one computed field in `owned_defs` declares a
    /// dependency on them. Values are stripped by the caller after
    /// derivation so they are never persisted or returned to callers.
    ///
    /// When an [`EntityCache`] is attached, both `_changelog` and
    /// `_file_created` go through the cache's memoization layer
    /// (`get_or_load_changelog` / `get_or_load_file_created`) so repeated
    /// list/read calls on a steady-state board do not re-read every task's
    /// JSONL changelog and re-stat every entity file. The cache invalidates
    /// those memoized values on any mutation path that might move them, so
    /// the injected data always reflects the latest on-disk state without
    /// paying the per-entity I/O cost on every pass.
    async fn inject_compute_dependencies(
        &self,
        entity_type: &str,
        entity: &mut Entity,
        owned_defs: &[swissarmyhammer_fields::FieldDef],
    ) {
        let want_changelog = any_field_depends_on(owned_defs, "_changelog");
        let want_file_created = any_field_depends_on(owned_defs, "_file_created");
        if !want_changelog && !want_file_created {
            return;
        }

        if let Some(cache) = self.attached_cache() {
            // Batched loader — at most one read lock and at most one write
            // lock per entity, regardless of how many pseudo-fields are
            // requested. Matters under the 64-way `buffer_unordered` fan-out
            // used by `apply_compute_batch` where per-entity lock
            // contention would otherwise dominate.
            let (changelog, file_created) = cache
                .get_or_load_compute_inputs(
                    entity_type,
                    entity.id.as_str(),
                    want_changelog,
                    want_file_created,
                )
                .await;
            if want_changelog {
                entity.fields.insert("_changelog".to_string(), changelog);
            }
            if want_file_created {
                entity
                    .fields
                    .insert("_file_created".to_string(), file_created);
            }
            return;
        }

        // No cache — read from disk on every call. Same serialization
        // semantics as the cache slow path so cached and uncached lookups
        // produce identical entity.fields.
        if want_changelog {
            let entries = self
                .read_changelog(entity_type, entity.id.as_str())
                .await
                .unwrap_or_default();
            let json_entries: Vec<serde_json::Value> = entries
                .iter()
                .filter_map(|e| serde_json::to_value(e).ok())
                .collect();
            entity.fields.insert(
                "_changelog".to_string(),
                serde_json::Value::Array(json_entries),
            );
        }

        if want_file_created {
            entity.fields.insert(
                "_file_created".to_string(),
                self.compute_file_created_timestamp(entity_type, entity.id.as_str())
                    .await,
            );
        }
    }

    /// Stat the entity's source file and return its creation timestamp as an
    /// RFC 3339 JSON string, falling back to the modification time when the
    /// platform/filesystem doesn't expose btime. Returns `Value::Null` on any
    /// I/O error — this is a backstop signal, so a missing file should not
    /// fail the derivation.
    ///
    /// This is the raw I/O path used both by the direct compute dependency
    /// injection (when no cache is attached) and by the cache's lazy loader
    /// (`EntityCache::get_or_load_file_created`). It is `pub(crate)` so the
    /// cache module can call it without going through the public entity API.
    pub(crate) async fn compute_file_created_timestamp(
        &self,
        entity_type: &str,
        id: &str,
    ) -> serde_json::Value {
        let Ok(def) = self.entity_def(entity_type) else {
            return serde_json::Value::Null;
        };
        let path = io::entity_file_path(&self.entity_dir(entity_type), id, def);
        let Ok(meta) = tokio::fs::metadata(&path).await else {
            return serde_json::Value::Null;
        };
        let Ok(system_time) = meta.created().or_else(|_| meta.modified()) else {
            return serde_json::Value::Null;
        };
        let dt: chrono::DateTime<chrono::Utc> = system_time.into();
        serde_json::Value::String(dt.to_rfc3339())
    }

    /// Enrich attachment fields with metadata objects on read.
    ///
    /// Replaces stored filenames with rich JSON objects containing
    /// id, name, size, mime_type, and absolute path.
    async fn enrich_attachment_fields(&self, entity_type: &str, entity: &mut Entity) -> Result<()> {
        let field_defs: Vec<_> = self
            .fields
            .fields_for_entity(entity_type)
            .into_iter()
            .cloned()
            .collect();
        let entity_type_dir = self.entity_dir(entity_type);
        self.enrich_attachment_fields_with_defs(entity, &field_defs, &entity_type_dir)
            .await
    }

    /// Run attachment enrichment over `entity` using a caller-provided
    /// field-def slice.
    ///
    /// Separated from [`enrich_attachment_fields`] so the batch path in
    /// [`apply_compute_batch`] can reuse the `FieldDef` Vec it already
    /// cloned once across every entity — otherwise enrichment would
    /// re-traverse the `FieldsContext` HashMap for each of the 2000
    /// entities in a large-board `list()`.
    ///
    /// The function is a no-op for entities whose type has no attachment
    /// fields — callers in the batch path should check
    /// `owned_defs.iter().any(FieldType::Attachment)` up-front to avoid
    /// even scheduling this call on the hot path.
    async fn enrich_attachment_fields_with_defs(
        &self,
        entity: &mut Entity,
        field_defs: &[swissarmyhammer_fields::FieldDef],
        entity_type_dir: &Path,
    ) -> Result<()> {
        use serde_json::Value;

        for fd in field_defs {
            let FieldType::Attachment { multiple, .. } = &fd.type_ else {
                continue;
            };
            let Some(value) = entity.fields.get(fd.name.as_str()).cloned() else {
                continue;
            };

            if !*multiple {
                let Value::String(filename) = value else {
                    continue;
                };
                if let Some(meta) = io::attachment_metadata(&filename, entity_type_dir).await {
                    entity.set(fd.name.to_string(), meta);
                }
                continue;
            }

            // Multiple attachments — normalize to array then enrich each.
            let filenames = match value {
                Value::Array(arr) => arr,
                Value::Null => continue,
                other => vec![other],
            };
            let mut metadata_arr = Vec::new();
            for v in filenames {
                let Value::String(filename) = v else {
                    continue;
                };
                if let Some(meta) = io::attachment_metadata(&filename, entity_type_dir).await {
                    metadata_arr.push(meta);
                }
            }
            entity.set(fd.name.to_string(), Value::Array(metadata_arr));
        }

        Ok(())
    }

    /// Trash attachment files that were removed between old and new entity state.
    ///
    /// Compares attachment field values between old and new versions. Any filenames
    /// present in the old entity but absent from the new one are moved to
    /// `.attachments/.trash/`.
    async fn trash_removed_attachments(
        &self,
        entity_type: &str,
        old: &Entity,
        new: &Entity,
    ) -> Result<()> {
        let field_defs = self.fields.fields_for_entity(entity_type);
        let entity_type_dir = self.entity_dir(entity_type);

        for fd in &field_defs {
            if let FieldType::Attachment { multiple, .. } = &fd.type_ {
                let old_names =
                    Self::extract_attachment_filenames(old.fields.get(fd.name.as_str()), *multiple);
                let new_names =
                    Self::extract_attachment_filenames(new.fields.get(fd.name.as_str()), *multiple);

                for name in &old_names {
                    if !new_names.contains(name) {
                        io::trash_attachment(name, &entity_type_dir).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Trash all attachment files for an entity being deleted.
    ///
    /// Reads attachment field values and moves each referenced file to
    /// `.attachments/.trash/`.
    async fn trash_entity_attachments(&self, entity_type: &str, entity: &Entity) -> Result<()> {
        let field_defs = self.fields.fields_for_entity(entity_type);
        let entity_type_dir = self.entity_dir(entity_type);

        for fd in &field_defs {
            if let FieldType::Attachment { multiple, .. } = &fd.type_ {
                let filenames = Self::extract_attachment_filenames(
                    entity.fields.get(fd.name.as_str()),
                    *multiple,
                );
                for name in filenames {
                    io::trash_attachment(&name, &entity_type_dir).await?;
                }
            }
        }

        Ok(())
    }

    /// Extract attachment filenames from a field value.
    ///
    /// Returns a list of stored filenames (strings) from either a single
    /// value or an array, depending on the `multiple` flag.
    fn extract_attachment_filenames(
        value: Option<&serde_json::Value>,
        multiple: bool,
    ) -> Vec<String> {
        use serde_json::Value;

        let Some(value) = value else {
            return Vec::new();
        };

        if multiple {
            match value {
                Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                Value::String(s) => vec![s.clone()],
                _ => Vec::new(),
            }
        } else {
            match value {
                Value::String(s) => vec![s.clone()],
                _ => Vec::new(),
            }
        }
    }
}

/// Return `true` when any computed field in `defs` declares `dep_name` in its
/// `depends_on` list.
fn any_field_depends_on(defs: &[swissarmyhammer_fields::FieldDef], dep_name: &str) -> bool {
    defs.iter().any(|fd| {
        if let FieldType::Computed { depends_on, .. } = &fd.type_ {
            depends_on.iter().any(|dep| dep == dep_name)
        } else {
            false
        }
    })
}

/// Convert a `FieldsError` from the compute engine into the crate-local
/// `EntityError::ComputeError`, preserving the offending field name and
/// underlying message when available. Consumes `err` by value so the owned
/// strings inside `ComputeError` move through to the returned `EntityError`
/// without being cloned.
fn map_compute_error(err: swissarmyhammer_fields::FieldsError) -> EntityError {
    let (field, message) = match err {
        swissarmyhammer_fields::FieldsError::ComputeError { field, message } => (field, message),
        other => (String::new(), other.to_string()),
    };
    EntityError::ComputeError { field, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_fields_context;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn entity_dir_pluralizes() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert_eq!(ctx.entity_dir("task"), dir.path().join("tasks"));
        assert_eq!(ctx.entity_dir("tag"), dir.path().join("tags"));
        assert_eq!(ctx.entity_dir("board"), dir.path().join("boards"));
    }

    #[tokio::test]
    async fn entity_path_uses_correct_extension() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // task has body_field → .md
        let p = ctx.entity_path("task", "01ABC").unwrap();
        assert_eq!(p, dir.path().join("tasks").join("01ABC.md"));

        // tag has no body_field → .yaml
        let p = ctx.entity_path("tag", "bug").unwrap();
        assert_eq!(p, dir.path().join("tags").join("bug.yaml"));
    }

    #[tokio::test]
    async fn unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert!(ctx.entity_path("unicorn", "x").is_err());
        assert!(ctx.read("unicorn", "x").await.is_err());
    }

    #[tokio::test]
    async fn round_trip_plain_yaml() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));

        ctx.write(&tag).await.unwrap();

        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));
        assert_eq!(loaded.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn round_trip_with_body() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut task = Entity::new("task", "01ABC");
        task.set("title", json!("Fix bug"));
        task.set("body", json!("Details here.\n\n- [ ] Step 1"));

        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01ABC").await.unwrap();
        assert_eq!(loaded.get_str("title"), Some("Fix bug"));
        assert!(loaded.get_str("body").unwrap().contains("Step 1"));
    }

    #[tokio::test]
    async fn list_entities() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut t1 = Entity::new("tag", "bug");
        t1.set("tag_name", json!("Bug"));
        let mut t2 = Entity::new("tag", "feature");
        t2.set("tag_name", json!("Feature"));

        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();

        let tags = ctx.list("tag").await.unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[tokio::test]
    async fn delete_moves_to_trash() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        assert!(ctx.read("tag", "bug").await.is_ok());
        ctx.delete("tag", "bug").await.unwrap();

        // No longer readable from live storage
        assert!(ctx.read("tag", "bug").await.is_err());

        // Files moved to trash (new layout: {type}s/.trash/)
        let trash_dir = dir.path().join("tags").join(".trash");
        assert!(trash_dir.join("bug.yaml").exists());
    }

    #[tokio::test]
    async fn trash_dir_correct() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // New layout: {root}/{type}s/.trash/
        assert_eq!(ctx.trash_dir("tag"), dir.path().join("tags").join(".trash"));
        assert_eq!(
            ctx.trash_dir("task"),
            dir.path().join("tasks").join(".trash")
        );
    }

    #[tokio::test]
    async fn changelog_path_correct() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let p = ctx.changelog_path("tag", "bug").unwrap();
        assert_eq!(p, dir.path().join("tags").join("bug.jsonl"));
    }

    #[tokio::test]
    async fn migration_moves_old_trash() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Simulate old-style trash layout: {root}/.trash/{type}s/
        let old_trash = dir.path().join(".trash").join("tags");
        tokio::fs::create_dir_all(&old_trash).await.unwrap();
        tokio::fs::write(old_trash.join("bug.yaml"), "tag_name: Bug\n")
            .await
            .unwrap();
        tokio::fs::write(old_trash.join("bug.jsonl"), "{}\n")
            .await
            .unwrap();

        // Run migration
        ctx.migrate_trash_layout("tag").await.unwrap();

        // Files should now be in the new location: {type}s/.trash/
        let new_trash = dir.path().join("tags").join(".trash");
        assert!(new_trash.join("bug.yaml").exists());
        assert!(new_trash.join("bug.jsonl").exists());

        // Old location should be gone
        assert!(!old_trash.exists());
        // Old root .trash/ should also be gone
        assert!(!dir.path().join(".trash").exists());
    }

    // --- Attachment tests ---

    /// Build a FieldsContext with an entity type that has attachment fields.
    fn attachment_fields_context() -> Arc<FieldsContext> {
        let defs = vec![
            (
                "title",
                "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "avatar",
                "id: 00000000000000000000000AVT\nname: avatar\ntype:\n  kind: attachment\n  max_bytes: 1048576\n  multiple: false\n",
            ),
            (
                "files",
                "id: 00000000000000000000000FLS\nname: files\ntype:\n  kind: attachment\n  max_bytes: 1048576\n  multiple: true\n",
            ),
        ];
        let entities = vec![(
            "item",
            "name: item\nfields:\n  - title\n  - avatar\n  - files\n",
        )];

        let dir = TempDir::new().unwrap();
        Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
    }

    #[tokio::test]
    async fn write_attachment_copies_file_and_stores_filename() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Create a source file to attach
        let source = dir.path().join("photo.jpg");
        tokio::fs::write(&source, b"fake image data").await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test Item"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));

        ctx.write(&entity).await.unwrap();

        // Read raw (without compute) to check stored filename
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
        let raw = crate::io::read_entity(&path, "item", "01TEST", def)
            .await
            .unwrap();

        let stored = raw.fields.get("avatar").unwrap().as_str().unwrap();
        assert!(
            stored.contains("photo.jpg"),
            "stored name should contain original filename"
        );
        assert!(
            stored.len() > "photo.jpg".len(),
            "stored name should have ULID prefix"
        );

        // Verify the file was copied to .attachments/
        let att_dir = dir.path().join("items").join(".attachments");
        assert!(att_dir.join(stored).exists());

        // Verify contents match
        let copied = tokio::fs::read(att_dir.join(stored)).await.unwrap();
        assert_eq!(copied, b"fake image data");
    }

    #[tokio::test]
    async fn write_existing_attachment_filename_leaves_file_untouched() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Create a source file and write it as an attachment
        let source = dir.path().join("photo.jpg");
        tokio::fs::write(&source, b"original data").await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Get the stored filename
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
        let raw = crate::io::read_entity(&path, "item", "01TEST", def)
            .await
            .unwrap();
        let stored = raw
            .fields
            .get("avatar")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Write again with the stored filename (not a source path)
        let mut entity2 = Entity::new("item", "01TEST");
        entity2.set("title", json!("Updated Title"));
        entity2.set("avatar", json!(stored.clone()));
        ctx.write(&entity2).await.unwrap();

        // Verify the file still exists and contents unchanged
        let att_dir = dir.path().join("items").join(".attachments");
        let contents = tokio::fs::read(att_dir.join(&stored)).await.unwrap();
        assert_eq!(contents, b"original data");
    }

    #[tokio::test]
    async fn read_attachment_returns_metadata_with_path() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        let source = dir.path().join("photo.png");
        tokio::fs::write(&source, b"png data here").await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Read with compute (should enrich attachment fields)
        let read = ctx.read("item", "01TEST").await.unwrap();
        let meta = read.fields.get("avatar").unwrap();

        assert!(
            meta.is_object(),
            "attachment field should be a metadata object"
        );
        assert_eq!(meta["name"], "photo.png");
        assert_eq!(meta["mime_type"], "image/png");
        assert_eq!(meta["size"], 13); // b"png data here".len()
        assert!(meta["id"].is_string());
        assert!(meta["path"].as_str().unwrap().contains(".attachments"));

        // Verify the path is readable and content matches
        let resolved_path = meta["path"].as_str().unwrap();
        let contents = tokio::fs::read(resolved_path).await.unwrap();
        assert_eq!(contents, b"png data here");
    }

    #[tokio::test]
    async fn write_attachment_exceeding_max_bytes_errors() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // max_bytes is 1MB; create a file slightly over
        let source = dir.path().join("huge.bin");
        let data = vec![0u8; 1_048_577]; // 1MB + 1
        tokio::fs::write(&source, &data).await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));

        let result = ctx.write(&entity).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("too large"),
            "error should mention file too large: {}",
            err
        );
    }

    #[tokio::test]
    async fn update_removing_attachment_trashes_file() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        let source = dir.path().join("photo.jpg");
        tokio::fs::write(&source, b"image data").await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Get the stored filename
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
        let raw = crate::io::read_entity(&path, "item", "01TEST", def)
            .await
            .unwrap();
        let stored = raw
            .fields
            .get("avatar")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Update entity removing the avatar field
        let mut entity2 = Entity::new("item", "01TEST");
        entity2.set("title", json!("No Avatar"));
        // avatar field is absent → attachment should be trashed
        ctx.write(&entity2).await.unwrap();

        // Verify file moved to .trash
        let att_dir = dir.path().join("items").join(".attachments");
        assert!(
            !att_dir.join(&stored).exists(),
            "attachment should be removed from .attachments/"
        );
        let trash_dir = att_dir.join(".trash");
        assert!(
            trash_dir.join(&stored).exists(),
            "attachment should be in .attachments/.trash/"
        );
    }

    #[tokio::test]
    async fn delete_entity_trashes_attachment_files() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        let source = dir.path().join("doc.pdf");
        tokio::fs::write(&source, b"pdf content").await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Get stored filename
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
        let raw = crate::io::read_entity(&path, "item", "01TEST", def)
            .await
            .unwrap();
        let stored = raw
            .fields
            .get("avatar")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Delete the entity
        ctx.delete("item", "01TEST").await.unwrap();

        // Attachment file should be in .attachments/.trash/
        let att_dir = dir.path().join("items").join(".attachments");
        assert!(!att_dir.join(&stored).exists());
        assert!(att_dir.join(".trash").join(&stored).exists());
    }

    #[tokio::test]
    async fn multiple_attachments_add_read_remove() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Create two source files
        let src1 = dir.path().join("file1.txt");
        let src2 = dir.path().join("file2.txt");
        tokio::fs::write(&src1, b"content one").await.unwrap();
        tokio::fs::write(&src2, b"content two").await.unwrap();

        // Write entity with two attachments in the `files` (multiple) field
        let mut entity = Entity::new("item", "01MULTI");
        entity.set("title", json!("Multi"));
        entity.set(
            "files",
            json!([
                src1.to_string_lossy().to_string(),
                src2.to_string_lossy().to_string()
            ]),
        );
        ctx.write(&entity).await.unwrap();

        // Read raw to get stored filenames
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01MULTI", def);
        let raw = crate::io::read_entity(&path, "item", "01MULTI", def)
            .await
            .unwrap();
        let stored_arr = raw.fields.get("files").unwrap().as_array().unwrap();
        assert_eq!(stored_arr.len(), 2);
        let stored1 = stored_arr[0].as_str().unwrap().to_string();
        let stored2 = stored_arr[1].as_str().unwrap().to_string();

        // Read with compute — should get metadata array
        let read = ctx.read("item", "01MULTI").await.unwrap();
        let meta_arr = read.fields.get("files").unwrap().as_array().unwrap();
        assert_eq!(meta_arr.len(), 2);
        assert_eq!(meta_arr[0]["name"], "file1.txt");
        assert_eq!(meta_arr[1]["name"], "file2.txt");

        // Update removing one attachment (keep stored2, drop stored1)
        let mut entity2 = Entity::new("item", "01MULTI");
        entity2.set("title", json!("Multi"));
        entity2.set("files", json!([stored2.clone()]));
        ctx.write(&entity2).await.unwrap();

        // stored1 should be trashed, stored2 should remain
        let att_dir = dir.path().join("items").join(".attachments");
        assert!(!att_dir.join(&stored1).exists());
        assert!(att_dir.join(".trash").join(&stored1).exists());
        assert!(att_dir.join(&stored2).exists());
    }

    #[tokio::test]
    async fn write_attachment_source_not_found_errors() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!("/nonexistent/path/photo.jpg"));

        let result = ctx.write(&entity).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "error should mention source not found: {}",
            err
        );
    }

    #[tokio::test]
    async fn write_enriched_attachment_object_preserves_file() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Create and write an entity with an attachment
        let source = dir.path().join("photo.png");
        tokio::fs::write(&source, b"png data").await.unwrap();

        let mut entity = Entity::new("item", "01ENRICH");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Read — avatar is now an enriched metadata object
        let read = ctx.read("item", "01ENRICH").await.unwrap();
        let meta = read.fields.get("avatar").unwrap().clone();
        assert!(meta.is_object(), "should be enriched");

        // Write back unchanged — the enriched object should round-trip
        let mut entity2 = Entity::new("item", "01ENRICH");
        entity2.set("title", json!("Updated Title"));
        entity2.set("avatar", meta);
        ctx.write(&entity2).await.unwrap();

        // Verify the attachment file still exists and data is intact
        let att_dir = dir.path().join("items").join(".attachments");
        let entries: Vec<_> = std::fs::read_dir(&att_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_str().unwrap_or("").starts_with('.'))
            .collect();
        assert_eq!(entries.len(), 1, "attachment file should still exist");
        let contents = tokio::fs::read(entries[0].path()).await.unwrap();
        assert_eq!(contents, b"png data");
    }

    #[tokio::test]
    async fn write_enriched_objects_mixed_with_new_paths() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Create and attach first file
        let src1 = dir.path().join("file1.txt");
        tokio::fs::write(&src1, b"content one").await.unwrap();

        let mut entity = Entity::new("item", "01MIX");
        entity.set("title", json!("Mixed"));
        entity.set("files", json!([src1.to_string_lossy().to_string()]));
        ctx.write(&entity).await.unwrap();

        // Read to get enriched metadata
        let read = ctx.read("item", "01MIX").await.unwrap();
        let enriched_arr = read
            .fields
            .get("files")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(enriched_arr.len(), 1);

        // Create a second source file to append
        let src2 = dir.path().join("file2.txt");
        tokio::fs::write(&src2, b"content two").await.unwrap();

        // Write back with mixed array: enriched object + new source path
        let mut entity2 = Entity::new("item", "01MIX");
        entity2.set("title", json!("Mixed"));
        entity2.set(
            "files",
            json!([enriched_arr[0], src2.to_string_lossy().to_string()]),
        );
        ctx.write(&entity2).await.unwrap();

        // Read again — should have two attachments
        let read2 = ctx.read("item", "01MIX").await.unwrap();
        let files = read2.fields.get("files").unwrap().as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["name"], "file1.txt");
        assert_eq!(files[1]["name"], "file2.txt");
    }

    #[tokio::test]
    async fn write_mixed_enriched_stored_and_source_paths() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Create two source files and write them
        let src1 = dir.path().join("a.txt");
        let src2 = dir.path().join("b.txt");
        tokio::fs::write(&src1, b"aaa").await.unwrap();
        tokio::fs::write(&src2, b"bbb").await.unwrap();

        let mut entity = Entity::new("item", "01ALL3");
        entity.set("title", json!("Three Shapes"));
        entity.set(
            "files",
            json!([
                src1.to_string_lossy().to_string(),
                src2.to_string_lossy().to_string()
            ]),
        );
        ctx.write(&entity).await.unwrap();

        // Read to get enriched metadata and raw stored filenames
        let read = ctx.read("item", "01ALL3").await.unwrap();
        let enriched = read.fields.get("files").unwrap().as_array().unwrap();
        let enriched_obj = enriched[0].clone(); // enriched metadata object

        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01ALL3", def);
        let raw = crate::io::read_entity(&path, "item", "01ALL3", def)
            .await
            .unwrap();
        let stored_filename = raw.fields.get("files").unwrap().as_array().unwrap()[1]
            .as_str()
            .unwrap()
            .to_string(); // raw stored filename string

        // Create a third file to add as source path
        let src3 = dir.path().join("c.txt");
        tokio::fs::write(&src3, b"ccc").await.unwrap();

        // Write with all three shapes: enriched object, stored filename, source path
        let mut entity2 = Entity::new("item", "01ALL3");
        entity2.set("title", json!("Three Shapes"));
        entity2.set(
            "files",
            json!([
                enriched_obj,
                stored_filename,
                src3.to_string_lossy().to_string()
            ]),
        );
        ctx.write(&entity2).await.unwrap();

        // Read — should have three attachments
        let read2 = ctx.read("item", "01ALL3").await.unwrap();
        let files = read2.fields.get("files").unwrap().as_array().unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0]["name"], "a.txt");
        assert_eq!(files[1]["name"], "b.txt");
        assert_eq!(files[2]["name"], "c.txt");
    }

    // -----------------------------------------------------------------------
    // enrich_attachment_fields — targeted tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn enrich_attachment_fields_populates_full_metadata() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Write two source files as a multiple attachment
        let src1 = dir.path().join("image.png");
        let src2 = dir.path().join("readme.md");
        tokio::fs::write(&src1, b"PNG data bytes").await.unwrap();
        tokio::fs::write(&src2, b"# Hello").await.unwrap();

        let mut entity = Entity::new("item", "01ENRICH");
        entity.set("title", json!("Enrich Test"));
        entity.set(
            "files",
            json!([
                src1.to_string_lossy().to_string(),
                src2.to_string_lossy().to_string()
            ]),
        );
        ctx.write(&entity).await.unwrap();

        // Read — enrichment happens via apply_compute → enrich_attachment_fields
        let read = ctx.read("item", "01ENRICH").await.unwrap();
        let files = read.fields.get("files").unwrap().as_array().unwrap();
        assert_eq!(files.len(), 2);

        // Verify each enriched object has the required shape
        for meta in files {
            assert!(meta["id"].is_string(), "should have id");
            assert!(meta["name"].is_string(), "should have name");
            assert!(meta["size"].is_number(), "should have size");
            assert!(meta["mime_type"].is_string(), "should have mime_type");
            assert!(meta["path"].is_string(), "should have path");
        }

        assert_eq!(files[0]["name"], "image.png");
        assert_eq!(files[0]["size"], 14); // b"PNG data bytes".len()
        assert_eq!(files[0]["mime_type"], "image/png");

        assert_eq!(files[1]["name"], "readme.md");
        assert_eq!(files[1]["size"], 7); // b"# Hello".len()
    }

    #[tokio::test]
    async fn enrich_attachment_fields_missing_file_silently_drops() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // Write one real attachment first
        let src = dir.path().join("real.txt");
        tokio::fs::write(&src, b"exists").await.unwrap();

        let mut entity = Entity::new("item", "01MISS");
        entity.set("title", json!("Missing File Test"));
        entity.set("files", json!([src.to_string_lossy().to_string()]));
        ctx.write(&entity).await.unwrap();

        // Get the stored filename, then manually add a bogus one to the YAML
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01MISS", def);
        let raw = crate::io::read_entity(&path, "item", "01MISS", def)
            .await
            .unwrap();
        let stored = raw.fields.get("files").unwrap().as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .to_string();

        // Rewrite entity with the real stored filename plus a nonexistent one
        let mut entity2 = Entity::new("item", "01MISS");
        entity2.set("title", json!("Missing File Test"));
        entity2.set("files", json!([stored.clone(), "01BOGUS-nonexistent.txt"]));
        // Write raw to disk (bypass resolve so the bogus name stays as-is)
        crate::io::write_entity(&path, &entity2, def).await.unwrap();

        // Read with enrichment — missing file should be silently dropped
        let read = ctx.read("item", "01MISS").await.unwrap();
        let files = read.fields.get("files").unwrap().as_array().unwrap();

        // Only the real file should appear (missing one silently skipped)
        assert_eq!(
            files.len(),
            1,
            "missing attachment should be silently dropped during enrichment"
        );
        assert_eq!(files[0]["name"], "real.txt");
    }

    #[tokio::test]
    async fn enrich_attachment_fields_empty_array_unchanged() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        let mut entity = Entity::new("item", "01EMPTY");
        entity.set("title", json!("Empty Attachments"));
        entity.set("files", json!([]));
        ctx.write(&entity).await.unwrap();

        let read = ctx.read("item", "01EMPTY").await.unwrap();
        let files = read.fields.get("files").unwrap().as_array().unwrap();
        assert!(
            files.is_empty(),
            "empty attachment array should remain empty after enrichment"
        );
    }

    #[tokio::test]
    async fn resolve_attachment_value_copies_source_to_attachments_dir() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        let source = dir.path().join("document.pdf");
        tokio::fs::write(&source, b"PDF content here")
            .await
            .unwrap();

        let mut entity = Entity::new("item", "01RESOLVE");
        entity.set("title", json!("Resolve Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Verify file landed in .attachments/
        let att_dir = dir.path().join("items").join(".attachments");
        let mut entries = tokio::fs::read_dir(&att_dir).await.unwrap();
        let mut found = false;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with("-document.pdf") {
                found = true;
                let contents = tokio::fs::read(entry.path()).await.unwrap();
                assert_eq!(contents, b"PDF content here");
            }
        }
        assert!(
            found,
            "source file should be copied to .attachments/ with ULID prefix"
        );
    }

    #[tokio::test]
    async fn resolve_attachment_value_existing_filename_returned_as_is() {
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);

        // First write: copy a source file
        let source = dir.path().join("notes.txt");
        tokio::fs::write(&source, b"my notes").await.unwrap();

        let mut entity = Entity::new("item", "01EXIST");
        entity.set("title", json!("Existing Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        // Get the stored filename from raw YAML
        let def = ctx.entity_def("item").unwrap();
        let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01EXIST", def);
        let raw = crate::io::read_entity(&path, "item", "01EXIST", def)
            .await
            .unwrap();
        let stored = raw
            .fields
            .get("avatar")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Second write: use the stored filename directly
        let mut entity2 = Entity::new("item", "01EXIST");
        entity2.set("title", json!("Existing Test v2"));
        entity2.set("avatar", json!(stored.clone()));
        ctx.write(&entity2).await.unwrap();

        // Verify the stored filename didn't change (no new copy)
        let raw2 = crate::io::read_entity(&path, "item", "01EXIST", def)
            .await
            .unwrap();
        let stored2 = raw2
            .fields
            .get("avatar")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            stored, stored2,
            "stored filename should be returned as-is without creating a new copy"
        );

        // Only one file should exist in .attachments/ (no duplicates)
        let att_dir = dir.path().join("items").join(".attachments");
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&att_dir).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                count += 1;
            }
        }
        assert_eq!(count, 1, "should have exactly one attachment file");
    }

    // ===========================================================================
    // Additional tests from main
    // ===========================================================================

    #[tokio::test]
    async fn archive_dir_correct() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert_eq!(
            ctx.archive_dir("tag"),
            dir.path().join("tags").join(".archive")
        );
        assert_eq!(
            ctx.archive_dir("task"),
            dir.path().join("tasks").join(".archive")
        );
    }

    #[tokio::test]
    async fn list_archived_returns_archived_only() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create two tags
        let mut t1 = Entity::new("tag", "bug");
        t1.set("tag_name", json!("Bug"));
        let mut t2 = Entity::new("tag", "feature");
        t2.set("tag_name", json!("Feature"));

        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();

        // Archive only "bug"
        ctx.archive("tag", "bug").await.unwrap();

        // list() should only return "feature"
        let live = ctx.list("tag").await.unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, "feature");

        // list_archived() should only return "bug"
        let archived = ctx.list_archived("tag").await.unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, "bug");
    }

    #[tokio::test]
    async fn read_archived_returns_entity() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        ctx.archive("tag", "bug").await.unwrap();

        // read() on archived entity should fail
        assert!(ctx.read("tag", "bug").await.is_err());

        // read_archived() should succeed
        let archived = ctx.read_archived("tag", "bug").await.unwrap();
        assert_eq!(archived.get_str("tag_name"), Some("Bug"));
        assert_eq!(archived.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn archive_writes_changelog() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        ctx.archive("tag", "bug").await.unwrap();

        // Changelog lives in the archive directory
        let archive_log = dir.path().join("tags").join(".archive").join("bug.jsonl");
        let content = tokio::fs::read_to_string(&archive_log).await.unwrap();
        assert!(
            content.contains("\"archive\""),
            "changelog should contain archive op"
        );
    }

    #[tokio::test]
    async fn unarchive_writes_changelog() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        ctx.archive("tag", "bug").await.unwrap();
        ctx.unarchive("tag", "bug").await.unwrap();

        // After unarchive, changelog is back in live dir
        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        assert!(
            log.iter().any(|e| e.op == "unarchive"),
            "changelog should contain unarchive op"
        );
    }

    #[tokio::test]
    async fn root_and_fields_accessors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert_eq!(ctx.root(), dir.path());
        // fields() should return the same FieldsContext
        assert!(ctx.fields().get_entity("tag").is_some());
        assert!(ctx.fields().get_entity("task").is_some());
    }

    #[tokio::test]
    async fn list_archived_with_compute_engine() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());
        let ctx = EntityContext::new(dir.path(), fields.clone()).with_compute(compute);

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();
        ctx.archive("tag", "bug").await.unwrap();

        // list_archived with compute engine
        let archived = ctx.list_archived("tag").await.unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    async fn extract_attachment_filenames_edge_cases() {
        // None value returns empty
        let empty: Vec<String> = EntityContext::extract_attachment_filenames(None, false);
        assert!(empty.is_empty());

        let empty_multi: Vec<String> = EntityContext::extract_attachment_filenames(None, true);
        assert!(empty_multi.is_empty());

        // Non-string single value returns empty
        let num = json!(42);
        let result = EntityContext::extract_attachment_filenames(Some(&num), false);
        assert!(result.is_empty());

        // Non-string/non-array multiple value returns empty
        let result_multi = EntityContext::extract_attachment_filenames(Some(&num), true);
        assert!(result_multi.is_empty());

        // String value for multiple returns single-element vec
        let s = json!("filename.txt");
        let result = EntityContext::extract_attachment_filenames(Some(&s), true);
        assert_eq!(result, vec!["filename.txt".to_string()]);

        // Array with mixed types filters non-strings
        let arr = json!(["file1.txt", 42, "file2.txt"]);
        let result = EntityContext::extract_attachment_filenames(Some(&arr), true);
        assert_eq!(
            result,
            vec!["file1.txt".to_string(), "file2.txt".to_string()]
        );
    }

    #[tokio::test]
    async fn migrate_trash_no_op_when_old_layout_absent() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // No old-style trash exists; migration should be a no-op
        ctx.migrate_trash_layout("tag").await.unwrap();
        // Nothing should be created
        assert!(!dir.path().join("tags").join(".trash").exists());
    }

    #[tokio::test]
    async fn entity_def_returns_correct_definition() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let def = ctx.entity_def("tag").unwrap();
        assert_eq!(def.name, "tag");
        assert!(def.body_field.is_none());

        let def = ctx.entity_def("task").unwrap();
        assert_eq!(def.name, "task");
        assert_eq!(def.body_field.as_deref(), Some("body"));
    }

    #[tokio::test]
    async fn entity_def_unknown_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.entity_def("nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown entity type"));
    }

    #[tokio::test]
    async fn read_changelog_empty_when_no_writes() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let log = ctx.read_changelog("tag", "nonexistent").await.unwrap();
        assert!(log.is_empty());
    }

    #[tokio::test]
    async fn read_changelog_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.read_changelog("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn changelog_path_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.changelog_path("unicorn", "x");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn entity_path_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.entity_path("unicorn", "x");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_empty_entity_type() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.list("tag").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.list("unicorn").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_archived_empty() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.list_archived("tag").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_archived_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.list_archived("unicorn").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_archived_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.read_archived("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_archived_not_found_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.read_archived("tag", "nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn delete_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.delete("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn archive_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.archive("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unarchive_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.unarchive("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn restore_from_trash_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.restore_from_trash("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn restore_from_archive_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.restore_from_archive("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn migration_handles_already_existing_dest() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create old-style trash with a file
        let old_trash = dir.path().join(".trash").join("tags");
        tokio::fs::create_dir_all(&old_trash).await.unwrap();
        tokio::fs::write(old_trash.join("dup.yaml"), "tag_name: Dup\n")
            .await
            .unwrap();

        // Also create new-style trash with the same filename already present
        let new_trash = dir.path().join("tags").join(".trash");
        tokio::fs::create_dir_all(&new_trash).await.unwrap();
        tokio::fs::write(new_trash.join("dup.yaml"), "tag_name: Existing\n")
            .await
            .unwrap();

        // Migration should handle the AlreadyExists case gracefully
        ctx.migrate_trash_layout("tag").await.unwrap();

        // The new trash file should still exist (migration skips on AlreadyExists)
        assert!(new_trash.join("dup.yaml").exists());
    }

    #[tokio::test]
    async fn write_task_with_body_round_trips_through_context() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut task = Entity::new("task", "01TEST");
        task.set("title", json!("Test Task"));
        task.set(
            "body",
            json!("# Heading\n\nParagraph text.\n\n- Item 1\n- Item 2"),
        );
        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01TEST").await.unwrap();
        assert_eq!(loaded.get_str("title"), Some("Test Task"));
        assert!(loaded.get_str("body").unwrap().contains("# Heading"));
    }

    #[tokio::test]
    async fn delete_nonexistent_entity_does_not_error() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Deleting an entity that doesn't exist should succeed (moves to trash, nothing found)
        let result = ctx.delete("tag", "nonexistent").await;
        // It succeeds but returns None (no changelog entry since entity had no fields)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn archive_nonexistent_entity_succeeds_with_none() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Archiving entity that doesn't exist should succeed with None
        let result = ctx.archive("tag", "nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn read_changelog_with_trash_fallback_unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.read_changelog_with_trash_fallback("unicorn", "x").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn enrich_attachment_fields_without_compute_engine() {
        // Test that attachment enrichment happens even without a compute engine
        let dir = TempDir::new().unwrap();
        let fields = attachment_fields_context();
        let ctx = EntityContext::new(dir.path(), fields);
        // No .with_compute() — but attachment enrichment should still work

        let source = dir.path().join("photo.png");
        tokio::fs::write(&source, b"png data").await.unwrap();

        let mut entity = Entity::new("item", "01TEST");
        entity.set("title", json!("Test"));
        entity.set("avatar", json!(source.to_string_lossy().to_string()));
        ctx.write(&entity).await.unwrap();

        let read = ctx.read("item", "01TEST").await.unwrap();
        let meta = read.fields.get("avatar").unwrap();
        assert!(
            meta.is_object(),
            "attachment should be enriched without compute engine"
        );
        assert_eq!(meta["name"], "photo.png");
    }

    #[tokio::test]
    async fn list_with_compute_engine_enriches_entities() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());
        let ctx = EntityContext::new(dir.path(), fields.clone()).with_compute(compute);

        let mut t1 = Entity::new("tag", "t1");
        t1.set("tag_name", json!("One"));
        let mut t2 = Entity::new("tag", "t2");
        t2.set("tag_name", json!("Two"));
        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();

        let tags = ctx.list("tag").await.unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[tokio::test]
    async fn read_with_compute_engine_derives_fields() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());
        let ctx = EntityContext::new(dir.path(), fields.clone()).with_compute(compute);

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));
    }

    // -----------------------------------------------------------------------
    // list_where tests (from kanban branch)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn list_where_filters_by_field() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut t1 = Entity::new("tag", "bug");
        t1.set("tag_name", json!("Bug"));
        t1.set("color", json!("#ff0000"));
        let mut t2 = Entity::new("tag", "feature");
        t2.set("tag_name", json!("Feature"));
        t2.set("color", json!("#00ff00"));

        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();

        let result = ctx
            .list_where(
                "tag",
                |entities| crate::filter::EntityFilterContext::new(entities),
                |entity, _ctx| entity.get_str("tag_name") == Some("Bug"),
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.as_ref(), "bug");
    }

    #[tokio::test]
    async fn list_where_with_context_extra() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut t1 = Entity::new("tag", "bug");
        t1.set("tag_name", json!("Bug"));
        let mut t2 = Entity::new("tag", "feature");
        t2.set("tag_name", json!("Feature"));

        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();

        // Inject a set of allowed tag names via extras
        let allowed: std::collections::HashSet<String> =
            ["Feature"].iter().map(|s| s.to_string()).collect();

        let result = ctx
            .list_where(
                "tag",
                |entities| {
                    let mut fctx = crate::filter::EntityFilterContext::new(entities);
                    fctx.insert(allowed.clone());
                    fctx
                },
                |entity, fctx| {
                    let allowed = fctx.get::<std::collections::HashSet<String>>().unwrap();
                    entity
                        .get_str("tag_name")
                        .is_some_and(|name| allowed.contains(name))
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.as_ref(), "feature");
    }

    // -----------------------------------------------------------------------
    // _changelog injection tests
    // -----------------------------------------------------------------------

    /// Build a FieldsContext whose "task" entity includes a computed field
    /// that depends on `_changelog`.
    fn fields_context_with_changelog_computed() -> Arc<FieldsContext> {
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

    /// Build a FieldsContext whose "task" entity has a computed field
    /// that does NOT depend on `_changelog`.
    fn fields_context_with_plain_computed() -> Arc<FieldsContext> {
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
                "upper_title",
                "id: 00000000000000000000000UPR\nname: upper_title\ntype:\n  kind: computed\n  derive: upper-title\n",
            ),
        ];
        let entities = vec![(
            "task",
            "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - upper_title\n",
        )];
        let dir = TempDir::new().unwrap();
        Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
    }

    /// Build a ComputeEngine with a "count-changelog" derivation that reads
    /// the `_changelog` array and returns its length.
    fn compute_engine_with_changelog_counter() -> Arc<swissarmyhammer_fields::ComputeEngine> {
        let mut engine = swissarmyhammer_fields::ComputeEngine::new();
        engine.register(
            "count-changelog",
            Box::new(|fields| {
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

    /// Build a ComputeEngine with an "upper-title" derivation that does
    /// not need `_changelog`.
    fn compute_engine_with_upper_title() -> Arc<swissarmyhammer_fields::ComputeEngine> {
        let mut engine = swissarmyhammer_fields::ComputeEngine::new();
        engine.register(
            "upper-title",
            Box::new(|fields| {
                let title = fields
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_uppercase();
                Box::pin(async move { serde_json::Value::String(title) })
            }),
        );
        Arc::new(engine)
    }

    #[tokio::test]
    async fn changelog_injected_for_changelog_dependent_computed_field() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_changelog_computed();
        let compute = compute_engine_with_changelog_counter();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        // Write a task (legacy fallback doesn't write changelog entries)
        let mut task = Entity::new("task", "01ABC");
        task.set("title", json!("Hello"));
        ctx.write(&task).await.unwrap();

        // Manually append changelog entries so read_changelog finds them
        let log_path = ctx.changelog_path("task", "01ABC").unwrap();
        let entry1 = ChangeEntry::new(
            "task",
            "01ABC",
            "create",
            vec![(
                "title".into(),
                FieldChange::Set {
                    value: json!("Hello"),
                },
            )],
        );
        let entry2 = ChangeEntry::new(
            "task",
            "01ABC",
            "update",
            vec![(
                "title".into(),
                FieldChange::Changed {
                    old_value: json!("Hello"),
                    new_value: json!("Updated"),
                },
            )],
        );
        changelog::append_changelog(&log_path, &entry1)
            .await
            .unwrap();
        changelog::append_changelog(&log_path, &entry2)
            .await
            .unwrap();

        // Read the entity — derivation should see 2 changelog entries
        let loaded = ctx.read("task", "01ABC").await.unwrap();
        let count = loaded.fields.get("change_count").unwrap().as_u64().unwrap();
        assert_eq!(count, 2, "expected 2 changelog entries, got {}", count);
    }

    #[tokio::test]
    async fn changelog_not_injected_for_non_changelog_computed_field() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_plain_computed();
        let compute = compute_engine_with_upper_title();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        let mut task = Entity::new("task", "01ABC");
        task.set("title", json!("hello"));
        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01ABC").await.unwrap();
        // The derivation ran successfully without _changelog
        assert_eq!(loaded.get_str("upper_title"), Some("HELLO"));
        // _changelog was never injected, so it should not appear
        assert!(
            !loaded.fields.contains_key("_changelog"),
            "_changelog should not be present in entity fields"
        );
    }

    #[tokio::test]
    async fn changelog_stripped_after_derivation() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_changelog_computed();
        let compute = compute_engine_with_changelog_counter();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        let mut task = Entity::new("task", "01ABC");
        task.set("title", json!("Test"));
        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01ABC").await.unwrap();
        assert!(
            !loaded.fields.contains_key("_changelog"),
            "_changelog must be stripped from entity fields after derivation"
        );
        // But the computed field was still derived
        assert!(loaded.fields.contains_key("change_count"));
    }

    /// Build a FieldsContext whose "task" entity includes a computed field
    /// that depends on `_file_created`.
    fn fields_context_with_file_created_computed() -> Arc<FieldsContext> {
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
                "file_ts",
                "id: 00000000000000000000000FTS\nname: file_ts\ntype:\n  kind: computed\n  derive: capture-file-created\n  depends_on:\n    - _file_created\n",
            ),
        ];
        let entities = vec![(
            "task",
            "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - file_ts\n",
        )];
        let dir = TempDir::new().unwrap();
        Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
    }

    /// Build a ComputeEngine with a "capture-file-created" derivation that
    /// returns the injected `_file_created` value verbatim.
    fn compute_engine_with_file_created_capture() -> Arc<swissarmyhammer_fields::ComputeEngine> {
        let mut engine = swissarmyhammer_fields::ComputeEngine::new();
        engine.register(
            "capture-file-created",
            Box::new(|fields| {
                let v = fields
                    .get("_file_created")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                Box::pin(async move { v })
            }),
        );
        Arc::new(engine)
    }

    #[tokio::test]
    async fn apply_compute_injects_file_created_when_field_depends_on_it() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_file_created_computed();
        let compute = compute_engine_with_file_created_capture();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        let mut task = Entity::new("task", "01FILE");
        task.set("title", json!("File ts test"));
        let before = std::time::SystemTime::now();
        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01FILE").await.unwrap();
        let ts_str = loaded
            .fields
            .get("file_ts")
            .and_then(|v| v.as_str())
            .expect("file_ts should resolve to an RFC 3339 string");

        // Parse the timestamp and verify it falls within ±5 seconds of the
        // write window.
        let parsed = chrono::DateTime::parse_from_rfc3339(ts_str)
            .unwrap_or_else(|e| panic!("file_ts {ts_str:?} should parse as RFC 3339: {e}"));
        let ts_system: std::time::SystemTime = parsed.into();

        let lower = before
            .checked_sub(std::time::Duration::from_secs(5))
            .unwrap();
        let upper = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        assert!(
            ts_system >= lower && ts_system <= upper,
            "file_ts {ts_str} should be within ±5s of write time",
        );
    }

    #[tokio::test]
    async fn apply_compute_strips_file_created_after_derivation() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_file_created_computed();
        let compute = compute_engine_with_file_created_capture();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        let mut task = Entity::new("task", "01STRIP");
        task.set("title", json!("Strip test"));
        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01STRIP").await.unwrap();
        assert!(
            !loaded.fields.contains_key("_file_created"),
            "_file_created must be stripped from entity fields after derivation"
        );
        // Capture field was still populated
        assert!(loaded.fields.contains_key("file_ts"));
    }

    /// Exercises the "entity file missing" branch of the injector
    /// (`tokio::fs::metadata(&path).await` fails) to lock in the no-panic /
    /// Null-return contract. `read()` cannot reach this branch because it fails
    /// earlier when the file is absent; call `apply_compute_with_query`
    /// directly with a hand-built entity whose id has no corresponding file.
    #[tokio::test]
    async fn apply_compute_file_created_null_when_md_missing() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_file_created_computed();
        let compute = compute_engine_with_file_created_capture();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        let mut entity = Entity::new("task", "01PHANTOM");
        entity.set("title", json!("Phantom"));

        let query_fn = ctx.build_entity_query_fn();
        ctx.apply_compute_with_query("task", &mut entity, &query_fn)
            .await
            .expect("apply_compute_with_query must not error on missing file");

        let captured = entity
            .fields
            .get("file_ts")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        assert!(
            captured.is_null(),
            "file_ts should be Null when the entity file is missing, got {captured:?}"
        );
        assert!(
            !entity.fields.contains_key("_file_created"),
            "_file_created must still be stripped after a Null injection"
        );
    }

    #[tokio::test]
    async fn changelog_empty_array_when_no_changelog_file_exists() {
        let dir = TempDir::new().unwrap();
        let fields = fields_context_with_changelog_computed();
        let compute = compute_engine_with_changelog_counter();
        let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

        // Write an entity but do NOT write any changelog entries.
        // The JSONL file simply does not exist.
        let mut task = Entity::new("task", "01XYZ");
        task.set("title", json!("Brand new"));
        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01XYZ").await.unwrap();
        // With no changelog file, _changelog should be injected as []
        // and the derivation should see count == 0.
        let count = loaded.fields.get("change_count").unwrap().as_u64().unwrap();
        assert_eq!(
            count, 0,
            "expected 0 changelog entries for an entity with no changelog file"
        );
    }

    #[tokio::test]
    async fn list_where_predicate_accesses_all_entities() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut t1 = Entity::new("tag", "a");
        t1.set("tag_name", json!("Alpha"));
        let mut t2 = Entity::new("tag", "b");
        t2.set("tag_name", json!("Beta"));
        let mut t3 = Entity::new("tag", "c");
        t3.set("tag_name", json!("Charlie"));

        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();
        ctx.write(&t3).await.unwrap();

        // Keep only entities when total count > 2 (cross-entity logic)
        let result = ctx
            .list_where(
                "tag",
                |entities| crate::filter::EntityFilterContext::new(entities),
                |_entity, fctx| fctx.entities.len() > 2,
            )
            .await
            .unwrap();

        // All 3 pass because entities.len() == 3 > 2
        assert_eq!(result.len(), 3);
    }

    // =========================================================================
    // EntityCache integration tests
    // =========================================================================

    /// When a cache is attached, `list()` should serve from the in-memory map
    /// and not hit `io::read_entity_dir` — beyond the single preload call
    /// issued by `EntityCache::load_all`.
    #[tokio::test]
    async fn test_list_hits_cache_not_disk() {
        use crate::cache::EntityCache;
        use crate::io::READ_ENTITY_DIR_CALLS;
        use std::sync::atomic::Ordering;

        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();

        // Seed some entities on disk through a bare context.
        let seed_ctx = EntityContext::new(dir.path(), Arc::clone(&fields));
        for i in 0..5 {
            let mut tag = Entity::new("tag", format!("t{i}"));
            tag.set("tag_name", json!(format!("Tag {i}")));
            seed_ctx.write(&tag).await.unwrap();
        }
        drop(seed_ctx);

        // Build a fresh cache-wired context.
        let ctx = Arc::new(EntityContext::new(dir.path(), Arc::clone(&fields)));
        let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        ctx.attach_cache(&cache);

        // One preload call hits disk.
        let before = READ_ENTITY_DIR_CALLS.load(Ordering::Relaxed);
        cache.load_all("tag").await.unwrap();
        let after_load = READ_ENTITY_DIR_CALLS.load(Ordering::Relaxed);
        assert_eq!(
            after_load - before,
            1,
            "load_all should issue exactly one read_entity_dir"
        );

        // 100 list calls must serve from cache — zero additional disk reads.
        for _ in 0..100 {
            let tags = ctx.list("tag").await.unwrap();
            assert_eq!(tags.len(), 5);
        }

        let after_list = READ_ENTITY_DIR_CALLS.load(Ordering::Relaxed);
        assert_eq!(
            after_list - after_load,
            0,
            "100 list() calls after load_all must not touch disk"
        );
    }

    /// When a cache is attached, `EntityContext::write` delegates to
    /// `EntityCache::write`, which emits an `EntityChanged` event with the
    /// field-level diff (the sub-card 1 shape).
    #[tokio::test]
    async fn test_write_goes_through_cache_when_attached() {
        use crate::cache::EntityCache;
        use crate::events::EntityEvent;

        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();

        let ctx = Arc::new(EntityContext::new(dir.path(), Arc::clone(&fields)));
        let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        ctx.attach_cache(&cache);

        // Subscribe before the write so we catch the event.
        let mut rx = cache.subscribe();

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Exactly one EntityChanged event with a non-empty `changes` vec.
        let evt = rx.try_recv().expect("expected EntityChanged event");
        match evt {
            EntityEvent::EntityChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "bug");
                assert!(
                    !changes.is_empty(),
                    "new entity should report fields in `changes`"
                );
            }
            other => panic!("expected EntityChanged, got {other:?}"),
        }

        // And the cache is populated — subsequent reads don't hit disk.
        let cached = cache.get("tag", "bug").await.unwrap();
        assert_eq!(cached.get_str("tag_name"), Some("Bug"));
    }

    /// Attaching the cache twice is a programming error — second call panics.
    #[tokio::test]
    #[should_panic(expected = "attach_cache called more than once")]
    async fn test_attach_cache_twice_panics() {
        use crate::cache::EntityCache;

        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = Arc::new(EntityContext::new(dir.path(), fields));
        let cache1 = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        let cache2 = Arc::new(EntityCache::new(Arc::clone(&ctx)));
        ctx.attach_cache(&cache1);
        ctx.attach_cache(&cache2); // should panic
    }
}
