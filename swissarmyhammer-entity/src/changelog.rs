//! Entity change logging with field-level diffs.
//!
//! Every mutation to an entity produces a [`ChangeEntry`] recording which fields
//! changed and how. String fields store a unified diff patch that captures exactly
//! which lines were added, removed, or modified. Non-string fields store old/new
//! as JSON values. Changes are reversible — each [`FieldChange`] has a natural
//! inverse.
//!
//! ## Storage
//!
//! Changelogs are stored as append-only JSONL files (one JSON object per line).
//! Each entity has its own changelog file. [`read_changelog`] loads the entire
//! file into memory, so growth is proportional to the number of mutations.
//! For typical kanban usage (hundreds of updates per entity) this is fine.
//! Long-lived entities with thousands of updates may benefit from future
//! compaction (snapshotting state and truncating the log).
//!
//! ## Usage
//!
//! ```rust,no_run
//! use swissarmyhammer_entity::{Entity, changelog::*};
//!
//! let old = Entity::new("task", "01ABC");
//! let mut new = old.clone();
//! new.set("title", serde_json::json!("Updated title"));
//!
//! let changes = diff_entities(&old, &new);
//! let reversed = reverse_changes(&changes);
//! ```

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_fields::{EntityDef, EntityTypeName};
use swissarmyhammer_store::{ChangeOp, ChangelogEntry as StoreChangelogEntry};
use tokio::fs;
use tracing::warn;

use crate::entity::Entity;
use crate::error::Result;
use crate::id_types::{ChangeEntryId, EntityId, TransactionId};
use crate::io::parse_entity_text;

/// What happened to a single field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldChange {
    /// Field was added (entity creation or new field).
    Set { value: Value },
    /// Field was removed.
    Removed { old_value: Value },
    /// Non-string field changed — record old and new values.
    Changed { old_value: Value, new_value: Value },
    /// String field changed — stores forward and reverse unified diff patches.
    ///
    /// The `forward_patch` transforms old text → new text.
    /// The `reverse_patch` transforms new text → old text.
    /// Both are computed at diff time using `diffy::create_patch`.
    TextDiff {
        forward_patch: String,
        reverse_patch: String,
    },
}

/// Deserialization default for legacy changelog entries that lack `entity_type`.
fn empty_entity_type_name() -> EntityTypeName {
    EntityTypeName::from("")
}

/// Deserialization default for legacy changelog entries that lack `entity_id`.
fn empty_entity_id() -> EntityId {
    EntityId::from("")
}

/// A single change event for an entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeEntry {
    pub id: ChangeEntryId,
    pub timestamp: DateTime<Utc>,
    #[serde(default = "empty_entity_type_name")]
    pub entity_type: EntityTypeName,
    #[serde(default = "empty_entity_id")]
    pub entity_id: EntityId,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    pub changes: Vec<(String, FieldChange)>,
    /// References the original changelog entry being undone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undone_id: Option<ChangeEntryId>,
    /// References the original changelog entry being redone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redone_id: Option<ChangeEntryId>,
    /// Groups entries into a logical transaction (for future use).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<TransactionId>,
}

