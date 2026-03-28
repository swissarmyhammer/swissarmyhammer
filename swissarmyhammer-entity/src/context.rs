//! EntityContext — root-aware I/O coordinator for dynamic entities.
//!
//! Given a storage root and a FieldsContext, this handles all directory
//! resolution, file I/O, and changelog management. Consumers (like kanban)
//! create an EntityContext and delegate all entity I/O to it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swissarmyhammer_fields::{
    ComputeEngine, EntityDef, EntityTypeName, FieldType, FieldsContext, ValidationEngine,
};
use tokio::sync::RwLock;

use crate::changelog::{self, ChangeEntry, FieldChange};
use crate::entity::Entity;
use crate::error::{EntityError, Result};
use crate::id_types::{ChangeEntryId, EntityId, TransactionId};
use crate::io;
use crate::undo_stack::UndoStack;

/// Root-aware I/O coordinator for dynamic entities.
///
/// Maps entity types to storage directories under a root path,
/// handles read/write/delete/list, and manages per-entity changelogs.
pub struct EntityContext {
    root: PathBuf,
    fields: Arc<FieldsContext>,
    validation: Option<Arc<ValidationEngine>>,
    compute: Option<Arc<ComputeEngine>>,
    /// Maps ChangeEntry ULID to (entity_type, entity_id) for reverse lookups.
    changelog_index: RwLock<HashMap<ChangeEntryId, (EntityTypeName, EntityId)>>,
    /// Active transaction ULID — when set, all ChangeEntries get this stamped.
    current_transaction: RwLock<Option<TransactionId>>,
    /// Maps transaction ULID to the ordered list of ChangeEntry ULIDs it contains.
    transaction_index: RwLock<HashMap<TransactionId, Vec<ChangeEntryId>>>,
    /// YAML-persisted undo/redo stack, loaded from `root/undo_stack.yaml`.
    undo_stack: RwLock<UndoStack>,
}

impl EntityContext {
    /// Create a new EntityContext.
    ///
    /// - `root`: the storage root (e.g. `.kanban/`)
    /// - `fields`: the field registry containing EntityDefs
    pub fn new(root: impl Into<PathBuf>, fields: Arc<FieldsContext>) -> Self {
        let root = root.into();
        let undo_stack = UndoStack::load(&root.join("undo_stack.yaml"));
        Self {
            root,
            fields,
            validation: None,
            compute: None,
            changelog_index: RwLock::new(HashMap::new()),
            current_transaction: RwLock::new(None),
            transaction_index: RwLock::new(HashMap::new()),
            undo_stack: RwLock::new(undo_stack),
        }
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

    /// Get the storage root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the FieldsContext.
    pub fn fields(&self) -> &FieldsContext {
        &self.fields
    }

    /// Get a read lock on the undo stack.
    pub async fn undo_stack(&self) -> tokio::sync::RwLockReadGuard<'_, UndoStack> {
        self.undo_stack.read().await
    }

    /// Get a write lock on the undo stack.
    pub async fn undo_stack_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, UndoStack> {
        self.undo_stack.write().await
    }

    /// Save the undo stack to disk at `root/undo_stack.yaml`.
    ///
    /// Returns an `EntityError` if the write fails.
    pub async fn save_undo_stack(&self) -> Result<()> {
        let stack = self.undo_stack.read().await;
        let path = self.root.join("undo_stack.yaml");
        Ok(stack.save(&path)?)
    }

    /// Generate a new transaction ULID.
    ///
    /// This is a static helper — it does not set the transaction on the context.
    /// Use [`set_transaction`] to activate it.
    pub fn generate_transaction_id() -> TransactionId {
        TransactionId::new()
    }

    /// Set the active transaction ID.
    ///
    /// All subsequent `write()` and `delete()` calls will stamp this
    /// transaction ID on their ChangeEntry and register the entry ULID
    /// in the transaction index.
    pub async fn set_transaction(&self, tx_id: impl Into<TransactionId>) {
        *self.current_transaction.write().await = Some(tx_id.into());
    }

    /// Clear the active transaction ID.
    ///
    /// Subsequent `write()` and `delete()` calls will not stamp a transaction ID.
    pub async fn clear_transaction(&self) {
        *self.current_transaction.write().await = None;
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
    pub async fn write(&self, entity: &Entity) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(&entity.entity_type)?;
        let dir = self.entity_dir(&entity.entity_type);

        // Apply validation and strip computed fields
        let mut entity = entity.clone();
        let entity_type = entity.entity_type.clone();
        let entity_id = entity.id.clone();
        self.apply_validation(&entity_type, &mut entity).await?;

        let path = io::entity_file_path(&dir, &entity.id, def);

        // Read previous state for diffing (if it exists)
        let previous = io::read_entity(&path, &entity.entity_type, &entity.id, def)
            .await
            .ok();

        // Write the entity
        io::write_entity(&path, &entity, def).await?;

        // Compute and append changelog
        let changes = match &previous {
            Some(old) => changelog::diff_entities(old, &entity),
            None => {
                // Creation — all fields are Set
                let mut changes: Vec<_> = entity
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
                    .collect();
                changes.sort_by(|a, b| a.0.cmp(&b.0));
                changes
            }
        };

        if !changes.is_empty() {
            let op = if previous.is_some() {
                "update"
            } else {
                "create"
            };
            let mut entry = ChangeEntry::new(entity_type.clone(), entity_id.clone(), op, changes);

            // Stamp transaction ID if one is active
            let tx_id = self.current_transaction.read().await.clone();
            if let Some(ref tx) = tx_id {
                entry = entry.with_transaction_id(tx.clone());
            }

            let log_path = path.with_extension("jsonl");
            changelog::append_changelog(&log_path, &entry).await?;

            let ulid = entry.id.clone();
            self.changelog_index
                .write()
                .await
                .insert(ulid.clone(), (entity_type, entity_id));

            // Register in transaction index if applicable
            if let Some(ref tx) = tx_id {
                self.transaction_index
                    .write()
                    .await
                    .entry(tx.clone())
                    .or_default()
                    .push(ulid.clone());
            }

            return Ok(Some(ulid));
        }

        Ok(None)
    }

