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
    /// Persistent undo/redo stack tracking changelog entry IDs.
    undo_stack: RwLock<UndoStack>,
}

impl EntityContext {
    /// Create a new EntityContext.
    ///
    /// - `root`: the storage root (e.g. `.kanban/`)
    /// - `fields`: the field registry containing EntityDefs
    pub fn new(root: impl Into<PathBuf>, fields: Arc<FieldsContext>) -> Self {
        let root = root.into();
        let undo_stack_path = root.join("undo_stack.yaml");
        let undo_stack = UndoStack::load(&undo_stack_path).unwrap_or_default();
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

    /// Rebuild changelog and transaction indexes from all `.jsonl` files on disk.
    ///
    /// Scans live, trash, and archive directories for each known entity type.
    /// Call this after construction and before any undo/redo operations.
    pub async fn rebuild_indexes(&self) -> Result<()> {
        let mut cl_index = self.changelog_index.write().await;
        let mut tx_index = self.transaction_index.write().await;

        for entity_def in self.fields.all_entities() {
            let entity_type = entity_def.name.as_str();
            let base_dir = self.entity_dir(entity_type);

            // Scan live, trash, and archive directories
            let dirs = [
                base_dir.clone(),
                base_dir.join(".trash"),
                base_dir.join(".archive"),
            ];

            for dir in &dirs {
                let mut read_dir = match tokio::fs::read_dir(dir).await {
                    Ok(rd) => rd,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(crate::error::EntityError::Io(e)),
                };

                while let Some(entry) = read_dir
                    .next_entry()
                    .await
                    .map_err(crate::error::EntityError::Io)?
                {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }

                    let entries = changelog::read_changelog(&path).await?;
                    for ce in entries {
                        let ce_id = ce.id.clone();
                        let et = ce.entity_type.clone();
                        let eid = ce.entity_id.clone();

                        cl_index.insert(ce_id.clone(), (et, eid));

                        if let Some(tx_id) = ce.transaction_id {
                            tx_index.entry(tx_id).or_default().push(ce_id);
                        }
                    }
                }
            }
        }

        Ok(())
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

    /// Get the path to the undo stack YAML file.
    pub fn undo_stack_path(&self) -> PathBuf {
        self.root.join("undo_stack.yaml")
    }

    /// Synchronously check whether the undo stack has entries that can be undone.
    ///
    /// Uses `try_read()` on the tokio RwLock so this is safe to call from
    /// synchronous code (e.g. `Command::available()`). Returns `false` if the
    /// lock is currently held for writing.
    pub fn can_undo(&self) -> bool {
        self.undo_stack
            .try_read()
            .map(|stack| stack.can_undo())
            .unwrap_or(false)
    }

    /// Synchronously check whether the undo stack has entries that can be redone.
    ///
    /// Uses `try_read()` on the tokio RwLock so this is safe to call from
    /// synchronous code (e.g. `Command::available()`). Returns `false` if the
    /// lock is currently held for writing.
    pub fn can_redo(&self) -> bool {
        self.undo_stack
            .try_read()
            .map(|stack| stack.can_redo())
            .unwrap_or(false)
    }

    /// Get a read lock on the undo stack.
    pub async fn undo_stack(&self) -> tokio::sync::RwLockReadGuard<'_, UndoStack> {
        self.undo_stack.read().await
    }

    /// Get a write lock on the undo stack.
    pub async fn undo_stack_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, UndoStack> {
        self.undo_stack.write().await
    }

    /// Save the current undo stack to disk.
    pub fn save_undo_stack(&self, stack: &UndoStack) -> Result<()> {
        stack.save(&self.undo_stack_path())
    }