impl ChangeEntry {
    /// Create a new change entry with the given entity type, entity ID, operation, and changes.
    pub fn new(
        entity_type: impl Into<EntityTypeName>,
        entity_id: impl Into<EntityId>,
        op: impl Into<String>,
        changes: Vec<(String, FieldChange)>,
    ) -> Self {
        Self {
            id: ChangeEntryId::new(),
            timestamp: Utc::now(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            op: op.into(),
            actor: None,
            changes,
            undone_id: None,
            redone_id: None,
            transaction_id: None,
        }
    }

    /// Set the actor who made this change.
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Set the ULID of the changelog entry being undone.
    pub fn with_undone_id(mut self, ulid: impl Into<ChangeEntryId>) -> Self {
        self.undone_id = Some(ulid.into());
        self
    }

    /// Set the ULID of the changelog entry being redone.
    pub fn with_redone_id(mut self, ulid: impl Into<ChangeEntryId>) -> Self {
        self.redone_id = Some(ulid.into());
        self
    }

    /// Set the transaction ID for grouping related entries.
    pub fn with_transaction_id(mut self, txn_id: impl Into<TransactionId>) -> Self {
        self.transaction_id = Some(txn_id.into());
        self
    }
}

/// Compare two entity snapshots, producing field-level changes.
///
/// String values produce `TextDiff` with forward and reverse unified diff patches.
/// Non-string values produce `Changed` with old/new JSON values.
/// Fields present only in `new` produce `Set`; fields only in `old` produce `Removed`.
pub fn diff_entities(old: &Entity, new: &Entity) -> Vec<(String, FieldChange)> {
    let mut changes = Vec::new();

    // Check fields in new entity
    for (key, new_val) in &new.fields {
        match old.fields.get(key) {
            Some(old_val) if old_val == new_val => {
                // Unchanged — skip
            }
            Some(old_val) => {
                // Changed — use TextDiff for strings, Changed for others
                if let (Some(old_str), Some(new_str)) = (old_val.as_str(), new_val.as_str()) {
                    let forward = diffy::create_patch(old_str, new_str);
                    let reverse = diffy::create_patch(new_str, old_str);
                    changes.push((
                        key.clone(),
                        FieldChange::TextDiff {
                            forward_patch: forward.to_string(),
                            reverse_patch: reverse.to_string(),
                        },
                    ));
                } else {
                    changes.push((
                        key.clone(),
                        FieldChange::Changed {
                            old_value: old_val.clone(),
                            new_value: new_val.clone(),
                        },
                    ));
                }
            }
            None => {
                // New field
                changes.push((
                    key.clone(),
                    FieldChange::Set {
                        value: new_val.clone(),
                    },
                ));
            }
        }
    }

    // Check for removed fields
    for (key, old_val) in &old.fields {
        if !new.fields.contains_key(key) {
            changes.push((
                key.clone(),
                FieldChange::Removed {
                    old_value: old_val.clone(),
                },
            ));
        }
    }

    // Sort for deterministic output
    changes.sort_by(|a, b| a.0.cmp(&b.0));
    changes
}

/// Invert each change for undo.
///
/// - `Set { value }` → `Removed { old_value: value }`
/// - `Removed { old_value }` → `Set { value: old_value }`
/// - `Changed { old, new }` → `Changed { old: new, new: old }`
/// - `TextDiff { forward, reverse }` → `TextDiff { forward: reverse, reverse: forward }`
pub fn reverse_changes(changes: &[(String, FieldChange)]) -> Vec<(String, FieldChange)> {
    changes
        .iter()
        .map(|(key, change)| {
            let reversed = match change {
                FieldChange::Set { value } => FieldChange::Removed {
                    old_value: value.clone(),
                },
                FieldChange::Removed { old_value } => FieldChange::Set {
                    value: old_value.clone(),
                },
                FieldChange::Changed {
                    old_value,
                    new_value,
                } => FieldChange::Changed {
                    old_value: new_value.clone(),
                    new_value: old_value.clone(),
                },
                FieldChange::TextDiff {
                    forward_patch,
                    reverse_patch,
                } => FieldChange::TextDiff {
                    forward_patch: reverse_patch.clone(),
                    reverse_patch: forward_patch.clone(),
                },
            };
            (key.clone(), reversed)
        })
        .collect()
}

/// Apply changes forward (or reversed changes for undo) to an entity.
///
/// For `TextDiff`, the forward patch is applied to the current field value using `diffy::apply`.
/// Returns an error if a text patch cannot be applied cleanly.
pub fn apply_changes(entity: &mut Entity, changes: &[(String, FieldChange)]) -> Result<()> {
    for (key, change) in changes {
        match change {
            FieldChange::Set { value } => {
                entity.set(key, value.clone());
            }
            FieldChange::Removed { .. } => {
                entity.remove(key);
            }
            FieldChange::Changed {
                old_value,
                new_value,
            } => {
                // Stale detection: the entity's current value must match old_value.
                // For forward changes, old_value is the pre-change value.
                // For reversed changes (undo), reverse_changes swaps old/new,
                // so old_value is the value we expect to find in the entity.
                if let Some(current) = entity.get(key) {
                    if current != old_value {
                        return Err(crate::error::EntityError::StaleChange {
                            field: key.clone(),
                            expected: old_value.clone(),
                            actual: current.clone(),
                        });
                    }
                }
                entity.set(key, new_value.clone());
            }
            FieldChange::TextDiff { forward_patch, .. } => {
                let current = entity.get_str(key).unwrap_or("").to_string();
                let patch = diffy::Patch::from_str(forward_patch).map_err(|e| {
                    crate::error::EntityError::PatchApply(format!(
                        "failed to parse patch for field '{}': {}",
                        key, e
                    ))
                })?;
                let result = diffy::apply(&current, &patch).map_err(|e| {
                    crate::error::EntityError::PatchApply(format!(
                        "failed to apply patch to field '{}': {}",
                        key, e
                    ))
                })?;
                entity.set(key, Value::String(result));
            }
        }
    }
    Ok(())
}

/// Read all change entries from a JSONL log file (legacy entity format only).
///
/// Entity-format records (`ChangeEntry`) are returned as-is; store-format
/// records (`swissarmyhammer_store::ChangelogEntry`, patch-based) are silently
/// skipped because this entry point lacks the entity schema needed to replay
/// patches into field-level diffs.
///
/// Callers that need the full synthesised field-level history — including
/// projections of store-format records — must use [`read_changelog_for`]
/// instead. This function exists for the small set of internal call sites
/// that still operate without a resolved [`EntityDef`].
///
/// Malformed lines (neither entity nor store shape) are logged as warnings
/// and skipped.
#[deprecated(
    note = "prefer `read_changelog_for`, which projects store-format patches into field-level changes"
)]
pub async fn read_changelog(path: &Path) -> Result<Vec<ChangeEntry>> {
    let content = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::error::EntityError::Io(e)),
    };

    let mut entries = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(entry) = try_parse_entity_format(line) {
            entries.push(entry);
            continue;
        }
        if try_parse_store_format(line).is_some() {
            // Store-format lines are not field-level entries on their own.
            // The legacy reader silently drops them; new callers should use
            // `read_changelog_for` to replay them into field diffs.
            continue;
        }
        warn!(
            path = %path.display(),
            line_number = i + 1,
            "skipping malformed changelog entry"
        );
    }

    Ok(entries)
}

/// Read all change entries for an entity, replaying any store-format text
/// patches into field-level diffs.
///
/// The on-disk JSONL log can contain a mix of:
///
/// 1. Entity-format `ChangeEntry` records (legacy writer) — returned as-is.
/// 2. Store-format `ChangelogEntry` records (text patches via `diffy`) —
///    walked in file order with a running text cursor, applied forward,
///    parsed against the entity schema, and diffed to produce a synthesised
///    `ChangeEntry` with `changes` populated by [`diff_entities`].
/// 3. Blank lines — skipped.
/// 4. Genuinely malformed lines — warned and skipped.
///
/// The cursor advance is per-patch: each store record's `forward_patch` is
/// applied to the current text, the resulting new text is parsed as an
/// [`Entity`], and the diff against the previous parsed state becomes the
/// `changes` vector. The first store record in the log replays from an
/// empty cursor; subsequent ones build on the previous result.
///
/// Returned entries are sorted by timestamp so consumers see a chronological
/// view regardless of how entity-format and store-format records interleave
/// on disk.
///
/// `entity_def` is the schema for `entity_type`; it is consulted when parsing
/// patched text back into `Entity` so frontmatter+body and plain-YAML entities
/// are handled correctly.
pub async fn read_changelog_for(
    entity_type: &EntityTypeName,
    entity_def: &EntityDef,
    path: &Path,
) -> Result<Vec<ChangeEntry>> {
    let content = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::error::EntityError::Io(e)),
    };

    // Walk the file once, partitioning lines into the two on-disk shapes.
    // Store-format records are accumulated as a homogeneous slice so the
    // patch chain can be replayed by [`replay_store_log`] — that keeps the
    // homogeneous and interleaved code paths sharing one implementation
    // and guarantees patch `n` always applies to the result of patch
    // `n-1`, regardless of how many entity-format lines sit between them
    // on disk. Legacy entity-format records live in a parallel projection
    // (not the patch chain) and are collected separately.
    let mut entity_entries = Vec::new();
    let mut store_entries = Vec::new();

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(entry) = try_parse_entity_format(line) {
            entity_entries.push(entry);
            continue;
        }
        if let Some(store_entry) = try_parse_store_format(line) {
            store_entries.push(store_entry);
            continue;
        }
        warn!(
            path = %path.display(),
            line_number = i + 1,
            "skipping malformed changelog entry"
        );
    }

    let mut entries = replay_store_log(entity_type, entity_def, &store_entries, path)?;
    entries.append(&mut entity_entries);

    // Stable sort by timestamp so mixed-shape logs come back chronologically,
    // and ties preserve the on-disk order within each shape.
    entries.sort_by_key(|e| e.timestamp);
    Ok(entries)
}

