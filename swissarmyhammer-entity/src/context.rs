//! EntityContext — root-aware I/O coordinator for dynamic entities.
//!
//! Given a storage root and a FieldsContext, this handles all directory
//! resolution, file I/O, and changelog management. Consumers (like kanban)
//! create an EntityContext and delegate all entity I/O to it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use swissarmyhammer_fields::{
    ComputeEngine, EntityDef, FieldType, FieldsContext, ValidationEngine,
};
use swissarmyhammer_store::{StoreContext, StoreHandle, StoredItemId};
use tokio::sync::RwLock;

use crate::changelog::{self, ChangeEntry, FieldChange};
use crate::entity::Entity;
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
        }
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
    /// If a `ComputeEngine` is attached, computed fields are derived after reading.
    pub async fn read(&self, entity_type: impl AsRef<str>, id: impl AsRef<str>) -> Result<Entity> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        let def = self.entity_def(entity_type)?;
        let path = io::entity_file_path(&self.entity_dir(entity_type), id, def);
        let mut entity = io::read_entity(&path, entity_type, id, def).await?;
        self.apply_compute(entity_type, &mut entity).await?;
        Ok(entity)
    }

    /// Write an entity, automatically computing and logging field-level changes.
    ///
    /// If a `ValidationEngine` is attached, fields are validated/transformed
    /// before writing. Computed fields are stripped (they are derived on read).
    /// If a previous version exists, diffs against it and appends a changelog
    /// entry. On creation (no previous version), all fields are logged as `Set`.
    ///
    /// Returns `Ok(Some(ulid))` when changes were logged, or `Ok(None)` when
    /// no changes were detected (idempotent write).
    pub async fn write(
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

        if let Some(sh) = store_handle {
            let entry_id = sh.write(&entity).await?;

            // Append a legacy field-level changelog entry so that the activity
            // log (which reads per-entity JSONL) continues to work even when
            // I/O is delegated to a StoreHandle.
            if entry_id.is_some() {
                let is_create = previous.is_none();
                let op = if is_create { "create" } else { "update" };
                let changes = if let Some(ref old) = previous {
                    changelog::diff_entities(old, &entity)
                } else {
                    entity
                        .fields
                        .iter()
                        .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
                        .collect()
                };
                if !changes.is_empty() {
                    let entry = ChangeEntry::new(
                        entity.entity_type.as_str(),
                        entity.id.as_str(),
                        op,
                        changes,
                    );
                    let log_path = path.with_extension("jsonl");
                    changelog::append_changelog(&log_path, &entry).await?;
                }
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
        } else {
            // Fallback for tests or entity types without a registered store
            io::write_entity(&path, &entity, def).await?;
            Ok(None)
        }
    }

    /// Delete an entity by type and ID.
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
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);
        let archive = self.archive_dir(entity_type);
        io::restore_entity_files(&path, &archive).await
    }

    /// Archive an entity by type and ID.
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
    /// If a `ComputeEngine` is attached, computed fields are derived for each entity.
    pub async fn list(&self, entity_type: impl AsRef<str>) -> Result<Vec<Entity>> {
        let entity_type = entity_type.as_ref();
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let mut entities = io::read_entity_dir(&dir, entity_type, def).await?;
        if self.compute.is_some() {
            let query_fn = self.build_entity_query_fn();
            for entity in &mut entities {
                self.apply_compute_with_query(entity_type, entity, &query_fn)
                    .await?;
            }
        }
        Ok(entities)
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

        // Apply defaults for missing fields
        for fd in &field_defs {
            if matches!(&fd.type_, FieldType::Computed { .. }) {
                continue;
            }
            if !entity.fields.contains_key(fd.name.as_str()) {
                if let Some(ref default) = fd.default {
                    entity.set(fd.name.to_string(), default.clone());
                }
            }
        }

        // Process attachment fields — copy source files, validate sizes.
        let entity_type_dir = self.entity_dir(&entity_type);
        for fd in &field_defs {
            if let FieldType::Attachment {
                max_bytes,
                multiple,
            } = &fd.type_
            {
                self.process_attachment_field(
                    &mut entity,
                    fd.name.as_str(),
                    *max_bytes,
                    *multiple,
                    &entity_type_dir,
                )
                .await?;
            }
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

        if multiple {
            // Array of attachment values
            let values = match value {
                Value::Array(arr) => arr,
                Value::Null => return Ok(()),
                // Single value provided for a multiple field — wrap in array
                other => vec![other],
            };
            let mut result = Vec::new();
            for v in values {
                match v {
                    Value::String(s) => {
                        let stored = self
                            .resolve_attachment_value(&s, field_name, max_bytes, entity_type_dir)
                            .await?;
                        result.push(Value::String(stored));
                    }
                    Value::Object(ref obj) => {
                        // Enriched metadata object from a read round-trip.
                        // Reconstruct the stored filename as `{id}-{name}` and
                        // verify it still exists in `.attachments/`.
                        if let (Some(id), Some(name)) = (
                            obj.get("id").and_then(|v| v.as_str()),
                            obj.get("name").and_then(|v| v.as_str()),
                        ) {
                            let stored = format!("{}-{}", id, name);
                            let att_dir = crate::io::attachments_dir(entity_type_dir);
                            let path = att_dir.join(&stored);
                            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                                result.push(Value::String(stored));
                            } else {
                                return Err(EntityError::AttachmentNotFound {
                                    field: field_name.to_string(),
                                    filename: stored,
                                });
                            }
                        }
                    }
                    other => {
                        tracing::warn!(
                            field = field_name,
                            value = ?other,
                            "skipping non-string/non-object value in attachment array"
                        );
                    }
                }
            }
            entity.set(field_name, Value::Array(result));
        } else {
            // Single attachment value
            match value {
                Value::String(s) => {
                    let stored = self
                        .resolve_attachment_value(&s, field_name, max_bytes, entity_type_dir)
                        .await?;
                    entity.set(field_name, Value::String(stored));
                }
                Value::Object(ref obj) => {
                    // Enriched metadata object — reconstruct stored filename
                    if let (Some(id), Some(name)) = (
                        obj.get("id").and_then(|v| v.as_str()),
                        obj.get("name").and_then(|v| v.as_str()),
                    ) {
                        let stored = format!("{}-{}", id, name);
                        let att_dir = crate::io::attachments_dir(entity_type_dir);
                        let path = att_dir.join(&stored);
                        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                            entity.set(field_name, Value::String(stored));
                        } else {
                            return Err(EntityError::AttachmentNotFound {
                                field: field_name.to_string(),
                                filename: stored,
                            });
                        }
                    }
                }
                Value::Null => {}
                _ => {}
            }
        }

        Ok(())
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
    /// The query reads raw entities from disk (without applying compute)
    /// to avoid infinite recursion.
    fn build_entity_query_fn(&self) -> std::sync::Arc<swissarmyhammer_fields::EntityQueryFn> {
        let root = self.root.clone();
        let fields_ctx = Arc::clone(&self.fields);
        std::sync::Arc::new(Box::new(move |et: &str| {
            let root = root.clone();
            let fields_ctx = Arc::clone(&fields_ctx);
            let et = et.to_string();
            Box::pin(async move {
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
    async fn apply_compute_with_query(
        &self,
        entity_type: &str,
        entity: &mut Entity,
        query_fn: &std::sync::Arc<swissarmyhammer_fields::EntityQueryFn>,
    ) -> Result<()> {
        // Enrich attachment fields with metadata (runs regardless of ComputeEngine)
        self.enrich_attachment_fields(entity_type, entity).await?;

        let Some(ref engine) = self.compute else {
            return Ok(());
        };
        let field_defs = self.fields.fields_for_entity(entity_type);
        let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
        engine
            .derive_all(&mut entity.fields, &owned_defs, Some(query_fn))
            .await
            .map_err(|e| {
                let (field, message) = match &e {
                    swissarmyhammer_fields::FieldsError::ComputeError { field, message } => {
                        (field.clone(), message.clone())
                    }
                    other => (String::new(), other.to_string()),
                };
                EntityError::ComputeError { field, message }
            })?;
        Ok(())
    }

    /// Enrich attachment fields with metadata objects on read.
    ///
    /// Replaces stored filenames with rich JSON objects containing
    /// id, name, size, mime_type, and absolute path.
    async fn enrich_attachment_fields(&self, entity_type: &str, entity: &mut Entity) -> Result<()> {
        use serde_json::Value;

        let field_defs = self.fields.fields_for_entity(entity_type);
        let entity_type_dir = self.entity_dir(entity_type);

        for fd in &field_defs {
            if let FieldType::Attachment { multiple, .. } = &fd.type_ {
                let Some(value) = entity.fields.get(fd.name.as_str()).cloned() else {
                    continue;
                };

                if *multiple {
                    let filenames = match value {
                        Value::Array(arr) => arr,
                        Value::Null => continue,
                        other => vec![other],
                    };
                    let mut metadata_arr = Vec::new();
                    for v in filenames {
                        if let Value::String(filename) = v {
                            if let Some(meta) =
                                io::attachment_metadata(&filename, &entity_type_dir).await
                            {
                                metadata_arr.push(meta);
                            }
                        }
                    }
                    entity.set(fd.name.to_string(), Value::Array(metadata_arr));
                } else if let Value::String(filename) = value {
                    if let Some(meta) = io::attachment_metadata(&filename, &entity_type_dir).await {
                        entity.set(fd.name.to_string(), meta);
                    }
                }
            }
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
                        .map_or(false, |name| allowed.contains(name))
                },
            )
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id.as_ref(), "feature");
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
}