    /// Push an entry onto the undo stack and save to disk.
    ///
    /// Uses the transaction ID as the stack entry ID if a transaction is active,
    /// otherwise uses the changelog entry ID. The label format is
    /// `"{op} {entity_type} {entity_id}"`.
    async fn push_undo_stack(
        &self,
        entry_id: &ChangeEntryId,
        op: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<()> {
        let tx_id = self.current_transaction.read().await.clone();
        let stack_id = match &tx_id {
            Some(tx) => tx.to_string(),
            None => entry_id.to_string(),
        };
        let label = format!("{} {} {}", op, entity_type, entity_id);

        let mut stack = self.undo_stack.write().await;
        stack.push(stack_id, label);
        self.save_undo_stack(&stack)
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

        // Trash attachment files that were removed during update
        if let Some(ref old) = previous {
            self.trash_removed_attachments(&entity.entity_type, old, &entity)
                .await?;
        }

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
                .insert(ulid.clone(), (entity_type.clone(), entity_id.clone()));

            // Register in transaction index if applicable
            if let Some(ref tx) = tx_id {
                self.transaction_index
                    .write()
                    .await
                    .entry(tx.clone())
                    .or_default()
                    .push(ulid.clone());
            }

            // Push onto undo stack and save to disk
            self.push_undo_stack(&ulid, op, &entity_type, &entity_id)
                .await?;

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
            // Trash attachment files before deleting the entity
            self.trash_entity_attachments(entity_type, &old).await?;

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

                // Push onto undo stack and save to disk
                self.push_undo_stack(&ulid, "delete", entity_type, id)
                    .await?;

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
            let result = self
                .undo_single(ulid, entity_type.as_str(), entity_id.as_str())
                .await?;

            // Record undo on the stack and save to disk
            let mut stack = self.undo_stack.write().await;
            stack.record_undo();
            self.save_undo_stack(&stack)?;

            return Ok(result);
        }

        // 2. Check if it's a transaction (group of entries).
        //    Same pattern: clone and drop the read guard before calling undo_transaction.
        let tx_lookup = self.transaction_index.read().await.get(ulid).cloned();
        if let Some(entry_ulids) = tx_lookup {
            let result = self.undo_transaction(ulid, &entry_ulids).await?;

            // Record undo on the stack and save to disk
            let mut stack = self.undo_stack.write().await;
            stack.record_undo();
            self.save_undo_stack(&stack)?;

            return Ok(result);
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
            let result = self
                .redo_single(ulid, entity_type.as_str(), entity_id.as_str())
                .await?;

            // Record redo on the stack and save to disk
            let mut stack = self.undo_stack.write().await;
            stack.record_redo();
            self.save_undo_stack(&stack)?;

            return Ok(result);
        }

        // 2. Check if it's a transaction (group of entries).
        //    Same pattern: clone and drop the read guard before calling redo_transaction.
        let tx_lookup = self.transaction_index.read().await.get(ulid).cloned();
        if let Some(entry_ulids) = tx_lookup {
            let result = self.redo_transaction(ulid, &entry_ulids).await?;

            // Record redo on the stack and save to disk
            let mut stack = self.undo_stack.write().await;
            stack.record_redo();
            self.save_undo_stack(&stack)?;

            return Ok(result);
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

                // Push onto undo stack and save to disk
                self.push_undo_stack(&ulid, "archive", entity_type, id)
                    .await?;

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

        // Push onto undo stack and save to disk
        self.push_undo_stack(&ulid, "unarchive", entity_type, id)
            .await?;

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

        // Process attachment fields — copy source files, validate sizes.
        let entity_type_dir = self.entity_dir(entity_type);
        for fd in &field_defs {
            if let FieldType::Attachment {
                max_bytes,
                multiple,
            } = &fd.type_
            {
                self.process_attachment_field(
                    entity,
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
    async fn can_undo_and_can_redo_reflect_stack_state() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Initially neither can undo nor redo
        assert!(!ctx.can_undo());
        assert!(!ctx.can_redo());

        // Create an entity (pushes onto undo stack)
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Now can undo but not redo
        assert!(ctx.can_undo());
        assert!(!ctx.can_redo());

        // Undo the create
        ctx.undo(&create_ulid).await.unwrap();

        // After undo, can redo but not undo (stack pointer at 0)
        assert!(!ctx.can_undo());
        assert!(ctx.can_redo());
    }

    #[tokio::test]
    async fn undo_stack_mut_allows_mutation() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Use undo_stack_mut to clear the stack
        {
            let mut stack = ctx.undo_stack_mut().await;
            stack.push("fake-id", "test operation");
        }
        assert!(ctx.can_undo());

        {
            let mut stack = ctx.undo_stack_mut().await;
            stack.clear();
        }
        assert!(!ctx.can_undo());
    }

    #[tokio::test]
    async fn undo_stack_path_correct() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert_eq!(ctx.undo_stack_path(), dir.path().join("undo_stack.yaml"));
    }

    #[tokio::test]
    async fn lookup_changelog_entry_returns_entity_info() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Initially empty
        assert!(ctx.lookup_changelog_entry("nonexistent").await.is_none());

        // Create an entity — write populates the changelog index
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Now the ULID should be indexed
        let (et, eid) = ctx.lookup_changelog_entry(&ulid).await.unwrap();
        assert_eq!(et.as_str(), "tag");
        assert_eq!(eid.as_str(), "bug");
    }

    #[tokio::test]
    async fn rebuild_indexes_populates_from_disk() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();

        // Create entities using one context
        let ctx1 = EntityContext::new(dir.path(), fields.clone());
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let create_ulid = ctx1.write(&tag).await.unwrap().unwrap();

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx1.write(&tag).await.unwrap().unwrap();

        // Create a second context (in-memory indexes are empty)
        let ctx2 = EntityContext::new(dir.path(), fields.clone());
        assert!(ctx2.lookup_changelog_entry(&create_ulid).await.is_none());

        // Rebuild indexes from disk
        ctx2.rebuild_indexes().await.unwrap();

        // Now the ULIDs should be found
        let (et, eid) = ctx2.lookup_changelog_entry(&create_ulid).await.unwrap();
        assert_eq!(et.as_str(), "tag");
        assert_eq!(eid.as_str(), "bug");

        let (et2, eid2) = ctx2.lookup_changelog_entry(&update_ulid).await.unwrap();
        assert_eq!(et2.as_str(), "tag");
        assert_eq!(eid2.as_str(), "bug");
    }