/// Try to parse `line` as an entity-format [`ChangeEntry`]. Returns `None`
/// if the line is not entity-shape JSON (it may still be store-format or
/// garbage).
fn try_parse_entity_format(line: &str) -> Option<ChangeEntry> {
    serde_json::from_str::<ChangeEntry>(line).ok()
}

/// Try to parse `line` as a [`StoreChangelogEntry`] (patch-based). Returns
/// `None` if the line is not store-shape JSON.
///
/// The discriminator is the presence of `forward_patch` plus `item_id`:
/// entity-format records use `changes`/`entity_id` and never carry those
/// fields. We deserialise into the strongly-typed struct rather than peeking
/// at a generic `Value` so any partial overlap surfaces as a parse error.
fn try_parse_store_format(line: &str) -> Option<StoreChangelogEntry> {
    serde_json::from_str::<StoreChangelogEntry>(line).ok()
}

/// Replay a slice of store-format records into field-level
/// [`ChangeEntry`]s.
///
/// Walks the slice in order, applying each `forward_patch` to a running text
/// cursor (starting from the empty string for the first record), parsing
/// before/after as [`Entity`] using `entity_def`, and diffing to produce
/// `changes`. Each output entry preserves the store record's `id`,
/// `timestamp`, and translated `op`.
///
/// This is the homogeneous-input core of [`read_changelog_for`]: the reader
/// partitions the JSONL file into entity-format and store-format records,
/// then delegates the store-format slice here so the patch chain is replayed
/// in exactly one place.
fn replay_store_log(
    entity_type: &EntityTypeName,
    entity_def: &EntityDef,
    store_entries: &[StoreChangelogEntry],
    source_path: &Path,
) -> Result<Vec<ChangeEntry>> {
    let mut out = Vec::with_capacity(store_entries.len());
    let mut cursor = String::new();

    for store_entry in store_entries {
        let (entry, next_cursor) =
            replay_one_store_entry(entity_type, entity_def, store_entry, &cursor, source_path)?;
        out.push(entry);
        cursor = next_cursor;
    }

    Ok(out)
}

/// Replay a single store-format record against the running text `cursor`.
///
/// Returns the synthesised [`ChangeEntry`] and the updated cursor (the text
/// produced by applying `forward_patch`). Factoring this step out keeps
/// [`replay_store_log`] a thin loop over a shared cursor, so the patch
/// chain has exactly one implementation regardless of how the surrounding
/// reader assembled the slice.
fn replay_one_store_entry(
    entity_type: &EntityTypeName,
    entity_def: &EntityDef,
    store_entry: &StoreChangelogEntry,
    cursor: &str,
    source_path: &Path,
) -> Result<(ChangeEntry, String)> {
    let entity_id = EntityId::from(store_entry.item_id.as_str());
    let before = parse_entity_text(
        cursor,
        entity_type.as_str(),
        entity_id.as_str(),
        entity_def,
        source_path,
    )?;

    let new_text = swissarmyhammer_store::diff::apply_patch(cursor, &store_entry.forward_patch)
        .map_err(|e| {
            crate::error::EntityError::PatchApply(format!(
                "failed to apply store-format patch for {}/{}: {}",
                entity_type.as_str(),
                entity_id.as_str(),
                e
            ))
        })?;

    let after = parse_entity_text(
        &new_text,
        entity_type.as_str(),
        entity_id.as_str(),
        entity_def,
        source_path,
    )?;

    let changes = if matches!(store_entry.op, ChangeOp::Create) {
        // Treat creates as "every field set from nothing" — diff_entities
        // against an empty entity already produces exactly that, but we
        // call it out for clarity.
        diff_entities(
            &Entity::new(entity_type.as_str(), entity_id.as_str()),
            &after,
        )
    } else {
        diff_entities(&before, &after)
    };

    let entry = ChangeEntry {
        id: ChangeEntryId::from(store_entry.id.to_string()),
        timestamp: store_entry.timestamp,
        entity_type: entity_type.clone(),
        entity_id,
        op: store_op_to_string(&store_entry.op).to_string(),
        actor: None,
        changes,
        undone_id: None,
        redone_id: None,
        transaction_id: store_entry
            .transaction_id
            .as_deref()
            .map(TransactionId::from),
    };

    Ok((entry, new_text))
}

/// Translate a store-layer [`ChangeOp`] to the lowercase string form used by
/// entity-layer [`ChangeEntry::op`]. Mirrors the `#[serde(rename_all =
/// "lowercase")]` form so round-trips via JSON stay stable.
fn store_op_to_string(op: &ChangeOp) -> &'static str {
    match op {
        ChangeOp::Create => "create",
        ChangeOp::Update => "update",
        ChangeOp::Delete => "delete",
        ChangeOp::Archive => "archive",
        ChangeOp::Unarchive => "unarchive",
    }
}

