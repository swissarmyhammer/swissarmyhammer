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
    ComputeEngine, EntityDef, EntityTypeName, FieldType, FieldsContext, ValidationEngine,
};
use swissarmyhammer_store::{StoreContext, StoreHandle, StoredItemId, UndoEntryId};
use tokio::sync::RwLock;

use crate::changelog::{self, ChangeEntry};
use crate::entity::{Entity, EntityLocation};
use crate::error::{EntityError, Result};
use crate::id_types::EntityId;
use crate::io;
use crate::store::EntityTypeStore;

/// One of the staging directories an entity's files can be moved into.
///
/// Each entity type keeps its live, trashed, and archived files under the same
/// parent (`{root}/{type}s/`), so a staging directory is fully described by
/// which subdirectory it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StagingDir {
    /// `{root}/{type}s/.trash/` — where `delete()` moves files.
    Trash,
    /// `{root}/{type}s/.archive/` — where `archive()` moves files.
    Archive,
}

/// A staging operation that moves an entity's files between live storage and
/// a [`StagingDir`].
///
/// `delete`, `archive`, and `unarchive` share one routing path — cache
/// delegation, store-handle delegation, undo-stack push, and legacy file-move
/// fallback — and differ only by this discriminant. Holding the difference in
/// data rather than in three parallel method bodies keeps them from drifting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StagingOp {
    /// Move the live file into `.trash/`.
    Delete,
    /// Move the live file into `.archive/`.
    Archive,
    /// Move the most recently archived file back into live storage.
    Unarchive,
}