    /// Delete an entity by type and ID.
    ///
    /// Logs a "delete" changelog entry with all fields as `Removed`,
    /// then moves the data file and changelog to the trash directory
    /// (`{root}/{type}s/.trash/`). The entity is no longer listed or
    /// readable, but its files are preserved for recovery.
    ///
    /// Returns `Ok(Some(ulid))` when a delete changelog entry was logged,
    /// or `Ok(None)` if the entity had no fields to record.
    pub async fn delete(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Option<ChangeEntryId>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);

        let mut result_ulid = None;

        // Read current state to log deletion
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
                let mut entry = ChangeEntry::new(entity_type, id, "delete", changes);

                // Stamp transaction ID if one is active
                let tx_id = self.current_transaction.read().await.clone();
                if let Some(ref tx) = tx_id {
                    entry = entry.with_transaction_id(tx.clone());
                }

                let log_path = path.with_extension("jsonl");
                changelog::append_changelog(&log_path, &entry).await?;

                let ulid = entry.id.clone();
                self.changelog_index.write().await.insert(
                    ulid.clone(),
                    (EntityTypeName::from(entity_type), EntityId::from(id)),
                );

                // Register in transaction index if applicable
                if let Some(ref tx) = tx_id {
                    self.transaction_index
                        .write()
                        .await
                        .entry(tx.clone())
                        .or_default()
                        .push(ulid.clone());
                }