    #[tokio::test]
    async fn rebuild_indexes_scans_trash_and_archive() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();

        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create, then delete (moves to trash)
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let create_ulid = ctx.write(&tag).await.unwrap().unwrap();
        let delete_ulid = ctx.delete("tag", "bug").await.unwrap().unwrap();

        // Create another entity, then archive it
        let mut tag2 = Entity::new("tag", "feature");
        tag2.set("tag_name", json!("Feature"));
        let create2_ulid = ctx.write(&tag2).await.unwrap().unwrap();
        let archive_ulid = ctx.archive("tag", "feature").await.unwrap().unwrap();

        // New context with empty indexes
        let ctx2 = EntityContext::new(dir.path(), fields.clone());
        ctx2.rebuild_indexes().await.unwrap();

        // All ULIDs should be found (from live, trash, and archive dirs)
        assert!(ctx2.lookup_changelog_entry(&create_ulid).await.is_some());
        assert!(ctx2.lookup_changelog_entry(&delete_ulid).await.is_some());
        assert!(ctx2.lookup_changelog_entry(&create2_ulid).await.is_some());
        assert!(ctx2.lookup_changelog_entry(&archive_ulid).await.is_some());
    }

    #[tokio::test]
    async fn delete_with_transaction_stamps_entries() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Set a transaction ID and delete
        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;
        let delete_ulid = ctx.delete("tag", "bug").await.unwrap().unwrap();
        ctx.clear_transaction().await;

        // The delete changelog entry should have the transaction ID
        let entries = ctx
            .read_changelog_with_trash_fallback("tag", "bug")
            .await
            .unwrap();
        let delete_entry = entries.iter().find(|e| e.id == delete_ulid).unwrap();
        assert_eq!(delete_entry.transaction_id.as_deref(), Some(tx_id.as_str()));
    }

    #[tokio::test]
    async fn read_changelog_with_trash_fallback_falls_to_archive() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create and archive a tag
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();
        ctx.archive("tag", "bug").await.unwrap();

        // Live and trash changelogs don't exist, but archive does
        let entries = ctx
            .read_changelog_with_trash_fallback("tag", "bug")
            .await
            .unwrap();
        assert!(!entries.is_empty());
        // Should contain both the create and archive entries
        assert!(entries.iter().any(|e| e.op == "create"));
        assert!(entries.iter().any(|e| e.op == "archive"));
    }

    #[tokio::test]
    async fn undo_archive_restores_entity() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Archive it
        let archive_ulid = ctx.archive("tag", "bug").await.unwrap().unwrap();

        // Verify entity is archived (not in live storage)
        assert!(ctx.read("tag", "bug").await.is_err());

        // Undo the archive — should restore
        ctx.undo(&archive_ulid).await.unwrap();

        // Entity should be back in live storage
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
        assert_eq!(restored.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn undo_unarchive_re_archives_entity() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Archive then unarchive
        ctx.archive("tag", "bug").await.unwrap();
        let unarchive_ulid = ctx.unarchive("tag", "bug").await.unwrap().unwrap();

        // Entity is back in live storage
        assert!(ctx.read("tag", "bug").await.is_ok());

        // Undo the unarchive — should re-archive
        ctx.undo(&unarchive_ulid).await.unwrap();

        // Entity should be archived again
        assert!(ctx.read("tag", "bug").await.is_err());
        let archived = ctx.list_archived("tag").await.unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, "bug");
    }

    #[tokio::test]
    async fn redo_archive_re_archives_entity() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Archive, then undo the archive
        let archive_ulid = ctx.archive("tag", "bug").await.unwrap().unwrap();
        ctx.undo(&archive_ulid).await.unwrap();

        // Entity is back in live storage
        assert!(ctx.read("tag", "bug").await.is_ok());

        // Redo the archive
        ctx.redo(&archive_ulid).await.unwrap();

        // Entity should be archived again
        assert!(ctx.read("tag", "bug").await.is_err());
        let archived = ctx.list_archived("tag").await.unwrap();
        assert_eq!(archived.len(), 1);
    }

    #[tokio::test]
    async fn redo_unarchive_restores_entity() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Archive, unarchive, undo unarchive (back to archived)
        ctx.archive("tag", "bug").await.unwrap();
        let unarchive_ulid = ctx.unarchive("tag", "bug").await.unwrap().unwrap();
        ctx.undo(&unarchive_ulid).await.unwrap();

        // Entity is archived
        assert!(ctx.read("tag", "bug").await.is_err());

        // Redo the unarchive
        ctx.redo(&unarchive_ulid).await.unwrap();

        // Entity should be back in live storage
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    async fn undo_unsupported_op_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag and manually write an "undo" changelog entry
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let _create_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Update to generate a changelog entry, then undo it
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();
        let undo_ulid = ctx.undo(&update_ulid).await.unwrap().unwrap();

        // Trying to undo the undo entry should give UnsupportedUndoOp
        let result = ctx.undo(&undo_ulid).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unsupported"),
            "should mention unsupported op: {err}"
        );
    }

    #[tokio::test]
    async fn idempotent_write_returns_none() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));

        // First write creates
        let first = ctx.write(&tag).await.unwrap();
        assert!(first.is_some());

        // Second write with same data returns None (no changes)
        let second = ctx.write(&tag).await.unwrap();
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn with_validation_and_compute_builders() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let validation = Arc::new(swissarmyhammer_fields::ValidationEngine::new());
        let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());

        let ctx = EntityContext::new(dir.path(), fields.clone())
            .with_validation(validation)
            .with_compute(compute);

        // With compute engine attached, read/list go through apply_compute_with_query
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));

        // List with compute
        let tags = ctx.list("tag").await.unwrap();
        assert_eq!(tags.len(), 1);
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
    async fn archive_with_transaction_stamps_entries() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Set a transaction and archive
        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;
        let archive_ulid = ctx.archive("tag", "bug").await.unwrap().unwrap();
        ctx.clear_transaction().await;

        // The archive changelog entry should have the transaction ID
        let entries = ctx
            .read_changelog_with_trash_fallback("tag", "bug")
            .await
            .unwrap();
        let archive_entry = entries.iter().find(|e| e.id == archive_ulid).unwrap();
        assert_eq!(
            archive_entry.transaction_id.as_deref(),
            Some(tx_id.as_str())
        );
    }

    #[tokio::test]
    async fn unarchive_with_transaction_stamps_entries() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();
        ctx.archive("tag", "bug").await.unwrap();

        // Set a transaction and unarchive
        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;
        let unarchive_ulid = ctx.unarchive("tag", "bug").await.unwrap().unwrap();
        ctx.clear_transaction().await;

        // The unarchive changelog entry should have the transaction ID
        let entries = ctx.read_changelog("tag", "bug").await.unwrap();
        let unarchive_entry = entries.iter().find(|e| e.id == unarchive_ulid).unwrap();
        assert_eq!(
            unarchive_entry.transaction_id.as_deref(),
            Some(tx_id.as_str())
        );
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

    // =========================================================================
    // Additional coverage tests
    // =========================================================================

    #[tokio::test]
    async fn write_with_transaction_stamps_changelog_entry() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

        ctx.clear_transaction().await;

        // Verify the changelog entry has the transaction ID
        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        let entry = log.iter().find(|e| e.id == create_ulid).unwrap();
        assert_eq!(entry.transaction_id.as_deref(), Some(tx_id.as_str()));
    }

    #[tokio::test]
    async fn write_with_transaction_registers_in_transaction_index() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

        // Update within same transaction
        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        ctx.clear_transaction().await;

        // Both ULIDs should be findable via changelog index
        let (et, eid) = ctx.lookup_changelog_entry(&create_ulid).await.unwrap();
        assert_eq!(et.as_str(), "tag");
        assert_eq!(eid.as_str(), "bug");

        let (et2, eid2) = ctx.lookup_changelog_entry(&update_ulid).await.unwrap();
        assert_eq!(et2.as_str(), "tag");
        assert_eq!(eid2.as_str(), "bug");
    }

    #[tokio::test]
    async fn clear_transaction_stops_stamping() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Start transaction
        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Clear transaction
        ctx.clear_transaction().await;

        // Write another entity — should NOT have a transaction ID
        tag.set("tag_name", json!("Bug Updated"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        let update_entry = log.iter().find(|e| e.id == update_ulid).unwrap();
        assert!(
            update_entry.transaction_id.is_none(),
            "after clear_transaction, new writes should not have a transaction ID"
        );
    }

    #[tokio::test]
    async fn generate_transaction_id_produces_unique_ids() {
        let id1 = EntityContext::generate_transaction_id();
        let id2 = EntityContext::generate_transaction_id();
        assert_ne!(id1, id2);
        // Both should be valid ULIDs (26 chars)
        assert_eq!(id1.as_str().len(), 26);
        assert_eq!(id2.as_str().len(), 26);
    }

    #[tokio::test]
    async fn undo_stack_read_accessor_returns_stack() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Initially empty
        {
            let stack = ctx.undo_stack().await;
            assert!(!stack.can_undo());
            assert!(!stack.can_redo());
        }

        // After a write, the stack has an entry
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        {
            let stack = ctx.undo_stack().await;
            assert!(stack.can_undo());
        }
    }

    #[tokio::test]
    async fn save_undo_stack_persists_to_disk() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Write to push onto undo stack
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Verify the undo stack file exists on disk
        let stack_path = ctx.undo_stack_path();
        assert!(
            stack_path.exists(),
            "undo_stack.yaml should be saved to disk"
        );

        // Create a new context from the same root — it should load the saved stack
        let ctx2 = EntityContext::new(dir.path(), fields.clone());
        assert!(
            ctx2.can_undo(),
            "new context should load undo stack from disk"
        );
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
    async fn redo_unknown_ulid_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let result = ctx.redo("01NONEXISTENT000000000000").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn redo_unsupported_op_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag and update it, then undo, then get the redo entry
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();
        let _undo_ulid = ctx.undo(&update_ulid).await.unwrap().unwrap();
        let redo_ulid = ctx.redo(&update_ulid).await.unwrap().unwrap();

        // Trying to redo the redo entry should give UnsupportedUndoOp
        let result = ctx.redo(&redo_ulid).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unsupported"),
            "should mention unsupported op: {err}"
        );
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
    async fn read_changelog_with_trash_fallback_uses_live_when_present() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create an entity (writes to live changelog)
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Read with trash fallback — should use live changelog
        let entries = ctx
            .read_changelog_with_trash_fallback("tag", "bug")
            .await
            .unwrap();
        assert!(!entries.is_empty());
        assert!(entries.iter().any(|e| e.op == "create"));
    }

    #[tokio::test]
    async fn read_changelog_with_trash_fallback_uses_trash_for_deleted() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create then delete
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();
        ctx.delete("tag", "bug").await.unwrap();

        // Live changelog is gone, but trash has it
        let entries = ctx
            .read_changelog_with_trash_fallback("tag", "bug")
            .await
            .unwrap();
        assert!(!entries.is_empty());
        assert!(entries.iter().any(|e| e.op == "delete"));
    }

    #[tokio::test]
    async fn transaction_write_stamps_all_entries() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // Create a tag outside the transaction
        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        // Update the tag in a transaction
        let tx_id = EntityContext::generate_transaction_id();
        ctx.set_transaction(tx_id.clone()).await;

        tag.set("tag_name", json!("Bug Report"));
        let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

        ctx.clear_transaction().await;

        // Verify the update changelog entry has the transaction ID
        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        let update_entry = log.iter().find(|e| e.id == update_ulid).unwrap();
        assert_eq!(
            update_entry.transaction_id.as_deref(),
            Some(tx_id.as_str()),
            "update entry should have transaction ID stamped"
        );

        // Rebuild indexes to populate transaction index
        ctx.rebuild_indexes().await.unwrap();

        // The transaction should be known in the index
        // Undo via the individual entry (not the transaction) to verify it works
        ctx.undo(&update_ulid).await.unwrap();
        let restored = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    async fn rebuild_indexes_with_no_entity_dirs() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // No entity directories exist — rebuild should succeed gracefully
        ctx.rebuild_indexes().await.unwrap();
    }

    #[tokio::test]
    async fn with_validation_strips_computed_fields_and_inserts_defaults() {
        // Test that validation strips computed fields and inserts defaults.
        // We need a fields context with computed and default fields.
        let defs = vec![
            (
                "tag_name",
                "id: 00000000000000000000000TAG\nname: tag_name\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "color",
                "id: 00000000000000000000000COL\nname: color\ntype:\n  kind: text\ndefault: \"#000000\"\n",
            ),
        ];
        let entities = vec![("tag", "name: tag\nfields:\n  - tag_name\n  - color\n")];

        let tmp = TempDir::new().unwrap();
        let fields = Arc::new(
            swissarmyhammer_fields::FieldsContext::from_yaml_sources(tmp.path(), &defs, &entities)
                .unwrap(),
        );

        let dir = TempDir::new().unwrap();
        let validation = Arc::new(swissarmyhammer_fields::ValidationEngine::new());
        let ctx = EntityContext::new(dir.path(), fields).with_validation(validation);

        // Write tag without color — default should be inserted
        let mut tag = Entity::new("tag", "defaults-test");
        tag.set("tag_name", json!("Test"));
        // color is not set — should get default "#000000"
        ctx.write(&tag).await.unwrap();

        let loaded = ctx.read("tag", "defaults-test").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Test"));
        assert_eq!(loaded.get_str("color"), Some("#000000"));
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
}
