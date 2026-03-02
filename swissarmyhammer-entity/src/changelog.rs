//! Entity change logging with field-level diffs.
//!
//! Every mutation to an entity produces a [`ChangeEntry`] recording which fields
//! changed and how. String fields store a unified text diff (via `similar`)
//! instead of full old/new values. Changes are reversible — each [`FieldChange`]
//! has a natural inverse.
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
use similar::TextDiff;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
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
    /// String field changed — record a unified text diff.
    TextDiff { diff: String },
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
/// String values get text diffs via `similar`; others get `Changed` with old/new.
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
                // Changed — use text diff for strings, Changed for others
                if let (Some(old_str), Some(new_str)) = (old_val.as_str(), new_val.as_str()) {
                    let diff = make_text_diff(old_str, new_str);
                    changes.push((key.clone(), FieldChange::TextDiff { diff }));
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
/// - `TextDiff { diff }` → `TextDiff { reversed_diff }`
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
                FieldChange::TextDiff { diff } => FieldChange::TextDiff {
                    diff: reverse_unified_diff(diff),
                },
            };
            (key.clone(), reversed)
        })
        .collect()
}

/// Apply changes forward (or reversed changes for undo) to an entity.
pub fn apply_changes(
    entity: &mut Entity,
    changes: &[(String, FieldChange)],
) -> Result<()> {
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
            FieldChange::TextDiff { diff } => {
                let old_text = entity.get_str(key).unwrap_or("");
                let new_text = apply_unified_diff(old_text, diff);
                entity.set(key, Value::String(new_text));
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
pub async fn read_changelog(path: &Path) -> Result<Vec<ChangeEntry>> {
    let content = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::error::EntityError::Io(e)),
    };

    let entries = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<ChangeEntry>(line).ok())
        .collect();

    Ok(entries)
}

// --- Internal helpers ---

/// Create a unified diff between two strings.
fn make_text_diff(old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    diff.unified_diff()
        .context_radius(3)
        .header("old", "new")
        .to_string()
}