                result_ulid = Some(ulid);
            }
        }

        let trash = self.trash_dir(entity_type);
        io::trash_entity_files(&path, &trash).await?;
        Ok(result_ulid)
    }

    /// Undo a specific changelog operation by its ULID.
    ///
    /// Looks up the changelog entry, reverses its changes, and applies them.
    /// For "update" ops, the reversed changes are applied to the current entity
    /// and a new "undo" changelog entry is appended. For "create" ops, the entity
    /// is deleted (moved to trash). For "delete" ops, the entity is restored from
    /// trash.
    ///
    /// Returns `Ok(Some(ulid))` with the ULID of the new undo changelog entry,
    /// or `Ok(None)` if nothing was undone.
    ///
    /// Returns an error if the ULID is not found in the changelog index, if
    /// the changelog entry cannot be found, or if a text diff cannot be applied
    /// (stale entity).
    pub async fn undo(&self, ulid: impl AsRef<str>) -> Result<Option<ChangeEntryId>> {
        let ulid = ulid.as_ref();
        // 1. Check if it's a single-entity changelog entry.
        //    Clone the result and drop the read guard before calling undo_single,
        //    which needs write access to changelog_index.
        let single_lookup = self.changelog_index.read().await.get(ulid).cloned();
        if let Some((entity_type, entity_id)) = single_lookup {
            return self
                .undo_single(ulid, entity_type.as_str(), entity_id.as_str())
                .await;
        }

        // 2. Check if it's a transaction (group of entries).
        //    Same pattern: clone and drop the read guard before calling undo_transaction.
        let tx_lookup = self.transaction_index.read().await.get(ulid).cloned();
        if let Some(entry_ulids) = tx_lookup {
            return self.undo_transaction(ulid, &entry_ulids).await;
        }

        // 3. Not found in either index
        Err(EntityError::ChangelogEntryNotFound {
            ulid: ulid.to_string(),
        })
    }

    /// Undo a single changelog entry by its ULID.
    async fn undo_single(
        &self,
        ulid: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<ChangeEntryId>> {
        // Read the changelog (with trash fallback so deleted entities work)
        let entries = self
            .read_changelog_with_trash_fallback(entity_type, entity_id)
            .await?;

        // Find the ChangeEntry with matching id
        let original_entry = entries
            .iter()
            .find(|e| e.id == ulid)
            .ok_or_else(|| EntityError::ChangelogEntryNotFound {
                ulid: ulid.to_string(),
            })?
            .clone();

        match original_entry.op.as_str() {
            "update" => {
                self.undo_update(entity_type, entity_id, &original_entry)
                    .await
            }
            "create" => {
                self.undo_create(entity_type, entity_id, &original_entry)
                    .await
            }
            "delete" => {
                self.undo_delete(entity_type, entity_id, &original_entry)
                    .await
            }
            "archive" => {
                self.undo_archive(entity_type, entity_id, &original_entry)
                    .await
            }
            "unarchive" => {
                self.undo_unarchive(entity_type, entity_id, &original_entry)
                    .await
            }
            other => Err(EntityError::UnsupportedUndoOp {
                op: other.to_string(),
            }),
        }
    }

    /// Undo an entire transaction by undoing each constituent entry in reverse order.
    ///
    /// Returns `Ok(Some(tx_ulid))` where `tx_ulid` is the original transaction ULID,
    /// to be used for redo.
    ///
    /// If an undo fails midway, attempts to roll back already-undone entries by
    /// redoing them in forward order. Returns `TransactionPartialFailure` with
    /// details about the failure and whether rollback succeeded.
    async fn undo_transaction(
        &self,
        tx_ulid: &str,
        entry_ulids: &[ChangeEntryId],
    ) -> Result<Option<ChangeEntryId>> {
        let mut completed: Vec<String> = Vec::new();

        // Undo in reverse order so later writes are reversed before earlier ones
        for ulid in entry_ulids.iter().rev() {
            // Clone and drop the read guard before calling undo_single
            let lookup = self
                .changelog_index
                .read()
                .await
                .get(ulid.as_str())
                .cloned();
            let (entity_type, entity_id) =
                lookup.ok_or_else(|| EntityError::ChangelogEntryNotFound {
                    ulid: ulid.to_string(),
                })?;

            match self
                .undo_single(ulid.as_str(), entity_type.as_str(), entity_id.as_str())
                .await
            {
                Ok(_) => {
                    completed.push(ulid.to_string());
                }
                Err(e) => {
                    // Attempt rollback: redo each completed entry in forward order
                    // (reverse of the order they were undone) to restore consistency
                    let mut rollback_succeeded = true;
                    for done_ulid in completed.iter().rev() {
                        let rb_lookup = self
                            .changelog_index
                            .read()
                            .await
                            .get(done_ulid.as_str())
                            .cloned();
                        if let Some((rb_type, rb_id)) = rb_lookup {
                            if self
                                .redo_single(done_ulid, rb_type.as_str(), rb_id.as_str())
                                .await
                                .is_err()
                            {
                                rollback_succeeded = false;
                                break;
                            }
                        } else {
                            rollback_succeeded = false;
                            break;
                        }
                    }

                    return Err(EntityError::TransactionPartialFailure {
                        original_error: e.to_string(),
                        completed,
                        failed_entry: ulid.to_string(),
                        rollback_succeeded,
                    });
                }
            }
        }
        Ok(Some(ChangeEntryId::from(tx_ulid)))
    }

    /// Undo an "update" operation by reversing its field changes.
    ///
    /// Reads the current entity, applies reversed changes, writes the entity
    /// file directly (bypassing `self.write()` to avoid double-logging), and
    /// appends an "undo" changelog entry with the reversed changes.
    async fn undo_update(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read current entity state
        let mut entity = io::read_entity(&path, entity_type, entity_id, def).await?;

        // Compute reversed changes and apply them
        let reversed = changelog::reverse_changes(&original_entry.changes);
        changelog::apply_changes(&mut entity, &reversed)?;

        // Write entity file directly (not through self.write())
        io::write_entity(&path, &entity, def).await?;

        // Create and append the undo changelog entry
        let undo_entry = ChangeEntry::new(entity_type, entity_id, "undo", reversed)
            .with_undone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &undo_entry).await?;

        let undo_ulid = undo_entry.id.clone();
        self.changelog_index.write().await.insert(
            undo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        Ok(Some(undo_ulid))
    }

    /// Undo a "create" operation by deleting (trashing) the entity.
    ///
    /// Appends an "undo" changelog entry referencing the original create,
    /// then moves the entity files to trash.
    async fn undo_create(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read current state to record the removal
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
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

        // Append undo changelog entry before trashing (so it goes with the files)
        let undo_entry = ChangeEntry::new(entity_type, entity_id, "undo", changes)
            .with_undone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &undo_entry).await?;

        let undo_ulid = undo_entry.id.clone();
        self.changelog_index.write().await.insert(
            undo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        // Move files to trash
        let trash = self.trash_dir(entity_type);
        io::trash_entity_files(&path, &trash).await?;

        Ok(Some(undo_ulid))
    }

    /// Undo a "delete" operation by restoring the entity from trash.
    ///
    /// Restores the entity files from trash back to live storage, then appends
    /// an "undo" changelog entry referencing the original delete.
    async fn undo_delete(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        // Restore files from trash to live storage
        self.restore_from_trash(entity_type, entity_id).await?;

        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read the restored entity to record the restoration as Set changes
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
            .collect();
        changes.sort_by(|a, b| a.0.cmp(&b.0));

        // Append undo entry to the restored changelog
        let undo_entry = ChangeEntry::new(entity_type, entity_id, "undo", changes)
            .with_undone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &undo_entry).await?;

        let undo_ulid = undo_entry.id.clone();
        self.changelog_index.write().await.insert(
            undo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        Ok(Some(undo_ulid))
    }

    /// Undo an "archive" operation by restoring the entity from the archive.
    ///
    /// Restores the entity files from the archive directory back to live storage,
    /// then appends an "undo" changelog entry referencing the original archive.
    /// This is structurally identical to `undo_delete()` but targets `.archive/`
    /// instead of `.trash/`.
    async fn undo_archive(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        // Restore files from archive to live storage
        self.restore_from_archive(entity_type, entity_id).await?;

        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read the restored entity to record the restoration as Set changes
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
            .collect();
        changes.sort_by(|a, b| a.0.cmp(&b.0));

        // Append undo entry to the restored changelog
        let undo_entry = ChangeEntry::new(entity_type, entity_id, "undo", changes)
            .with_undone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &undo_entry).await?;

        let undo_ulid = undo_entry.id.clone();
        self.changelog_index.write().await.insert(
            undo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        Ok(Some(undo_ulid))
    }

    /// Undo an "unarchive" operation by moving the entity back to the archive.
    ///
    /// Appends an "undo" changelog entry referencing the original unarchive,
    /// then moves the entity files back to the archive directory. This is
    /// structurally identical to `undo_create()` but targets `.archive/`
    /// instead of `.trash/`.
    async fn undo_unarchive(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read current state to record the removal
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
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

        // Append undo changelog entry before archiving (so it goes with the files)
        let undo_entry = ChangeEntry::new(entity_type, entity_id, "undo", changes)
            .with_undone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &undo_entry).await?;

        let undo_ulid = undo_entry.id.clone();
        self.changelog_index.write().await.insert(
            undo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        // Move files to archive
        let archive = self.archive_dir(entity_type);
        io::trash_entity_files(&path, &archive).await?;

        Ok(Some(undo_ulid))
    }

    /// Redo an "archive" operation by moving the entity back to the archive.
    ///
    /// The entity was originally archived, then undo restored it. Redo archives
    /// it again (same as undo-of-unarchive), reading the current entity to build
    /// Removed changes and appending a "redo" changelog entry before archiving.
    async fn redo_archive(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read current state to record the archival
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
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

        // Append redo changelog entry before archiving (so it goes with the files)
        let redo_entry = ChangeEntry::new(entity_type, entity_id, "redo", changes)
            .with_redone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &redo_entry).await?;

        let redo_ulid = redo_entry.id.clone();
        self.changelog_index.write().await.insert(
            redo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        // Move files to archive
        let archive = self.archive_dir(entity_type);
        io::trash_entity_files(&path, &archive).await?;

        Ok(Some(redo_ulid))
    }

    /// Redo an "unarchive" operation by restoring the entity from the archive.
    ///
    /// The entity was originally unarchived, then undo re-archived it. Redo
    /// restores it from the archive (same as undo-of-archive), reads the
    /// restored entity to build Set changes, and appends a "redo" changelog entry.
    async fn redo_unarchive(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        // Restore files from archive to live storage
        self.restore_from_archive(entity_type, entity_id).await?;

        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read the restored entity to record the restoration as Set changes
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
            .collect();
        changes.sort_by(|a, b| a.0.cmp(&b.0));

        // Append redo entry to the restored changelog
        let redo_entry = ChangeEntry::new(entity_type, entity_id, "redo", changes)
            .with_redone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &redo_entry).await?;

        let redo_ulid = redo_entry.id.clone();
        self.changelog_index.write().await.insert(
            redo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        Ok(Some(redo_ulid))
    }

    /// Redo a previously undone changelog operation by its original ULID.
    ///
    /// Re-applies the forward changes from the original entry. For "update"
    /// ops the original forward changes are applied directly. For "create" ops
    /// the entity is restored from trash (since undo trashed it). For "delete"
    /// ops the entity is trashed again (since undo restored it).
    ///
    /// Returns `Ok(Some(ulid))` with the ULID of the new redo changelog entry,
    /// or `Ok(None)` if nothing was redone.
    ///
    /// Returns an error if the ULID is not found in the changelog index, if
    /// the changelog entry cannot be found, or if a text diff cannot be applied
    /// (stale entity).
    pub async fn redo(&self, ulid: impl AsRef<str>) -> Result<Option<ChangeEntryId>> {
        let ulid = ulid.as_ref();
        // 1. Check if it's a single-entity changelog entry.
        //    Clone the result and drop the read guard before calling redo_single,
        //    which needs write access to changelog_index.
        let single_lookup = self.changelog_index.read().await.get(ulid).cloned();
        if let Some((entity_type, entity_id)) = single_lookup {
            return self
                .redo_single(ulid, entity_type.as_str(), entity_id.as_str())
                .await;
        }

        // 2. Check if it's a transaction (group of entries).
        //    Same pattern: clone and drop the read guard before calling redo_transaction.
        let tx_lookup = self.transaction_index.read().await.get(ulid).cloned();
        if let Some(entry_ulids) = tx_lookup {
            return self.redo_transaction(ulid, &entry_ulids).await;
        }

        // 3. Not found in either index
        Err(EntityError::ChangelogEntryNotFound {
            ulid: ulid.to_string(),
        })
    }

    /// Redo a single changelog entry by its ULID.
    async fn redo_single(
        &self,
        ulid: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<ChangeEntryId>> {
        // Read the changelog (with trash fallback so deleted entities work)
        let entries = self
            .read_changelog_with_trash_fallback(entity_type, entity_id)
            .await?;

        // Find the ChangeEntry with matching id
        let original_entry = entries
            .iter()
            .find(|e| e.id == ulid)
            .ok_or_else(|| EntityError::ChangelogEntryNotFound {
                ulid: ulid.to_string(),
            })?
            .clone();

        match original_entry.op.as_str() {
            "update" => {
                self.redo_update(entity_type, entity_id, &original_entry)
                    .await
            }
            "create" => {
                self.redo_create(entity_type, entity_id, &original_entry)
                    .await
            }
            "delete" => {
                self.redo_delete(entity_type, entity_id, &original_entry)
                    .await
            }
            "archive" => {
                self.redo_archive(entity_type, entity_id, &original_entry)
                    .await
            }
            "unarchive" => {
                self.redo_unarchive(entity_type, entity_id, &original_entry)
                    .await
            }
            other => Err(EntityError::UnsupportedUndoOp {
                op: other.to_string(),
            }),
        }
    }

    /// Redo an entire transaction by redoing each constituent entry in forward order.
    ///
    /// Returns `Ok(Some(tx_ulid))` where `tx_ulid` is the original transaction ULID.
    ///
    /// If a redo fails midway, attempts to roll back already-redone entries by
    /// undoing them in reverse order. Returns `TransactionPartialFailure` with
    /// details about the failure and whether rollback succeeded.
    async fn redo_transaction(
        &self,
        tx_ulid: &str,
        entry_ulids: &[ChangeEntryId],
    ) -> Result<Option<ChangeEntryId>> {
        let mut completed: Vec<String> = Vec::new();

        // Redo in forward order (same order they were originally executed)
        for ulid in entry_ulids.iter() {
            // Clone and drop the read guard before calling redo_single
            let lookup = self
                .changelog_index
                .read()
                .await
                .get(ulid.as_str())
                .cloned();
            let (entity_type, entity_id) =
                lookup.ok_or_else(|| EntityError::ChangelogEntryNotFound {
                    ulid: ulid.to_string(),
                })?;

            match self
                .redo_single(ulid.as_str(), entity_type.as_str(), entity_id.as_str())
                .await
            {
                Ok(_) => {
                    completed.push(ulid.to_string());
                }
                Err(e) => {
                    // Attempt rollback: undo each completed entry in reverse order
                    // to restore the pre-redo state
                    let mut rollback_succeeded = true;
                    for done_ulid in completed.iter().rev() {
                        let rb_lookup = self
                            .changelog_index
                            .read()
                            .await
                            .get(done_ulid.as_str())
                            .cloned();
                        if let Some((rb_type, rb_id)) = rb_lookup {
                            if self
                                .undo_single(done_ulid, rb_type.as_str(), rb_id.as_str())
                                .await
                                .is_err()
                            {
                                rollback_succeeded = false;
                                break;
                            }
                        } else {
                            rollback_succeeded = false;
                            break;
                        }
                    }

                    return Err(EntityError::TransactionPartialFailure {
                        original_error: e.to_string(),
                        completed,
                        failed_entry: ulid.to_string(),
                        rollback_succeeded,
                    });
                }
            }
        }
        Ok(Some(ChangeEntryId::from(tx_ulid)))
    }

    /// Redo an "update" operation by re-applying its forward field changes.
    ///
    /// Reads the current entity, applies the original forward changes (not
    /// reversed — this is the key difference from undo), writes the entity
    /// file directly, and appends a "redo" changelog entry.
    async fn redo_update(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read current entity state
        let mut entity = io::read_entity(&path, entity_type, entity_id, def).await?;

        // Apply the original forward changes directly
        changelog::apply_changes(&mut entity, &original_entry.changes)?;

        // Write entity file directly (not through self.write())
        io::write_entity(&path, &entity, def).await?;

        // Create and append the redo changelog entry
        let redo_entry = ChangeEntry::new(
            entity_type,
            entity_id,
            "redo",
            original_entry.changes.clone(),
        )
        .with_redone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &redo_entry).await?;

        let redo_ulid = redo_entry.id.clone();
        self.changelog_index.write().await.insert(
            redo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        Ok(Some(redo_ulid))
    }

    /// Redo a "create" operation by restoring the entity from trash.
    ///
    /// The entity was originally created, then undo trashed it. Redo restores
    /// it from trash (same as undo-of-delete), reads the restored entity to
    /// build Set changes, and appends a "redo" changelog entry.
    async fn redo_create(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        // Restore files from trash to live storage
        self.restore_from_trash(entity_type, entity_id).await?;

        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read the restored entity to record the restoration as Set changes
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
            .collect();
        changes.sort_by(|a, b| a.0.cmp(&b.0));

        // Append redo entry to the restored changelog
        let redo_entry = ChangeEntry::new(entity_type, entity_id, "redo", changes)
            .with_redone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &redo_entry).await?;

        let redo_ulid = redo_entry.id.clone();
        self.changelog_index.write().await.insert(
            redo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        Ok(Some(redo_ulid))
    }

    /// Redo a "delete" operation by trashing the entity again.
    ///
    /// The entity was originally deleted, then undo restored it. Redo trashes
    /// it again (same as undo-of-create), reading the current entity to build
    /// Removed changes and appending a "redo" changelog entry before trashing.
    async fn redo_delete(
        &self,
        entity_type: &str,
        entity_id: &str,
        original_entry: &ChangeEntry,
    ) -> Result<Option<ChangeEntryId>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, entity_id, def);

        // Read current state to record the removal
        let entity = io::read_entity(&path, entity_type, entity_id, def).await?;
        let mut changes: Vec<_> = entity
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

        // Append redo changelog entry before trashing (so it goes with the files)
        let redo_entry = ChangeEntry::new(entity_type, entity_id, "redo", changes)
            .with_redone_id(original_entry.id.clone());
        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &redo_entry).await?;

        let redo_ulid = redo_entry.id.clone();
        self.changelog_index.write().await.insert(
            redo_ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(entity_id)),
        );

        // Move files to trash
        let trash = self.trash_dir(entity_type);
        io::trash_entity_files(&path, &trash).await?;

        Ok(Some(redo_ulid))
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
    /// Reads the entity, appends an "archive" changelog entry, then moves the
    /// data file and changelog to the archive directory (`{root}/{type}s/.archive/`).
    /// Archived entities no longer appear in `list()` but remain accessible via
    /// `list_archived()` and `read_archived()`.
    ///
    /// Returns `Ok(Some(ulid))` when an archive changelog entry was logged,
    /// or `Ok(None)` if the entity had no fields to record.
    pub async fn archive(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Option<ChangeEntryId>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);

        let mut result_ulid = None;

        // Read current state to log archival
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
                let mut entry = ChangeEntry::new(entity_type, id, "archive", changes);

                // Stamp transaction ID if one is active
                let tx_id = self.current_transaction.read().await.clone();
                if let Some(ref tx) = tx_id {
                    entry = entry.with_transaction_id(tx.clone());
                }

                let log_path = path.with_extension("jsonl");
                changelog::append_changelog(&log_path, &entry).await?;

                let ulid = entry.id.clone();
                self.changelog_index.write().await.insert(
                    ulid.clone(),
                    (EntityTypeName::from(entity_type), EntityId::from(id)),
                );

                // Register in transaction index if applicable
                if let Some(ref tx) = tx_id {
                    self.transaction_index
                        .write()
                        .await
                        .entry(tx.clone())
                        .or_default()
                        .push(ulid.clone());
                }

                result_ulid = Some(ulid);
            }
        }

        let archive = self.archive_dir(entity_type);
        io::trash_entity_files(&path, &archive).await?;
        Ok(result_ulid)
    }

    /// Restore an entity from the archive back to live storage.
    ///
    /// Moves the entity data file and changelog from the archive directory
    /// (`{root}/{type}s/.archive/`) back to the live storage directory, then
    /// appends an "unarchive" changelog entry. The entity reappears in `list()`.
    ///
    /// Returns `Ok(Some(ulid))` when an unarchive changelog entry was logged,
    /// or `Ok(None)` if the entity had no fields to record after restoration.
    pub async fn unarchive(
        &self,
        entity_type: impl AsRef<str>,
        id: impl AsRef<str>,
    ) -> Result<Option<ChangeEntryId>> {
        let entity_type = entity_type.as_ref();
        let id = id.as_ref();
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);
        let archive = self.archive_dir(entity_type);

        // Restore files from archive to live storage
        io::restore_entity_files(&path, &archive).await?;

        // Read the restored entity to record the restoration as Set changes
        let entity = io::read_entity(&path, entity_type, id, def).await?;
        let mut changes: Vec<_> = entity
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
            .collect();
        changes.sort_by(|a, b| a.0.cmp(&b.0));

        if changes.is_empty() {
            return Ok(None);
        }

        // Append unarchive entry to the restored changelog
        let mut entry = ChangeEntry::new(entity_type, id, "unarchive", changes);

        // Stamp transaction ID if one is active
        let tx_id = self.current_transaction.read().await.clone();
        if let Some(ref tx) = tx_id {
            entry = entry.with_transaction_id(tx.clone());
        }

        let log_path = path.with_extension("jsonl");
        changelog::append_changelog(&log_path, &entry).await?;

        let ulid = entry.id.clone();
        self.changelog_index.write().await.insert(
            ulid.clone(),
            (EntityTypeName::from(entity_type), EntityId::from(id)),
        );

        // Register in transaction index if applicable
        if let Some(ref tx) = tx_id {
            self.transaction_index
                .write()
                .await
                .entry(tx.clone())
                .or_default()
                .push(ulid.clone());
        }

        Ok(Some(ulid))
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

    /// Look up which entity a changelog entry belongs to by its ULID.
    ///
    /// Returns `Some((entity_type, entity_id))` if the ULID is in the in-memory index,
    /// or `None` if not found. The index is populated by `write()` and `delete()` calls
    /// during this context's lifetime.
    pub async fn lookup_changelog_entry(
        &self,
        ulid: impl AsRef<str>,
    ) -> Option<(EntityTypeName, EntityId)> {
        self.changelog_index
            .read()
            .await
            .get(ulid.as_ref())
            .cloned()
    }

    // =========================================================================
    // Internal: validation and computation
    // =========================================================================

    /// Validate fields on write and strip computed fields.
    ///
    /// For each field defined on the entity type:
    /// - Skip `Computed` fields (remove from entity — they are derived on read).
    /// - If a validation engine is present, validate and possibly transform the value.
    /// - If a field has a default and is missing from the entity, insert the default.
    async fn apply_validation(
        &self,
        entity_type: impl AsRef<str>,
        entity: &mut Entity,
    ) -> Result<()> {
        let entity_type = entity_type.as_ref();
        let field_defs = self.fields.fields_for_entity(entity_type);
        if field_defs.is_empty() {
            return Ok(());
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

        // Validate fields
        let Some(ref engine) = self.validation else {
            return Ok(());
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
        let entity_def = self.entity_def(entity_type)?;
        engine
            .validate_entity(entity_def, &mut entity.fields)
            .await
            .map_err(|e| EntityError::ValidationFailed {
                field: format!("entity:{}", entity_type),
                message: e.to_string(),
            })?;

        Ok(())
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
    async fn apply_compute(&self, entity_type: &str, entity: &mut Entity) -> Result<()> {
        if self.compute.is_none() {
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
        assert!(trash_dir.join("bug.jsonl").exists());

        // Changelog in trash includes the delete entry
        let log_content = tokio::fs::read_to_string(trash_dir.join("bug.jsonl"))
            .await
            .unwrap();
        assert!(log_content.contains("\"delete\""));
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
    async fn write_creates_changelog_on_create() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].op, "create");
        assert!(log[0]
            .changes
            .iter()
            .all(|(_, c)| matches!(c, FieldChange::Set { .. })));
    }

    #[tokio::test]
    async fn write_creates_changelog_on_update() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Update
        tag.set("tag_name", json!("Bug Report"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].op, "create");
        assert_eq!(log[1].op, "update");
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
    async fn test_undo_update() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Update it
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Verify the update took effect
        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug Report"));

        // Undo the update
        ctx.undo(&update_ulid).await.unwrap();

        // Verify the field is restored to the original value
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    async fn test_undo_create() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Verify it exists
        assert!(ctx.read("tag", "bug").await.is_ok());

        // Undo the create
        ctx.undo(&create_ulid).await.unwrap();

        // Verify the entity is gone (in trash)
        assert!(ctx.read("tag", "bug").await.is_err());

        // Verify files are in trash (new layout: {type}s/.trash/)
        let trash_dir = dir.path().join("tags").join(".trash");
        assert!(trash_dir.join("bug.yaml").exists());
    }

    #[tokio::test]
    async fn test_undo_delete() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Delete it
        let delete_ulid = ctx.delete("tag", "bug").await.unwrap().unwrap();

        // Verify it's gone
        assert!(ctx.read("tag", "bug").await.is_err());

        // Undo the delete
        ctx.undo(&delete_ulid).await.unwrap();

        // Verify the entity is back
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
        assert_eq!(restored.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn test_undo_returns_ulid() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create and update
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Undo returns a new ULID
        let undo_result = ctx.undo(&update_ulid).await.unwrap();
        assert!(undo_result.is_some());

        let undo_ulid = undo_result.unwrap();
        // The undo ULID should be different from the original update ULID
        assert_ne!(undo_ulid, update_ulid);
        // It should be a valid ULID (26 chars)
        assert_eq!(undo_ulid.len(), 26);
    }

    #[tokio::test]
    async fn test_undo_stale_update_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Update it (change tag_name from "Bug" to "Bug Report")
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Update it again (change tag_name from "Bug Report" to "Something Else")
        tag.set("tag_name", json!("Something Else"));
        ctx.write(&tag).await.unwrap();

        // Now try to undo the first update — the undo expects to see "Bug Report"
        // but the current value is "Something Else", so reverse-applying the
        // TextDiff should fail because the text has been modified.
        let result = ctx.undo(&update_ulid).await;
        assert!(result.is_err(), "undoing a stale update should error");
    }

    #[tokio::test]
    async fn test_undo_unknown_ulid_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Try to undo a ULID that doesn't exist
        let result = ctx.undo("01NONEXISTENT000000000000").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn test_undo_changelog_has_undone_id() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create and update
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Undo the update
        let undo_ulid = ctx.undo(&update_ulid).await.unwrap().unwrap();

        // Read the changelog and find the undo entry
        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        let undo_entry = log.iter().find(|e| e.id == undo_ulid).unwrap();

        // Verify the undo entry has the correct fields
        assert_eq!(undo_entry.op, "undo");
        assert_eq!(undo_entry.undone_id.as_deref(), Some(update_ulid.as_str()));
        assert_eq!(undo_entry.entity_type, "tag");
        assert_eq!(undo_entry.entity_id, "bug");
    }

    #[tokio::test]
    async fn test_redo_update() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Update it
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Verify the update took effect
        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug Report"));

        // Undo the update
        ctx.undo(&update_ulid).await.unwrap();

        // Verify undo restored the original value
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));

        // Redo the update
        ctx.redo(&update_ulid).await.unwrap();

        // Verify the field has the updated value again
        let redone = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(redone.get_str("tag_name"), Some("Bug Report"));
    }

    #[tokio::test]
    async fn test_redo_create() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Verify it exists
        assert!(ctx.read("tag", "bug").await.is_ok());

        // Undo the create (trashes it)
        ctx.undo(&create_ulid).await.unwrap();

        // Verify the entity is gone
        assert!(ctx.read("tag", "bug").await.is_err());

        // Redo the create (restores it from trash)
        ctx.redo(&create_ulid).await.unwrap();

        // Verify the entity is back
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
        assert_eq!(restored.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn test_redo_delete() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Delete it
        let delete_ulid = ctx.delete("tag", "bug").await.unwrap().unwrap();

        // Verify it's gone
        assert!(ctx.read("tag", "bug").await.is_err());

        // Undo the delete (restores it)
        ctx.undo(&delete_ulid).await.unwrap();

        // Verify it's back
        assert!(ctx.read("tag", "bug").await.is_ok());

        // Redo the delete (trashes it again)
        ctx.redo(&delete_ulid).await.unwrap();

        // Verify the entity is gone again
        assert!(ctx.read("tag", "bug").await.is_err());

        // Verify files are in trash (new layout: {type}s/.trash/)
        let trash_dir = dir.path().join("tags").join(".trash");
        assert!(trash_dir.join("bug.yaml").exists());
    }

    #[tokio::test]
    async fn test_redo_returns_ulid() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create and update
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Undo the update
        ctx.undo(&update_ulid).await.unwrap();

        // Redo returns a new ULID
        let redo_result = ctx.redo(&update_ulid).await.unwrap();
        assert!(redo_result.is_some());

        let redo_ulid = redo_result.unwrap();
        // The redo ULID should be different from the original update ULID
        assert_ne!(redo_ulid, update_ulid);
        // It should be a valid ULID (26 chars)
        assert_eq!(redo_ulid.len(), 26);
    }

    #[tokio::test]
    async fn test_redo_stale_update_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Update it (change tag_name from "Bug" to "Bug Report")
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Undo the update (back to "Bug")
        ctx.undo(&update_ulid).await.unwrap();

        // Manually modify the entity to something different
        tag.set("tag_name", json!("Something Else"));
        ctx.write(&tag).await.unwrap();

        // Now try to redo the update — the redo expects to see "Bug"
        // but the current value is "Something Else", so applying the
        // TextDiff should fail because the text has been modified.
        let result = ctx.redo(&update_ulid).await;
        assert!(result.is_err(), "redoing a stale update should error");
    }

    #[tokio::test]
    async fn test_redo_unknown_ulid_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Try to redo a ULID that doesn't exist
        let result = ctx.redo("01NONEXISTENT000000000000").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn test_redo_changelog_has_redone_id() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create and update
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Undo the update
        ctx.undo(&update_ulid).await.unwrap();

        // Redo the update
        let redo_ulid = ctx.redo(&update_ulid).await.unwrap().unwrap();

        // Read the changelog and find the redo entry
        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        let redo_entry = log.iter().find(|e| e.id == redo_ulid).unwrap();

        // Verify the redo entry has the correct fields
        assert_eq!(redo_entry.op, "redo");
        assert_eq!(redo_entry.redone_id.as_deref(), Some(update_ulid.as_str()));
        assert_eq!(redo_entry.entity_type, "tag");
        assert_eq!(redo_entry.entity_id, "bug");
    }

    #[tokio::test]
    async fn test_undo_redo_undo_cycle() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag with initial value
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Update it
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Verify: "Bug Report"
        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug Report"));

        // Undo: back to "Bug"
        ctx.undo(&update_ulid).await.unwrap();
        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));

        // Redo: forward to "Bug Report"
        ctx.redo(&update_ulid).await.unwrap();
        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug Report"));

        // Undo again: back to "Bug"
        ctx.undo(&update_ulid).await.unwrap();
        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));

        // Color should remain unchanged throughout all operations
        assert_eq!(loaded.get_str("color"), Some("#ff0000"));
    }

    // =========================================================================
    // New tests for the relocated .trash/ layout and .archive/ support
    // =========================================================================

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
    async fn delete_moves_to_new_trash_location() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        ctx.delete("tag", "bug").await.unwrap();

        // Entity is gone from live storage
        assert!(ctx.read("tag", "bug").await.is_err());

        // Files are in the new trash location: {type}s/.trash/
        let trash_dir = dir.path().join("tags").join(".trash");
        assert!(trash_dir.join("bug.yaml").exists());
        assert!(trash_dir.join("bug.jsonl").exists());

        // Old-style .trash/ at root should NOT exist
        assert!(!dir.path().join(".trash").exists());
    }

    #[tokio::test]
    async fn archive_moves_to_archive_dir() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Entity is visible before archiving
        assert_eq!(ctx.list("tag").await.unwrap().len(), 1);

        let archive_ulid = ctx.archive("tag", "bug").await.unwrap();
        assert!(archive_ulid.is_some());

        // Entity is gone from live storage
        assert!(ctx.read("tag", "bug").await.is_err());

        // Entity excluded from list()
        assert_eq!(ctx.list("tag").await.unwrap().len(), 0);

        // Files are in the archive directory
        let archive_dir = dir.path().join("tags").join(".archive");
        assert!(archive_dir.join("bug.yaml").exists());
        assert!(archive_dir.join("bug.jsonl").exists());
    }

    #[tokio::test]
    async fn unarchive_restores_entity() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Archive the entity
        ctx.archive("tag", "bug").await.unwrap();
        assert_eq!(ctx.list("tag").await.unwrap().len(), 0);

        // Unarchive it
        let unarchive_ulid = ctx.unarchive("tag", "bug").await.unwrap();
        assert!(unarchive_ulid.is_some());

        // Entity is back in live storage
        assert_eq!(ctx.list("tag").await.unwrap().len(), 1);
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
        assert_eq!(restored.get_str("color"), Some("#ff0000"));

        // Archive directory is now empty
        let archive_dir = dir.path().join("tags").join(".archive");
        assert!(!archive_dir.join("bug.yaml").exists());
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
    async fn undo_delete_works_with_new_trash() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Delete it
        let delete_ulid = ctx.delete("tag", "bug").await.unwrap().unwrap();

        // Verify trash location is new-style
        let trash_dir = dir.path().join("tags").join(".trash");
        assert!(trash_dir.join("bug.yaml").exists());

        // Undo the delete — should work with the new trash layout
        ctx.undo(&delete_ulid).await.unwrap();

        // Entity is restored
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
        assert_eq!(restored.get_str("color"), Some("#ff0000"));
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
}