impl StagingOp {
    /// The verb this operation records in its undo-stack label.
    fn label(self) -> &'static str {
        match self {
            StagingOp::Delete => "delete",
            StagingOp::Archive => "archive",
            StagingOp::Unarchive => "unarchive",
        }
    }
}

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
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<PathBuf> {
        let entity_type = entity_type.into();
        let def = self.entity_def(&entity_type)?;
        Ok(io::entity_file_path(
            &self.entity_dir(&entity_type),
            id.into(),
            def,
        ))
    }

    /// Get the changelog path for a specific entity.
    pub fn changelog_path(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
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
    pub async fn read(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Entity> {
        let entity_type = entity_type.into();
        let id = id.into();
        if let Some(cache) = self.attached_cache() {
            if let Some(mut entity) = cache.get(&entity_type, &id).await {
                self.apply_compute(&entity_type, &mut entity).await?;
                return Ok(entity);
            }
        }
        self.read_internal(&entity_type, &id).await
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

        // Push onto the shared undo stack if a StoreContext is available.
        //
        // The store handle is now the sole writer of the per-entity
        // changelog — it appends a store-format `ChangelogEntry` (patch-based)
        // that the projecting reader translates back into field-level diffs
        // for the activity history pane. The entity layer used to append a
        // second legacy `ChangeEntry` here; that dual-write was removed by
        // card 01KQ5FJ0VXEQZVKHZBN49Q5GFS.
        if let (Some(sc), Some(eid)) = (self.store_context.get(), &entry_id) {
            let is_create = previous.is_none();
            let op = if is_create { "create" } else { "update" };
            let label = format!("{} {} {}", op, entity.entity_type, entity.id);
            let item_id = StoredItemId::from(entity.id.as_str());
            sc.push(*eid, label, item_id).await;
        }
        Ok(entry_id)
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
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Option<UndoEntryId>> {
        let entity_type = entity_type.into();
        let id = id.into();
        self.stage(&entity_type, &id, StagingOp::Delete).await
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
    ) -> Result<Option<UndoEntryId>> {
        self.stage_internal(entity_type, id, StagingOp::Delete)
            .await
    }

    /// Run a staging operation, routing through the cache when one is attached.
    ///
    /// `delete`, `archive`, and `unarchive` route identically and differ only
    /// by their [`StagingOp`], so the routing lives here once.
    async fn stage(
        &self,
        entity_type: &str,
        id: &str,
        op: StagingOp,
    ) -> Result<Option<UndoEntryId>> {
        if let Some(cache) = self.attached_cache() {
            return match op {
                StagingOp::Delete => cache.delete(entity_type, id).await,
                StagingOp::Archive => cache.archive(entity_type, id).await,
                StagingOp::Unarchive => cache.unarchive(entity_type, id).await,
            };
        }
        self.stage_internal(entity_type, id, op).await
    }

    /// Run a staging operation directly on disk, bypassing any attached cache.
    ///
    /// Delegates to the registered [`StoreHandle`] when the entity type has
    /// one: the handle records a store-format changelog entry, moves the
    /// files, and the entry is pushed onto the shared undo stack. The entity
    /// layer used to append a second legacy `ChangeEntry` here; that
    /// dual-write was removed by card 01KQ5FJ0VXEQZVKHZBN49Q5GFS.
    async fn stage_internal(
        &self,
        entity_type: &str,
        id: &str,
        op: StagingOp,
    ) -> Result<Option<UndoEntryId>> {
        let def = self.entity_def(entity_type)?;
        let path = io::entity_file_path(&self.entity_dir(entity_type), id, def);

        if op == StagingOp::Delete {
            // Trash the entity's attachment files before its data file
            // leaves live storage.
            if let Ok(previous) = io::read_entity(&path, entity_type, id, def).await {
                self.trash_entity_attachments(entity_type, &previous)
                    .await?;
            }
        }

        let store_handle = self.store_handles.read().await.get(entity_type).cloned();
        let Some(sh) = store_handle else {
            return self.stage_fallback(&path, entity_type, op).await;
        };

        let entity_id = EntityId::from(id);
        let entry_id = match op {
            StagingOp::Delete => sh.delete(&entity_id).await?,
            StagingOp::Archive => sh.archive(&entity_id).await?,
            StagingOp::Unarchive => sh.unarchive_latest(&entity_id).await?.1,
        };

        // Push onto the shared undo stack if a StoreContext is available
        if let Some(sc) = self.store_context.get() {
            let label = format!("{} {} {}", op.label(), entity_type, id);
            let item_id = StoredItemId::from(id);
            sc.push(entry_id, label, item_id).await;
        }
        Ok(Some(entry_id))
    }

    /// Legacy file-move fallback for entity types with no registered
    /// [`StoreHandle`].
    ///
    /// Reached only in tests and for entity types with no store — production
    /// always registers a handle — so callers that depend on a changelog
    /// entry must register one.
    async fn stage_fallback(
        &self,
        path: &Path,
        entity_type: &str,
        op: StagingOp,
    ) -> Result<Option<UndoEntryId>> {
        match op {
            StagingOp::Delete => {
                io::trash_entity_files(path, &self.staging_dir(entity_type, StagingDir::Trash))
                    .await?;
            }
            StagingOp::Archive => {
                io::trash_entity_files(path, &self.staging_dir(entity_type, StagingDir::Archive))
                    .await?;
            }
            StagingOp::Unarchive => {
                io::restore_entity_files(path, &self.staging_dir(entity_type, StagingDir::Archive))
                    .await?;
            }
        }
        Ok(None)
    }

    /// Resolve a [`StagingDir`] to its path for an entity type.
    fn staging_dir(&self, entity_type: &str, staging: StagingDir) -> PathBuf {
        match staging {
            StagingDir::Trash => self.trash_dir(entity_type),
            StagingDir::Archive => self.archive_dir(entity_type),
        }
    }

    /// Restore an entity from trash back to live storage.
    ///
    /// Moves the entity data file and changelog from the trash directory
    /// (`{root}/{type}s/.trash/`) back to the live storage directory.
    /// This is the inverse of the trash operation performed by `delete()`.
    pub async fn restore_from_trash(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<()> {
        let entity_type = entity_type.into();
        let id = id.into();
        self.restore(&entity_type, &id, StagingDir::Trash).await
    }

    /// Restore an entity from the archive back to live storage.
    ///
    /// Moves the entity data file and changelog from the archive directory
    /// (`{root}/{type}s/.archive/`) back to the live storage directory.
    /// This is the inverse of the archive operation performed by `archive()`.
    pub async fn restore_from_archive(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<()> {
        let entity_type = entity_type.into();
        let id = id.into();
        self.restore(&entity_type, &id, StagingDir::Archive).await
    }

    /// Restore an entity from a staging directory and refresh the cache.
    ///
    /// Restoring from `.trash/` and from `.archive/` differ only by the
    /// source directory, so both public entry points share this body.
    async fn restore(&self, entity_type: &str, id: &str, from: StagingDir) -> Result<()> {
        self.restore_internal(entity_type, id, from).await?;
        // Refresh the cache so the restored entity shows up in `list()`.
        if let Some(cache) = self.attached_cache() {
            let _ = cache.refresh_from_disk(entity_type, id).await;
        }
        Ok(())
    }

    /// Pure disk restore from a staging directory, bypassing any attached cache.
    async fn restore_internal(&self, entity_type: &str, id: &str, from: StagingDir) -> Result<()> {
        let def = self.entity_def(entity_type)?;
        let path = io::entity_file_path(&self.entity_dir(entity_type), id, def);
        io::restore_entity_files(&path, &self.staging_dir(entity_type, from)).await
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
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Option<UndoEntryId>> {
        let entity_type = entity_type.into();
        let id = id.into();
        self.stage(&entity_type, &id, StagingOp::Archive).await
    }

    /// Archive an entity directly on disk, bypassing any attached cache.
    ///
    /// This is the pure archive path — the cache itself calls it, and
    /// `archive()` falls through to it when no cache is attached.
    pub(crate) async fn archive_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Option<UndoEntryId>> {
        self.stage_internal(entity_type, id, StagingOp::Archive)
            .await
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
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Option<UndoEntryId>> {
        let entity_type = entity_type.into();
        let id = id.into();
        self.stage(&entity_type, &id, StagingOp::Unarchive).await
    }

    /// Unarchive an entity directly on disk, bypassing any attached cache.
    pub(crate) async fn unarchive_internal(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Option<UndoEntryId>> {
        self.stage_internal(entity_type, id, StagingOp::Unarchive)
            .await
    }

    /// Reconcile the cache entry for `(entity_type, id)` with the current
    /// on-disk state — used by the undo/redo command layer after a
    /// `StoreContext` operation has rewritten the file.
    ///
    /// `StoreContext::undo` and `redo` mutate the live data file directly
    /// (trashing, restoring from trash, moving between `.archive/` and
    /// live, etc.) without routing through `EntityContext::write` or
    /// `EntityContext::delete`, so the cache can hold stale state after
    /// an undo — a just-created entity that undo trashed would still
    /// appear to `read()`, a just-archived entity that undo restored
    /// would still be missing from `list()`.
    ///
    /// This method re-reads the live path for the entity:
    /// - If the file exists, the cache is refreshed to match disk (and
    ///   an `EntityChanged` event fires if the content differs).
    /// - If the file is gone (undo of a create, redo of a delete, redo
    ///   of an archive), the cache entry is evicted and an
    ///   `EntityDeleted` event fires.
    ///
    /// A no-op when no cache is attached, when the entity type is not
    /// registered, or when the file state already matches the cache.
    ///
    /// This exists specifically to bridge the store layer (which knows
    /// how to reverse bytes on disk) and the cache layer (which keeps
    /// an in-memory index of those bytes); see the [`UndoCmd`] /
    /// [`RedoCmd`] implementations that call it.
    ///
    /// [`UndoCmd`]: crate::UndoCmd
    /// [`RedoCmd`]: crate::RedoCmd
    pub async fn sync_entity_cache_from_disk(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) {
        let entity_type = entity_type.into();
        let id = id.into();
        let Some(cache) = self.attached_cache() else {
            return;
        };
        // An unknown entity type means the registry never saw a YAML for
        // it; nothing to reconcile.
        let def = match self.entity_def(&entity_type) {
            Ok(def) => def,
            Err(_) => return,
        };
        let path = io::entity_file_path(&self.entity_dir(&entity_type), &id, def);

        if path.exists() {
            // File present — pull disk state into the cache. Errors here
            // are non-fatal: `refresh_from_disk` only fails if the file
            // cannot be parsed, which means the cache is the better of
            // two bad options. Log and continue.
            if let Err(e) = cache.refresh_from_disk(&entity_type, &id).await {
                tracing::warn!(
                    entity_type = entity_type.as_str(),
                    id = id.as_str(),
                    error = %e,
                    "sync_entity_cache_from_disk: refresh_from_disk failed"
                );
            }
        } else {
            // File absent — the undo/redo either trashed it or moved it
            // to `.archive/`. Drop the cache entry so `read`/`list`
            // surface the deletion immediately.
            cache.evict(&entity_type, &id).await;
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
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Entity> {
        let entity_type = entity_type.into();
        let id = id.into();
        let def = self.entity_def(&entity_type)?;
        let path = io::entity_file_path(&self.archive_dir(&entity_type), &id, def);
        let mut entity = io::read_entity(&path, &entity_type, &id, def).await?;
        entity.location = EntityLocation::Archive;
        self.apply_compute(&entity_type, &mut entity).await?;
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

    /// Read the changelog for an entity, projecting store-layer text
    /// patches into field-level diffs.
    ///
    /// Delegates to [`changelog::read_changelog_for`] using the entity
    /// type's [`EntityDef`] so that records written by the store layer (text
    /// patches via `diffy`) are replayed forward and surfaced as
    /// `ChangeEntry`s with populated `changes`. Mixed-shape files (legacy
    /// entity-format lines plus store-format lines) are merged in
    /// chronological order.
    pub async fn read_changelog(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Vec<ChangeEntry>> {
        let entity_type = entity_type.into();
        let def = self.entity_def(&entity_type)?;
        let log_path = self.changelog_path(entity_type.clone(), id)?;
        changelog::read_changelog_for(&entity_type, def, &log_path).await
    }

    /// Read the changelog for an entity, falling back to the trash directory
    /// if the live changelog does not exist (e.g. the entity was deleted),
    /// and further falling back to the archive directory if neither the live
    /// nor trash changelog exists (e.g. the entity was archived).
    pub async fn read_changelog_with_trash_fallback(
        &self,
        entity_type: impl Into<EntityTypeName>,
        id: impl Into<EntityId>,
    ) -> Result<Vec<ChangeEntry>> {
        let entity_type = entity_type.into();
        let id = id.into();
        let live_path = self.changelog_path(entity_type.clone(), id.clone())?;
        let def = self.entity_def(&entity_type)?;
        let file_stem = io::entity_file_path(&self.entity_dir(&entity_type), &id, def)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&id)
            .to_string();

        let trash_path = self
            .staging_dir(&entity_type, StagingDir::Trash)
            .join(format!("{file_stem}.jsonl"));

        // Try live first, then trash, then archive
        let entries =
            changelog::read_changelog_with_fallback(&entity_type, def, &live_path, &trash_path)
                .await?;
        if entries.is_empty() && !live_path.exists() && !trash_path.exists() {
            let archive_path = self
                .staging_dir(&entity_type, StagingDir::Archive)
                .join(format!("{file_stem}.jsonl"));
            return changelog::read_changelog_for(&entity_type, def, &archive_path).await;
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
            let fd = field_defs
                .iter()
                .find(|f| f.name == name.as_str())
                .expect("names_to_validate is collected from field_defs");
            let value = entity
                .fields
                .get(name)
                .cloned()
                .expect("names_to_validate only holds names present in entity.fields");
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
mod tests;