/// Reverse a unified diff by swapping +/- lines.
fn reverse_unified_diff(diff: &str) -> String {
    diff.lines()
        .map(|line| {
            if let Some(rest) = line.strip_prefix('+') {
                if rest.starts_with("++") {
                    // Header line (+++ new) → (--- new)
                    format!("---{}", rest)
                } else {
                    format!("-{}", rest)
                }
            } else if let Some(rest) = line.strip_prefix('-') {
                if rest.starts_with("--") {
                    // Header line (--- old) → (+++ old)
                    format!("+++{}", rest)
                } else {
                    format!("+{}", rest)
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Apply a unified diff to a string.
///
/// This is a simple line-based application — it processes +/- lines to
/// reconstruct the result. Context lines and headers are skipped.
fn apply_unified_diff(old: &str, diff: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let mut result = Vec::new();
    let mut old_idx = 0;

    // Parse the diff to find hunk headers and apply changes
    let diff_lines: Vec<&str> = diff.lines().collect();
    let mut diff_idx = 0;

    // Skip header lines (--- and +++)
    while diff_idx < diff_lines.len() {
        let line = diff_lines[diff_idx];
        if line.starts_with("---") || line.starts_with("+++") {
            diff_idx += 1;
        } else {
            break;
        }
    }

    while diff_idx < diff_lines.len() {
        let line = diff_lines[diff_idx];
        if line.starts_with("@@") {
            // Parse hunk header: @@ -start,count +start,count @@
            if let Some((old_start, _)) = parse_hunk_header(line) {
                // Copy unchanged lines before this hunk
                let target = (old_start as usize).saturating_sub(1);
                while old_idx < target && old_idx < old_lines.len() {
                    result.push(old_lines[old_idx].to_string());
                    old_idx += 1;
                }
            }
            diff_idx += 1;
        } else if let Some(content) = line.strip_prefix('+') {
            // Added line
            result.push(content.to_string());
            diff_idx += 1;
        } else if line.starts_with('-') {
            // Removed line — skip in old
            old_idx += 1;
            diff_idx += 1;
        } else if line.starts_with(' ') {
            // Context line
            result.push(line[1..].to_string());
            old_idx += 1;
            diff_idx += 1;
        } else {
            // Unknown line, skip
            diff_idx += 1;
        }
    }

    // Copy remaining old lines
    while old_idx < old_lines.len() {
        result.push(old_lines[old_idx].to_string());
        old_idx += 1;
    }

    // Reconstruct with newlines, preserving trailing newline from original
    let mut output = result.join("\n");
    if old.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Parse a unified diff hunk header like `@@ -1,3 +1,4 @@`.
/// Returns (old_start, new_start).
fn parse_hunk_header(line: &str) -> Option<(i64, i64)> {
    // Format: @@ -old_start[,old_count] +new_start[,new_count] @@
    let line = line.strip_prefix("@@ ")?;
    let line = line.split(" @@").next()?;
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let old_start = parts[0]
        .strip_prefix('-')?
        .split(',')
        .next()?
        .parse::<i64>()
        .ok()?;
    let new_start = parts[1]
        .strip_prefix('+')?
        .split(',')
        .next()?
        .parse::<i64>()
        .ok()?;
    Some((old_start, new_start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
            FieldChange::Changed { old_value, new_value } => {
                assert_eq!(*old_value, serde_json::json!(1));
                assert_eq!(*new_value, serde_json::json!(2));
            }
            _ => panic!("expected Changed"),
        }
    }

    #[test]
    fn diff_changed_string_uses_text_diff() {
        let mut old = Entity::new("task", "01ABC");
        old.set("body", Value::String("line1\nline2\nline3".into()));
        let mut new = Entity::new("task", "01ABC");
        new.set("body", Value::String("line1\nmodified\nline3".into()));

        let changes = diff_entities(&old, &new);
        assert_eq!(changes.len(), 1);
        match &changes[0].1 {
            FieldChange::TextDiff { diff } => {
                assert!(diff.contains("-line2"));
                assert!(diff.contains("+modified"));
            }
            _ => panic!("expected TextDiff"),
        }
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
            FieldChange::Changed { old_value, new_value } => {
                assert_eq!(*old_value, serde_json::json!(2));
                assert_eq!(*new_value, serde_json::json!(1));
            }
            _ => panic!("expected Changed"),
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
        old.set("body", Value::String("line1\nline2\nline3".into()));
        old.set("count", serde_json::json!(5));

        let mut new = old.clone();
        new.set("title", Value::String("Updated".into()));
        new.set("body", Value::String("line1\nmodified\nline3".into()));
        new.set("count", serde_json::json!(10));
        new.set("extra", Value::String("new field".into()));
        new.remove("title"); // Actually remove title to test Removed

        // Re-add title with new value for a cleaner test
        let mut new = old.clone();
        new.set("title", Value::String("Updated".into()));
        new.set("body", Value::String("line1\nmodified\nline3".into()));
        new.remove("count");
        new.set("extra", Value::String("new field".into()));

        let changes = diff_entities(&old, &new);
        let reversed = reverse_changes(&changes);

        // Apply forward changes to old, should match new
        let mut forward = old.clone();
        apply_changes(&mut forward, &changes).unwrap();
        assert_eq!(forward.get_str("title"), Some("Updated"));
        assert_eq!(forward.get_str("body"), new.get_str("body"));
        assert_eq!(forward.get("count"), None);
        assert_eq!(forward.get_str("extra"), Some("new field"));

        // Apply reversed changes to forward, should match old
        apply_changes(&mut forward, &reversed).unwrap();
        assert_eq!(forward.get_str("title"), Some("Original"));
        assert_eq!(forward.get_str("body"), old.get_str("body"));
        assert_eq!(forward.get_i64("count"), Some(5));
        assert_eq!(forward.get("extra"), None);
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
    fn text_diff_round_trip() {
        let old_text = "line1\nline2\nline3\nline4\nline5";
        let new_text = "line1\nmodified\nline3\nline4\nline5\nline6";

        let diff = make_text_diff(old_text, new_text);

        // Apply forward
        let applied = apply_unified_diff(old_text, &diff);
        assert_eq!(applied, new_text);

        // Reverse and apply
        let reversed_diff = reverse_unified_diff(&diff);
        let restored = apply_unified_diff(new_text, &reversed_diff);
        assert_eq!(restored, old_text);
    }

    #[test]
    fn text_diff_empty_to_content() {
        let diff = make_text_diff("", "hello\nworld");
        let applied = apply_unified_diff("", &diff);
        assert_eq!(applied, "hello\nworld");
    }

    #[test]
    fn text_diff_content_to_empty() {
        let diff = make_text_diff("hello\nworld", "");
        let applied = apply_unified_diff("hello\nworld", &diff);
        assert_eq!(applied, "");
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
    fn parse_hunk_header_basic() {
        let result = parse_hunk_header("@@ -1,3 +1,4 @@");
        assert_eq!(result, Some((1, 1)));

        let result = parse_hunk_header("@@ -5 +5,2 @@");
        assert_eq!(result, Some((5, 5)));
    }
}
