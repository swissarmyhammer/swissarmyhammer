//! Entity change logging with field-level diffs.
//!
//! Every mutation to an entity produces a [`ChangeEntry`] recording which fields
//! changed and how. String fields store a unified diff patch that captures exactly
//! which lines were added, removed, or modified. Non-string fields store old/new
//! as JSON values. Changes are reversible — each [`FieldChange`] has a natural
//! inverse.
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
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tracing::warn;
use ulid::Ulid;

use crate::entity::Entity;
use crate::error::Result;

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

/// A single change event for an entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    pub changes: Vec<(String, FieldChange)>,
}

impl ChangeEntry {
    /// Create a new change entry with the given operation and changes.
    pub fn new(op: impl Into<String>, changes: Vec<(String, FieldChange)>) -> Self {
        Self {
            id: Ulid::new().to_string(),
            timestamp: Utc::now(),
            op: op.into(),
            actor: None,
            changes,
        }
    }

    /// Set the actor who made this change.
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
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
                changes.push((key.clone(), FieldChange::Set { value: new_val.clone() }));
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
            FieldChange::Changed { new_value, .. } => {
                entity.set(key, new_value.clone());
            }
            FieldChange::TextDiff { forward_patch, .. } => {
                let current = entity
                    .get_str(key)
                    .unwrap_or("")
                    .to_string();
                let patch = diffy::Patch::from_str(&forward_patch)
                    .map_err(|e| crate::error::EntityError::PatchApply(format!(
                        "failed to parse patch for field '{}': {}", key, e
                    )))?;
                let result = diffy::apply(&current, &patch)
                    .map_err(|e| crate::error::EntityError::PatchApply(format!(
                        "failed to apply patch to field '{}': {}", key, e
                    )))?;
                entity.set(key, Value::String(result));
            }
        }
    }
    Ok(())
}

/// Append a change entry to an entity's JSONL log file.
pub async fn append_changelog(path: &Path, entry: &ChangeEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut line = serde_json::to_string(entry).unwrap_or_default();
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(line.as_bytes()).await?;

    Ok(())
}

/// Read all change entries from a JSONL log file.
///
/// Malformed lines are logged as warnings and skipped.
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
        match serde_json::from_str::<ChangeEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                warn!(
                    path = %path.display(),
                    line_number = i + 1,
                    error = %e,
                    "skipping malformed changelog entry"
                );
            }
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        new_lines[2] = "MODIFIED line 3".into();           // edit near top
        new_lines.insert(10, "INSERTED after line 10".into()); // insert in middle
        new_lines[40] = "MODIFIED line 40".into();          // edit near bottom
        new_lines.push("APPENDED line 51".into());          // append at end
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
            "create",
            vec![(
                "title".to_string(),
                FieldChange::Set {
                    value: Value::String("First".into()),
                },
            )],
        );
        let entry2 = ChangeEntry::new(
            "update",
            vec![(
                "title".to_string(),
                FieldChange::Changed {
                    old_value: Value::String("First".into()),
                    new_value: Value::String("Second".into()),
                },
            )],
        );

        append_changelog(&log_path, &entry1).await.unwrap();
        append_changelog(&log_path, &entry2).await.unwrap();

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
}