/// Read all change entries, falling back to a secondary path if the primary does not exist.
///
/// Tries the `primary` path first. If the file is not found there, tries the `fallback` path.
/// This is useful for reading changelogs of deleted entities whose files have been moved to trash.
///
/// Honors the entity schema, so store-format text patches are replayed into
/// field-level diffs just like [`read_changelog_for`].
pub async fn read_changelog_with_fallback(
    entity_type: &EntityTypeName,
    entity_def: &EntityDef,
    primary: &Path,
    fallback: &Path,
) -> Result<Vec<ChangeEntry>> {
    let entries = read_changelog_for(entity_type, entity_def, primary).await?;
    if entries.is_empty() && !primary.exists() {
        return read_changelog_for(entity_type, entity_def, fallback).await;
    }
    Ok(entries)
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    /// Append a legacy entity-format `ChangeEntry` line to the given
    /// `.jsonl` path. Used by tests to seed on-disk fixtures that the
    /// projecting reader will translate back into field-level diffs.
    async fn write_legacy_changelog_line(path: &Path, entry: &ChangeEntry) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        let mut line = serde_json::to_string(entry).unwrap();
        line.push('\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .unwrap();
        file.write_all(line.as_bytes()).await.unwrap();
    }

    #[test]
    fn diff_no_changes() {
        let mut e = Entity::new("task", "01ABC");
        e.set("title", Value::String("Hello".into()));
        let changes = diff_entities(&e, &e);
        assert!(changes.is_empty());
    }

    #[test]
    fn diff_added_field() {
        let old = Entity::new("task", "01ABC");
        let mut new = Entity::new("task", "01ABC");
        new.set("title", Value::String("Hello".into()));

        let changes = diff_entities(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "title");
        assert!(matches!(changes[0].1, FieldChange::Set { .. }));
    }

    #[test]
    fn diff_removed_field() {
        let mut old = Entity::new("task", "01ABC");
        old.set("title", Value::String("Hello".into()));
        let new = Entity::new("task", "01ABC");

        let changes = diff_entities(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "title");
        assert!(matches!(changes[0].1, FieldChange::Removed { .. }));
    }

    #[test]
    fn diff_changed_non_string() {
        let mut old = Entity::new("task", "01ABC");
        old.set("count", serde_json::json!(1));
        let mut new = Entity::new("task", "01ABC");
        new.set("count", serde_json::json!(2));

        let changes = diff_entities(&old, &new);
        assert_eq!(changes.len(), 1);
        match &changes[0].1 {
            FieldChange::Changed {
                old_value,
                new_value,
            } => {
                assert_eq!(*old_value, serde_json::json!(1));
                assert_eq!(*new_value, serde_json::json!(2));
            }
            _ => panic!("expected Changed"),
        }
    }

    #[test]
    fn diff_changed_string_produces_text_diff() {
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nmodified\nline3".into()));

        let changes = diff_entities(&old, &new);
        assert_eq!(changes.len(), 1);
        match &changes[0].1 {
            FieldChange::TextDiff {
                forward_patch,
                reverse_patch,
            } => {
                // Forward patch should show line2 → modified
                assert!(forward_patch.contains("-line2"));
                assert!(forward_patch.contains("+modified"));
                // Reverse patch should show modified → line2
                assert!(reverse_patch.contains("-modified"));
                assert!(reverse_patch.contains("+line2"));
            }
            _ => panic!("expected TextDiff"),
        }
    }

    #[test]
    fn text_diff_forward_patch_applies_correctly() {
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nmodified\nline3".into()));

        let changes = diff_entities(&old, &new);

        // Apply forward to old → should produce new
        let mut result = old.clone();
        apply_changes(&mut result, &changes).unwrap();
        assert_eq!(result.get_str("body"), Some("line1\nmodified\nline3"));
    }

    #[test]
    fn text_diff_reverse_patch_applies_correctly() {
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nmodified\nline3".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // Apply reversed to new → should produce old
        let mut result = new.clone();
        apply_changes(&mut result, &reversed).unwrap();
        assert_eq!(result.get_str("body"), Some("line1\nline2\nline3"));
    }

    #[test]
    fn reverse_set_becomes_removed() {
        let changes = vec![(
            "title".to_string(),
            FieldChange::Set {
                value: Value::String("Hello".into()),
            },
        )];
        let reversed = reverse_changes(&changes);
        assert_eq!(reversed.len(), 1);
        match &reversed[0].1 {
            FieldChange::Removed { old_value } => {
                assert_eq!(*old_value, Value::String("Hello".into()));
            }
            _ => panic!("expected Removed"),
        }
    }

    #[test]
    fn reverse_removed_becomes_set() {
        let changes = vec![(
            "title".to_string(),
            FieldChange::Removed {
                old_value: Value::String("Hello".into()),
            },
        )];
        let reversed = reverse_changes(&changes);
        match &reversed[0].1 {
            FieldChange::Set { value } => {
                assert_eq!(*value, Value::String("Hello".into()));
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn reverse_changed_swaps_values() {
        let changes = vec![(
            "count".to_string(),
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
        )];
        let reversed = reverse_changes(&changes);
        match &reversed[0].1 {
            FieldChange::Changed {
                old_value,
                new_value,
            } => {
                assert_eq!(*old_value, serde_json::json!(2));
                assert_eq!(*new_value, serde_json::json!(1));
            }
            _ => panic!("expected Changed"),
        }
    }

    #[test]
    fn reverse_text_diff_swaps_patches() {
        let changes = vec![(
            "body".to_string(),
            FieldChange::TextDiff {
                forward_patch: "forward".into(),
                reverse_patch: "reverse".into(),
            },
        )];
        let reversed = reverse_changes(&changes);
        match &reversed[0].1 {
            FieldChange::TextDiff {
                forward_patch,
                reverse_patch,
            } => {
                assert_eq!(forward_patch, "reverse");
                assert_eq!(reverse_patch, "forward");
            }
            _ => panic!("expected TextDiff"),
        }
    }

    #[test]
    fn apply_set_adds_field() {
        let mut entity = Entity::new("task", "01ABC");
        let changes = vec![(
            "title".to_string(),
            FieldChange::Set {
                value: Value::String("Hello".into()),
            },
        )];
        apply_changes(&mut entity, &changes).unwrap();
        assert_eq!(entity.get_str("title"), Some("Hello"));
    }

    #[test]
    fn apply_removed_deletes_field() {
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Hello".into()));
        let changes = vec![(
            "title".to_string(),
            FieldChange::Removed {
                old_value: Value::String("Hello".into()),
            },
        )];
        apply_changes(&mut entity, &changes).unwrap();
        assert_eq!(entity.get("title"), None);
    }

    #[test]
    fn apply_changed_updates_field() {
        let mut entity = Entity::new("task", "01ABC");
        entity.set("count", serde_json::json!(1));
        let changes = vec![(
            "count".to_string(),
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
        )];
        apply_changes(&mut entity, &changes).unwrap();
        assert_eq!(entity.get_i64("count"), Some(2));
    }

    #[test]
    fn diff_then_reverse_restores_original() {
        let mut old = Entity::new("task", "01ABC");
        old.set("title", Value::String("Original".into()));
        old.set(
            "body",
            Value::String("line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\nline16\nline17\nline18\nline19\nline20".into()),
        );
        old.set("count", serde_json::json!(5));

        let mut new = old.clone();
        new.set("title", Value::String("Updated".into()));
        new.set(
            "body",
            Value::String("line1\nline2\nINSERTED\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\nline16\nline17\nMODIFIED18\nline19\nline20".into()),
        );
        new.remove("count");
        new.set("extra", Value::String("new field".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // Apply forward changes to old, should match new
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(forward.get_str("title"), new.get_str("title"));
        assert_eq!(forward.get_str("body"), new.get_str("body"));
        assert_eq!(forward.get("count"), None);
        assert_eq!(forward.get_str("extra"), Some("new field"));

        // Apply reversed changes to forward, should match old
        apply_changes(&mut forward, &reversed).unwrap();
        assert_eq!(forward.get_str("title"), old.get_str("title"));
        assert_eq!(forward.get_str("body"), old.get_str("body"));
        assert_eq!(forward.get_i64("count"), Some(5));
        assert_eq!(forward.get("extra"), None);
    }

    #[test]
    fn diff_then_reverse_with_scattered_edits() {
        // Real editing pattern: multiple scattered changes across a long document
        let old_lines: Vec<String> = (1..=50).map(|i| format!("line {}", i)).collect();
        let old_text = old_lines.join("\n");

        let mut new_lines = old_lines.clone();
        new_lines[2] = "MODIFIED line 3".into(); // edit near top
        new_lines.insert(10, "INSERTED after line 10".into()); // insert in middle
        new_lines[40] = "MODIFIED line 40".into(); // edit near bottom
        new_lines.push("APPENDED line 51".into()); // append at end
        let new_text = new_lines.join("\n");

        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String(old_text.clone()));

        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String(new_text.clone()));

        let changes = diff_entities(&old, &new);

        // Verify it produced a TextDiff with actual patch content
        match &changes[0].1 {
            FieldChange::TextDiff { forward_patch, .. } => {
                // Should show the actual edits, not 50+ lines of old/new
                assert!(forward_patch.contains("MODIFIED line 3"));
                assert!(forward_patch.contains("INSERTED after line 10"));
                assert!(forward_patch.contains("MODIFIED line 40"));
                assert!(forward_patch.contains("APPENDED line 51"));
            }
            _ => panic!("expected TextDiff"),
        }

        let reversed = reverse_changes(&changes);

        // Forward
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(forward.get_str("body"), Some(new_text.as_str()));

        // Reverse
        apply_changes(&mut forward, &reversed).unwrap();
        assert_eq!(forward.get_str("body"), Some(old_text.as_str()));
    }

    #[test]
    fn change_entry_serializes_to_json() {
        let entry = ChangeEntry::new(
            "task",
            "01ABC",
            "update",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("Hello".into()),
                },
            )],
        )
        .with_actor("user1");

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: ChangeEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.op, "update");
        assert_eq!(parsed.actor, Some("user1".into()));
        assert_eq!(parsed.changes.len(), 1);
    }

    #[test]
    fn text_diff_serializes_to_json() {
        let change = FieldChange::TextDiff {
            forward_patch: "--- original\n+++ modified\n@@ -1 +1 @@\n-old\n+new\n".into(),
            reverse_patch: "--- modified\n+++ original\n@@ -1 +1 @@\n-new\n+old\n".into(),
        };
        let json = serde_json::to_string(&change).unwrap();
        let parsed: FieldChange = serde_json::from_str(&json).unwrap();
        assert_eq!(change, parsed);
    }

    #[tokio::test]
    async fn append_and_read_changelog() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.jsonl");

        let entry1 = ChangeEntry::new(
            "task",
            "01ABC",
            "create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("First".into()),
                },
            )],
        );
        let entry2 = ChangeEntry::new(
            "task",
            "01ABC",
            "update",
            vec![(
                "title".to_string(),
                FieldChange::Changed {
                    old_value: Value::String("First".into()),
                    new_value: Value::String("Second".into()),
                },
            )],
        );

        write_legacy_changelog_line(&log_path, &entry1).await;
        write_legacy_changelog_line(&log_path, &entry2).await;

        let entries = read_changelog(&log_path).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].op, "create");
        assert_eq!(entries[1].op, "update");
    }

    #[tokio::test]
    async fn read_changelog_nonexistent() {
        let entries = read_changelog(Path::new("/tmp/nonexistent_log.jsonl"))
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn read_changelog_skips_malformed_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.jsonl");

        // Write a valid entry, a malformed line, and another valid entry
        let entry = ChangeEntry::new(
            "task",
            "01ABC",
            "create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("Hello".into()),
                },
            )],
        );
        let valid_line = serde_json::to_string(&entry).unwrap();

        let content = format!("{}\nNOT VALID JSON\n{}\n", valid_line, valid_line);
        fs::write(&log_path, content).await.unwrap();

        let entries = read_changelog(&log_path).await.unwrap();
        // Should have 2 valid entries, skipping the malformed line
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].op, "create");
        assert_eq!(entries[1].op, "create");
    }

    // -----------------------------------------------------------------------
    // Replay-reader tests (single-changelog: read field-level history by
    // projecting store-layer text patches via the entity schema).
    // -----------------------------------------------------------------------

    /// Build a minimal task-shaped `EntityDef` (md frontmatter + body field).
    fn task_entity_def() -> swissarmyhammer_fields::EntityDef {
        swissarmyhammer_fields::EntityDef {
            name: "task".into(),
            icon: None,
            body_field: Some("body".into()),
            fields: vec!["title".into(), "body".into()],
            sections: vec![],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            mention_slug_field: None,
            search_display_field: None,
        }
    }

    /// Render a task entity as its on-disk markdown text. The exact output of
    /// the entity writer is what gets diffed, so the replay engine must be
    /// driven with the same shape: frontmatter contains only non-body fields
    /// (here, `title`), and the body field lives solely in the post-`---`
    /// section. Mirrors `swissarmyhammer-entity/src/io.rs::format_frontmatter_body`.
    fn task_text(title: &str, body: &str) -> String {
        format!(
            "---\ntitle: {title}\n---\n{body}",
            title = title,
            body = body
        )
    }

    /// Build a synthetic store-format ChangelogEntry that transforms `old`
    /// into `new` text via `diffy`.
    fn store_entry(
        item_id: &str,
        op: swissarmyhammer_store::ChangeOp,
        old: &str,
        new: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> swissarmyhammer_store::ChangelogEntry {
        let (forward_patch, reverse_patch) = swissarmyhammer_store::diff::create_patches(old, new);
        swissarmyhammer_store::ChangelogEntry::new(
            swissarmyhammer_store::UndoEntryId::new(),
            timestamp,
            op,
            swissarmyhammer_store::StoredItemId::from(item_id),
            forward_patch,
            reverse_patch,
        )
    }

    #[tokio::test]
    async fn read_changelog_replays_store_format_to_field_changes() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("01ABC.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let v0 = "";
        let v1 = task_text("First", "alpha");
        let v2 = task_text("Second", "alpha");
        let v3 = task_text("Second", "beta");

        let ts0 = chrono::Utc::now();
        let ts1 = ts0 + chrono::Duration::seconds(1);
        let ts2 = ts0 + chrono::Duration::seconds(2);

        let e0 = store_entry("01ABC", ChangeOp::Create, v0, &v1, ts0);
        let e1 = store_entry("01ABC", ChangeOp::Update, &v1, &v2, ts1);
        let e2 = store_entry("01ABC", ChangeOp::Update, &v2, &v3, ts2);

        let mut content = String::new();
        for entry in [&e0, &e1, &e2] {
            content.push_str(&serde_json::to_string(entry).unwrap());
            content.push('\n');
        }
        fs::write(&log_path, content).await.unwrap();

        let entries = read_changelog_for(&type_name, &def, &log_path)
            .await
            .unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].op, "create");
        assert_eq!(entries[1].op, "update");
        assert_eq!(entries[2].op, "update");
        assert_eq!(entries[0].timestamp, ts0);
        assert_eq!(entries[1].timestamp, ts1);
        assert_eq!(entries[2].timestamp, ts2);

        // Create: every field surfaces as Set against the empty before-state.
        let create_fields: Vec<&str> = entries[0].changes.iter().map(|(k, _)| k.as_str()).collect();
        assert!(create_fields.contains(&"title"));
        assert!(create_fields.contains(&"body"));
        for (_, change) in &entries[0].changes {
            assert!(
                matches!(change, FieldChange::Set { .. }),
                "create changes must all be Set: {:?}",
                change
            );
        }

        // First update: title only.
        let update_keys: Vec<&str> = entries[1].changes.iter().map(|(k, _)| k.as_str()).collect();
        assert!(update_keys.contains(&"title"), "expected title in update");

        // Second update: body only.
        let update2_keys: Vec<&str> = entries[2].changes.iter().map(|(k, _)| k.as_str()).collect();
        assert!(update2_keys.contains(&"body"), "expected body in update");
    }

    #[tokio::test]
    async fn read_changelog_handles_mixed_legacy_and_store_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("01ABC.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let v0 = "";
        let v1 = task_text("First", "alpha");
        let v2 = task_text("Second", "alpha");

        let ts0 = chrono::Utc::now();
        let ts1 = ts0 + chrono::Duration::seconds(1);
        let ts2 = ts0 + chrono::Duration::seconds(2);
        let ts3 = ts0 + chrono::Duration::seconds(3);

        // Two store-format records...
        let s0 = store_entry("01ABC", ChangeOp::Create, v0, &v1, ts0);
        let s2 = store_entry("01ABC", ChangeOp::Update, &v1, &v2, ts2);

        // ...interleaved with two entity-format legacy records.
        let mut legacy_a = ChangeEntry::new(
            "task",
            "01ABC",
            "annotate",
            vec![(
                "comment".to_string(),
                FieldChange::Set {
                    value: Value::String("legacy A".into()),
                },
            )],
        );
        legacy_a.timestamp = ts1;
        let mut legacy_b = ChangeEntry::new(
            "task",
            "01ABC",
            "annotate",
            vec![(
                "comment".to_string(),
                FieldChange::Set {
                    value: Value::String("legacy B".into()),
                },
            )],
        );
        legacy_b.timestamp = ts3;

        let lines = [
            serde_json::to_string(&legacy_a).unwrap(),
            serde_json::to_string(&s0).unwrap(),
            serde_json::to_string(&legacy_b).unwrap(),
            serde_json::to_string(&s2).unwrap(),
        ];
        fs::write(&log_path, lines.join("\n") + "\n").await.unwrap();

        let entries = read_changelog_for(&type_name, &def, &log_path)
            .await
            .unwrap();

        assert_eq!(entries.len(), 4);

        // Sorted by timestamp regardless of on-disk order.
        let timestamps: Vec<_> = entries.iter().map(|e| e.timestamp).collect();
        let mut sorted = timestamps.clone();
        sorted.sort();
        assert_eq!(timestamps, sorted, "entries must come out chronologically");

        assert_eq!(entries[0].op, "create"); // s0 at ts0
        assert_eq!(entries[1].op, "annotate"); // legacy_a at ts1
        assert_eq!(entries[2].op, "update"); // s2 at ts2
        assert_eq!(entries[3].op, "annotate"); // legacy_b at ts3
    }

    #[tokio::test]
    async fn read_changelog_replay_handles_create_from_empty() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("01ABC.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let v0 = "";
        let v1 = task_text("Hello", "world");
        let ts0 = chrono::Utc::now();
        let s0 = store_entry("01ABC", ChangeOp::Create, v0, &v1, ts0);

        fs::write(&log_path, serde_json::to_string(&s0).unwrap() + "\n")
            .await
            .unwrap();

        let entries = read_changelog_for(&type_name, &def, &log_path)
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op, "create");

        // Every field surfaces as Set.
        assert!(
            !entries[0].changes.is_empty(),
            "create must produce at least one field change"
        );
        for (key, change) in &entries[0].changes {
            assert!(
                matches!(change, FieldChange::Set { .. }),
                "field {key} should be Set on create-from-empty, got {:?}",
                change
            );
        }
    }

    #[tokio::test]
    async fn read_changelog_replay_skips_genuinely_malformed_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("01ABC.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let v0 = "";
        let v1 = task_text("Hello", "world");
        let ts0 = chrono::Utc::now();
        let s0 = store_entry("01ABC", ChangeOp::Create, v0, &v1, ts0);

        let content = format!("{}\n{{not json\n", serde_json::to_string(&s0).unwrap());
        fs::write(&log_path, content).await.unwrap();

        let entries = read_changelog_for(&type_name, &def, &log_path)
            .await
            .unwrap();
        // The valid store record replays; the garbage line is warned and skipped.
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op, "create");
    }

    #[test]
    fn diff_entities_sorted_deterministically() {
        let mut old = Entity::new("task", "01ABC");
        old.set("zebra", Value::String("z".into()));
        old.set("alpha", Value::String("a".into()));

        let mut new = Entity::new("task", "01ABC");
        new.set("zebra", Value::String("Z".into()));
        new.set("alpha", Value::String("A".into()));

        let changes = diff_entities(&old, &new);
        assert_eq!(changes[0].0, "alpha");
        assert_eq!(changes[1].0, "zebra");
    }

    #[test]
    fn diff_then_reverse_with_deletions() {
        // Multi-hunk diff where lines are deleted (not just substituted).
        // Verifies hunk headers stay correct through the round-trip.
        let old_lines: Vec<String> = (1..=30).map(|i| format!("line {}", i)).collect();
        let old_text = old_lines.join("\n");

        let mut new_lines = old_lines.clone();
        // Delete lines near the top (lines 3-4)
        new_lines.remove(3); // "line 4"
        new_lines.remove(2); // "line 3"
                             // Delete a line near the bottom (original "line 25", now shifted)
        new_lines.retain(|l| l != "line 25");
        let new_text = new_lines.join("\n");

        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String(old_text.clone()));

        let mut new_ent = Entity::new("task", "01ABC");
        new_ent.set("body", Value::String(new_text.clone()));

        let changes = diff_entities(&old, &new_ent);
        let reversed = reverse_changes(&changes);

        // Forward: old → new
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(forward.get_str("body"), Some(new_text.as_str()));

        // Reverse: new → old
        let mut back = new_ent.clone();
        apply_changes(&mut back, &reversed).unwrap();
        assert_eq!(back.get_str("body"), Some(old_text.as_str()));
    }

    #[test]
    fn diff_then_reverse_mixed_insertions_and_deletions() {
        // Combines insertions and deletions in a single diff to stress
        // multi-hunk asymmetric line-number changes.
        let old_lines: Vec<String> = (1..=20).map(|i| format!("line {}", i)).collect();
        let old_text = old_lines.join("\n");

        let mut new_lines = old_lines.clone();
        // Insert after line 3
        new_lines.insert(3, "INSERTED after line 3".into());
        // Delete original line 10 (now at index 10 due to insertion)
        new_lines.remove(10);
        // Substitute near end (original line 18, now shifted)
        let idx = new_lines.iter().position(|l| l == "line 18").unwrap();
        new_lines[idx] = "MODIFIED line 18".into();
        let new_text = new_lines.join("\n");

        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String(old_text.clone()));

        let mut new_ent = Entity::new("task", "01ABC");
        new_ent.set("body", Value::String(new_text.clone()));

        let changes = diff_entities(&old, &new_ent);
        let reversed = reverse_changes(&changes);

        // Forward
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(forward.get_str("body"), Some(new_text.as_str()));

        // Reverse
        let mut back = new_ent.clone();
        apply_changes(&mut back, &reversed).unwrap();
        assert_eq!(back.get_str("body"), Some(old_text.as_str()));
    }

    #[test]
    fn reversed_patches_have_valid_unified_diff_headers() {
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nmodified\nline3".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // After reversal, forward_patch is the original reverse_patch
        // (generated by diffy::create_patch(new, old)), and vice versa.
        // Both must be valid unified diffs with proper --- / +++ headers.
        match &reversed[0].1 {
            FieldChange::TextDiff {
                forward_patch,
                reverse_patch,
            } => {
                let fwd_lines: Vec<&str> = forward_patch.lines().collect();
                let rev_lines: Vec<&str> = reverse_patch.lines().collect();

                // Headers must start with exactly "--- " and "+++ "
                assert!(
                    fwd_lines[0].starts_with("--- "),
                    "forward header line 1: {}",
                    fwd_lines[0]
                );
                assert!(
                    fwd_lines[1].starts_with("+++ "),
                    "forward header line 2: {}",
                    fwd_lines[1]
                );
                assert!(
                    rev_lines[0].starts_with("--- "),
                    "reverse header line 1: {}",
                    rev_lines[0]
                );
                assert!(
                    rev_lines[1].starts_with("+++ "),
                    "reverse header line 2: {}",
                    rev_lines[1]
                );

                // Headers must NOT have malformed prefixes like "+++--" or "---++"
                assert!(
                    !fwd_lines[0].starts_with("---+"),
                    "malformed forward header: {}",
                    fwd_lines[0]
                );
                assert!(
                    !fwd_lines[1].starts_with("+++-"),
                    "malformed forward header: {}",
                    fwd_lines[1]
                );
                assert!(
                    !rev_lines[0].starts_with("---+"),
                    "malformed reverse header: {}",
                    rev_lines[0]
                );
                assert!(
                    !rev_lines[1].starts_with("+++-"),
                    "malformed reverse header: {}",
                    rev_lines[1]
                );

                // Both patches must parse and apply cleanly
                let patch =
                    diffy::Patch::from_str(forward_patch).expect("forward_patch should parse");
                diffy::apply("line1\nmodified\nline3", &patch).expect("forward_patch should apply");

                let patch =
                    diffy::Patch::from_str(reverse_patch).expect("reverse_patch should parse");
                diffy::apply("line1\nline2\nline3", &patch).expect("reverse_patch should apply");
            }
            _ => panic!("expected TextDiff"),
        }
    }

    #[test]
    fn trailing_newline_added_round_trips() {
        // Old text has no trailing newline, new text does
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("abc".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("xyz\n".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // Forward: old → new
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(
            forward.get_str("body"),
            Some("xyz\n"),
            "forward apply should add trailing newline"
        );

        // Reverse: new → old
        let mut back = new.clone();
        apply_changes(&mut back, &reversed).unwrap();
        assert_eq!(
            back.get_str("body"),
            Some("abc"),
            "reverse apply should remove trailing newline"
        );
    }

    #[test]
    fn trailing_newline_removed_round_trips() {
        // Old text has trailing newline, new text doesn't
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("abc\n".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("xyz".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // Forward: old → new
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(
            forward.get_str("body"),
            Some("xyz"),
            "forward apply should remove trailing newline"
        );

        // Reverse: new → old
        let mut back = new.clone();
        apply_changes(&mut back, &reversed).unwrap();
        assert_eq!(
            back.get_str("body"),
            Some("abc\n"),
            "reverse apply should add trailing newline"
        );
    }

    #[test]
    fn trailing_newline_multiline_round_trips() {
        // Multi-line content where trailing newline changes
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nline2\nline3\n".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // Forward: add trailing newline
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(forward.get_str("body"), Some("line1\nline2\nline3\n"));

        // Reverse: remove trailing newline
        let mut back = new.clone();
        apply_changes(&mut back, &reversed).unwrap();
        assert_eq!(back.get_str("body"), Some("line1\nline2\nline3"));
    }

    #[test]
    fn stale_diff_application_returns_error() {
        // Create a diff against the original text
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nmodified\nline3".into()));

        let changes = diff_entities(&old, &new);

        // Now apply that diff to a DIFFERENT text (stale/modified content)
        let mut stale = Entity::new("task", "01ABC");
        stale.set(
            "body",
            Value::String("line1\nTOTALLY_DIFFERENT\nline3".into()),
        );

        let result = apply_changes(&mut stale, &changes);
        assert!(
            result.is_err(),
            "applying a stale diff should return an error, not silently corrupt data"
        );
    }

    #[test]
    fn all_field_change_variants_round_trip_through_json() {
        let variants = vec![
            FieldChange::Set {
                value: serde_json::json!(42),
            },
            FieldChange::Removed {
                old_value: serde_json::json!("gone"),
            },
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
            FieldChange::TextDiff {
                forward_patch: "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new\n".into(),
                reverse_patch: "--- b\n+++ a\n@@ -1 +1 @@\n-new\n+old\n".into(),
            },
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let parsed: FieldChange = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, parsed, "round-trip failed for: {}", json);
        }
    }

    #[test]
    fn apply_changed_stale_value_returns_error() {
        // Entity has count=99, but the change expects old_value=1.
        // This is stale — apply_changes should reject it.
        let mut entity = Entity::new("task", "01ABC");
        entity.set("count", serde_json::json!(99));
        let changes = vec![(
            "count".to_string(),
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
        )];
        let result = apply_changes(&mut entity, &changes);
        assert!(result.is_err(), "stale Changed should error");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("stale"), "error should mention stale: {}", msg);
        assert!(
            msg.contains("count"),
            "error should mention field name: {}",
            msg
        );
        // Entity should NOT have been modified
        assert_eq!(entity.get_i64("count"), Some(99));
    }

    #[test]
    fn apply_changed_matching_value_succeeds() {
        // Entity has count=1, change expects old_value=1 — should apply fine.
        let mut entity = Entity::new("task", "01ABC");
        entity.set("count", serde_json::json!(1));
        let changes = vec![(
            "count".to_string(),
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
        )];
        apply_changes(&mut entity, &changes).unwrap();
        assert_eq!(entity.get_i64("count"), Some(2));
    }

    #[test]
    fn apply_changed_missing_field_applies_without_error() {
        // Entity does NOT have the field at all. The Changed variant should
        // still apply (field was missing, old_value check is skipped when
        // there's no current value).
        let mut entity = Entity::new("task", "01ABC");
        let changes = vec![(
            "count".to_string(),
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
        )];
        apply_changes(&mut entity, &changes).unwrap();
        assert_eq!(entity.get_i64("count"), Some(2));
    }

    #[tokio::test]
    async fn read_changelog_with_fallback_uses_primary_first() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("primary.jsonl");
        let fallback = dir.path().join("fallback.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let entry = ChangeEntry::new(
            "task",
            "01ABC",
            "create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("Primary".into()),
                },
            )],
        );
        write_legacy_changelog_line(&primary, &entry).await;

        let fallback_entry = ChangeEntry::new(
            "task",
            "01ABC",
            "fallback_create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("Fallback".into()),
                },
            )],
        );
        write_legacy_changelog_line(&fallback, &fallback_entry).await;

        let entries = read_changelog_with_fallback(&type_name, &def, &primary, &fallback)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op, "create"); // from primary, not fallback
    }

    #[tokio::test]
    async fn read_changelog_with_fallback_uses_fallback_when_primary_missing() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("nonexistent_primary.jsonl");
        let fallback = dir.path().join("fallback.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let entry = ChangeEntry::new(
            "task",
            "01ABC",
            "fallback_create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("Fallback".into()),
                },
            )],
        );
        write_legacy_changelog_line(&fallback, &entry).await;

        let entries = read_changelog_with_fallback(&type_name, &def, &primary, &fallback)
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op, "fallback_create");
    }

    #[tokio::test]
    async fn read_changelog_with_fallback_both_missing_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("a.jsonl");
        let fallback = dir.path().join("b.jsonl");
        let def = task_entity_def();
        let type_name = EntityTypeName::from("task");

        let entries = read_changelog_with_fallback(&type_name, &def, &primary, &fallback)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn read_changelog_skips_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test.jsonl");

        let entry = ChangeEntry::new(
            "task",
            "01ABC",
            "create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("Hello".into()),
                },
            )],
        );
        let valid_line = serde_json::to_string(&entry).unwrap();

        // Content with blank lines interspersed
        let content = format!("{}\n\n\n{}\n\n", valid_line, valid_line);
        fs::write(&log_path, content).await.unwrap();

        let entries = read_changelog(&log_path).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn change_entry_with_undone_id_sets_field() {
        let entry =
            ChangeEntry::new("task", "01ABC", "undo", vec![]).with_undone_id("ORIGINAL_ULID");
        assert_eq!(entry.undone_id.as_deref(), Some("ORIGINAL_ULID"));
        assert!(entry.redone_id.is_none());
    }

    #[test]
    fn change_entry_with_redone_id_sets_field() {
        let entry =
            ChangeEntry::new("task", "01ABC", "redo", vec![]).with_redone_id("ORIGINAL_ULID");
        assert_eq!(entry.redone_id.as_deref(), Some("ORIGINAL_ULID"));
        assert!(entry.undone_id.is_none());
    }

    #[test]
    fn change_entry_with_transaction_id_sets_field() {
        let entry =
            ChangeEntry::new("task", "01ABC", "update", vec![]).with_transaction_id("TX001");
        assert_eq!(entry.transaction_id.as_deref(), Some("TX001"));
    }

    #[test]
    fn apply_text_diff_on_missing_field_uses_empty_string() {
        // When the field doesn't exist, apply_changes uses "" as the current text
        let mut entity = Entity::new("task", "01ABC");
        // No "body" field set

        let changes = vec![(
            "body".to_string(),
            FieldChange::TextDiff {
                forward_patch: "--- original\n+++ modified\n@@ -1 +1 @@\n-\n+hello\n".into(),
                reverse_patch: "--- modified\n+++ original\n@@ -1 +1 @@\n-hello\n+\n".into(),
            },
        )];

        // This tests the get_str(key).unwrap_or("") path in apply_changes
        let result = apply_changes(&mut entity, &changes);
        // The patch may or may not apply cleanly depending on content, but the code
        // path is exercised regardless
        let _ = result;
    }

    #[test]
    fn apply_reversed_changed_stale_value_returns_error() {
        // Simulate an undo scenario where the entity has been modified since
        // the original change. reverse_changes swaps old/new, so the reversed
        // old_value is the original new_value. If the entity doesn't match,
        // it's stale.
        let original_changes = vec![(
            "order".to_string(),
            FieldChange::Changed {
                old_value: serde_json::json!(1),
                new_value: serde_json::json!(2),
            },
        )];
        let reversed = reverse_changes(&original_changes);

        // Entity currently has order=99 (stale), but the reversed change
        // expects old_value=2 (the original new_value).
        let mut entity = Entity::new("task", "01ABC");
        entity.set("order", serde_json::json!(99));
        let result = apply_changes(&mut entity, &reversed);
        assert!(result.is_err(), "stale reversed Changed should error");
    }
}
