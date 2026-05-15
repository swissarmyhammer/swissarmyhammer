//! YAML merge strategy: three-way field merge with newest-wins conflict resolution.
//!
//! This module implements field-level merging for flat YAML entity files (tags, columns,
//! actors, boards, swimlanes, views). Only top-level fields are merged — the entities
//! in this system are flat YAML mappings.
//!
//! ## Algorithm
//!
//! For each field in the union of all three versions:
//! - Only ours changed → take ours
//! - Only theirs changed → take theirs
//! - Neither changed → keep base value
//! - Field added only in one side → take the addition
//! - Field removed only in one side → take the removal
//! - Both changed same field → resolve via JSONL changelog timestamps (newest wins),
//!   or fall back to configurable [`Precedence`]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::MergeError;

/// Which side wins when both changed the same field and no JSONL is available (or the
/// field does not appear in the changelog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Precedence {
    /// Their version wins (default — mirrors typical merge-driver convention).
    #[default]
    Theirs,
    /// Our version wins.
    Ours,
}

/// Options controlling YAML merge behaviour.
#[derive(Debug, Clone, Default)]
pub struct MergeOpts {
    /// Optional path to a JSONL changelog file for the entity being merged.
    /// When present, conflicting fields are resolved by comparing the most recent
    /// timestamp for each field across both sides.
    pub jsonl_path: Option<PathBuf>,
    /// Fallback precedence when the JSONL is absent or the conflicting field is not
    /// recorded in the changelog.
    pub fallback_precedence: Precedence,
}

/// A single parsed changelog field change — the field name and the new value after the change.
#[derive(Debug)]
struct FieldChange {
    /// Name of the changed field.
    field: String,
    /// The new value after the change, if present in the changelog entry.
    new_value: Option<serde_json::Value>,
}

/// A single parsed changelog entry — timestamp plus the list of field changes.
#[derive(Debug)]
struct ChangelogEntry {
    /// ISO-8601 timestamp string, used for lexicographic comparison (works for UTC).
    timestamp: String,
    /// Changes recorded in this entry.
    changes: Vec<FieldChange>,
}

/// Parse JSONL changelog content, returning entries ordered as they appear in the file.
///
/// Lines that fail to parse are silently skipped (defensive — corrupt log should not
/// prevent merging).
fn parse_changelog(content: &str) -> Vec<ChangelogEntry> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(timestamp) = value.get("timestamp").and_then(|t| t.as_str()) else {
            continue;
        };
        // changes is Vec<[field_name, FieldChange]> serialized as arrays.
        // FieldChange has a `new_value` key.
        let changes: Vec<FieldChange> = value
            .get("changes")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        // Each item is a two-element array: [field_name, {kind, old_value, new_value}]
                        let arr = item.as_array()?;
                        let field = arr.first()?.as_str()?.to_owned();
                        let new_value = arr.get(1).and_then(|fc| fc.get("new_value")).cloned();
                        Some(FieldChange { field, new_value })
                    })
                    .collect()
            })
            .unwrap_or_default();
        entries.push(ChangelogEntry {
            timestamp: timestamp.to_owned(),
            changes,
        });
    }
    entries
}

/// Determine the most recent timestamp at which each field was set to each specific value.
///
/// Returns a map of `field → (new_value_json_string, timestamp)` for the most recent
/// changelog entry that set that field.  When the caller has a candidate value for a
/// field from one side of the merge, it can look up the timestamp for that exact value.
/// If neither side's value matches a changelog entry, `None` is returned and the caller
/// falls back to `fallback_precedence`.
fn latest_value_timestamps(entries: &[ChangelogEntry]) -> HashMap<String, (String, String)> {
    // field → (new_value_as_string, timestamp) for the most-recent entry
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    for entry in entries {
        for change in &entry.changes {
            if let Some(new_val) = &change.new_value {
                let val_str = match new_val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let existing = map
                    .entry(change.field.clone())
                    .or_insert_with(|| (val_str.clone(), entry.timestamp.clone()));
                if entry.timestamp > existing.1 {
                    *existing = (val_str, entry.timestamp.clone());
                }
            }
        }
    }
    map
}

/// Merge three YAML mappings using field-level three-way merge with newest-wins conflict
/// resolution.
///
/// All three inputs must be YAML strings representing flat mappings (key → scalar value).
/// Only top-level field merge is performed; nested structures are treated as opaque values.
///
/// # Arguments
/// - `base` — common ancestor YAML content
/// - `ours` — our branch YAML content
/// - `theirs` — their branch YAML content
/// - `opts` — merge options (optional JSONL changelog path, fallback precedence)
///
/// # Returns
/// - `Ok(merged_yaml_string)` on success. All field conflicts are resolved internally
///   via the JSONL changelog or fallback precedence.
/// - `Err(MergeError::ParseFailure)` when any input cannot be parsed as a YAML mapping.
///   Serialization failures are also reported as `ParseFailure`.
pub fn merge_yaml(
    base: &str,
    ours: &str,
    theirs: &str,
    opts: &MergeOpts,
) -> Result<String, MergeError> {
    /// Parse a YAML string into a BTreeMap of (key → Value). Returns empty map for empty input.
    fn parse_mapping(yaml: &str) -> Result<BTreeMap<String, serde_yaml_ng::Value>, String> {
        let trimmed = yaml.trim();
        if trimmed.is_empty() {
            return Ok(BTreeMap::new());
        }
        let value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(trimmed).map_err(|e| format!("YAML parse error: {e}"))?;
        match value {
            serde_yaml_ng::Value::Mapping(m) => {
                let mut map = BTreeMap::new();
                for (k, v) in m {
                    if let serde_yaml_ng::Value::String(key) = k {
                        map.insert(key, v);
                    } else {
                        // Non-string keys — convert to string representation.
                        let key = format!("{k:?}");
                        map.insert(key, v);
                    }
                }
                Ok(map)
            }
            // Treat a null/empty document as an empty mapping.
            serde_yaml_ng::Value::Null => Ok(BTreeMap::new()),
            other => Err(format!("expected YAML mapping, got: {other:?}")),
        }
    }

    let base_map = parse_mapping(base).map_err(MergeError::ParseFailure)?;
    let ours_map = parse_mapping(ours).map_err(MergeError::ParseFailure)?;
    let theirs_map = parse_mapping(theirs).map_err(MergeError::ParseFailure)?;

    // Load JSONL changelog if provided.
    // The changelog is a single merged history file. We use it to determine which side's
    // value for a conflicting field is more recent: the side whose current value matches
    // the most recent changelog entry for that field wins.
    // If neither side's value is found in the changelog, fall back to `fallback_precedence`.
    let value_timestamps: HashMap<String, (String, String)> =
        if let Some(jsonl_path) = &opts.jsonl_path {
            let content = std::fs::read_to_string(jsonl_path).unwrap_or_default();
            let entries = parse_changelog(&content);
            latest_value_timestamps(&entries)
        } else {
            HashMap::new()
        };

    // Collect the union of all field keys.
    let mut all_keys: BTreeMap<String, ()> = BTreeMap::new();
    for k in base_map
        .keys()
        .chain(ours_map.keys())
        .chain(theirs_map.keys())
    {
        all_keys.insert(k.clone(), ());
    }

    let mut result: BTreeMap<String, serde_yaml_ng::Value> = BTreeMap::new();

    for key in all_keys.keys() {
        let base_val = base_map.get(key);
        let ours_val = ours_map.get(key);
        let theirs_val = theirs_map.get(key);

        let chosen = match (base_val, ours_val, theirs_val) {
            // Both removed — field is gone.
            (_, None, None) => None,

            // Added by ours only.
            (None, Some(v), None) => Some(v.clone()),

            // Added by theirs only.
            (None, None, Some(v)) => Some(v.clone()),

            // Removed by ours only (theirs unchanged from base).
            (Some(b), None, Some(t)) if t == b => None,

            // Removed by theirs only (ours unchanged from base).
            (Some(b), Some(o), None) if o == b => None,

            // Only ours changed.
            (Some(b), Some(o), Some(t)) if o != b && t == b => Some(o.clone()),

            // Only theirs changed.
            (Some(b), Some(o), Some(t)) if o == b && t != b => Some(t.clone()),

            // Neither side changed — keep base.
            (Some(b), Some(o), Some(t)) if o == b && t == b => Some(b.clone()),

            // Ours and theirs agree (both changed to same value or both added same value).
            (_, Some(o), Some(t)) if o == t => Some(o.clone()),

            // True conflict: both sides changed the same field to different values.
            // Resolve via JSONL changelog: find which side's current value is more recent.
            // The side whose value matches the most recent changelog entry wins.
            // If neither matches (both diverged from changelog), fall back to precedence.
            (_, Some(o), Some(t)) => {
                let winner =
                    resolve_conflict(key, o, t, &value_timestamps, opts.fallback_precedence);
                Some(winner.clone())
            }

            // Catch-all for asymmetric presence (one side present, other absent, base absent).
            (_, None, Some(t)) => Some(t.clone()),
            (_, Some(o), None) => Some(o.clone()),
        };

        if let Some(v) = chosen {
            result.insert(key.clone(), v);
        }
    }

    // Serialize result back to YAML.
    if result.is_empty() {
        return Ok(String::new());
    }

    let mapping: serde_yaml_ng::Mapping = result
        .into_iter()
        .map(|(k, v)| (serde_yaml_ng::Value::String(k), v))
        .collect();

    serde_yaml_ng::to_string(&serde_yaml_ng::Value::Mapping(mapping))
        .map_err(|e| MergeError::ParseFailure(format!("YAML serialize error: {e}")))
}

/// Resolve a field conflict between `ours` and `theirs` values.
///
/// Looks up each side's value in the JSONL changelog's most-recent-value map.  The side
/// whose value was set most recently wins.  If neither side's value appears in the
/// changelog (both diverged), `fallback_precedence` decides.
fn resolve_conflict<'a>(
    key: &str,
    ours: &'a serde_yaml_ng::Value,
    theirs: &'a serde_yaml_ng::Value,
    value_timestamps: &HashMap<String, (String, String)>,
    fallback_precedence: Precedence,
) -> &'a serde_yaml_ng::Value {
    // Convert a YAML Value to a string for comparison with JSON changelog values.
    fn yaml_val_to_string(v: &serde_yaml_ng::Value) -> String {
        match v {
            serde_yaml_ng::Value::String(s) => s.clone(),
            serde_yaml_ng::Value::Number(n) => n.to_string(),
            serde_yaml_ng::Value::Bool(b) => b.to_string(),
            serde_yaml_ng::Value::Null => "null".to_owned(),
            other => format!("{other:?}"),
        }
    }

    let ours_str = yaml_val_to_string(ours);
    let theirs_str = yaml_val_to_string(theirs);

    // The changelog records field → (most_recent_new_value, timestamp).
    // If our current value matches the logged new_value, it was set at that timestamp.
    if let Some((logged_val, logged_ts)) = value_timestamps.get(key) {
        let ours_matches = &ours_str == logged_val;
        let theirs_matches = &theirs_str == logged_val;
        match (ours_matches, theirs_matches) {
            // Only ours matches the most-recently-logged value → ours is the winner.
            (true, false) => return ours,
            // Only theirs matches → theirs is the winner.
            (false, true) => return theirs,
            // Both match the same logged value (shouldn't happen since ours != theirs) or
            // neither matches — fall through to precedence.
            _ => {
                // Neither side's value is the logged value. This means both sides diverged
                // from the last logged state. We cannot tell which is newer from the log alone.
                let _ = logged_ts; // suppress unused warning
            }
        }
    }

    // No changelog match — use fallback precedence.
    match fallback_precedence {
        Precedence::Ours => ours,
        Precedence::Theirs => theirs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_no_jsonl() -> MergeOpts {
        MergeOpts {
            jsonl_path: None,
            fallback_precedence: Precedence::Theirs,
        }
    }

    fn opts_ours_wins() -> MergeOpts {
        MergeOpts {
            jsonl_path: None,
            fallback_precedence: Precedence::Ours,
        }
    }

    /// Parse merged YAML back into a map for easy assertion.
    fn parse(yaml: &str) -> BTreeMap<String, String> {
        if yaml.trim().is_empty() {
            return BTreeMap::new();
        }
        let v: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
        match v {
            serde_yaml_ng::Value::Mapping(m) => m
                .into_iter()
                .map(|(k, v)| {
                    let key = match k {
                        serde_yaml_ng::Value::String(s) => s,
                        other => format!("{other:?}"),
                    };
                    let val = match v {
                        serde_yaml_ng::Value::String(s) => s,
                        serde_yaml_ng::Value::Number(n) => n.to_string(),
                        serde_yaml_ng::Value::Bool(b) => b.to_string(),
                        serde_yaml_ng::Value::Null => "null".to_owned(),
                        other => format!("{other:?}"),
                    };
                    (key, val)
                })
                .collect(),
            _ => BTreeMap::new(),
        }
    }

    /// Non-overlapping field changes auto-merge cleanly.
    #[test]
    fn non_overlapping_changes_auto_merge() {
        let base = "id: abc\ntitle: Original\ncolor: red\n";
        // Ours changed title; theirs changed color.
        let ours = "id: abc\ntitle: Updated\ncolor: red\n";
        let theirs = "id: abc\ntitle: Original\ncolor: blue\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);

        assert_eq!(map["id"], "abc");
        assert_eq!(map["title"], "Updated", "ours title change should be taken");
        assert_eq!(map["color"], "blue", "theirs color change should be taken");
    }

    /// When both sides change the same field with no JSONL, fallback precedence applies.
    #[test]
    fn conflict_without_jsonl_fallback_theirs() {
        let base = "id: abc\ntitle: Original\n";
        let ours = "id: abc\ntitle: OursTitle\n";
        let theirs = "id: abc\ntitle: TheirsTitle\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(map["title"], "TheirsTitle", "theirs should win by default");
    }

    /// Fallback precedence = Ours.
    #[test]
    fn conflict_without_jsonl_fallback_ours() {
        let base = "id: abc\ntitle: Original\n";
        let ours = "id: abc\ntitle: OursTitle\n";
        let theirs = "id: abc\ntitle: TheirsTitle\n";

        let merged = merge_yaml(base, ours, theirs, &opts_ours_wins()).unwrap();
        let map = parse(&merged);
        assert_eq!(
            map["title"], "OursTitle",
            "ours should win when precedence=Ours"
        );
    }

    /// Conflicting field resolved by JSONL changelog — the side whose value matches the
    /// most recent changelog entry wins.
    #[test]
    fn conflict_with_jsonl_theirs_value_matches_log() {
        use std::io::Write;

        // JSONL changelog records that "title" was last set to "TheirsTitle".
        // Even though fallback_precedence = Ours, the changelog should cause theirs to win
        // because "TheirsTitle" is the most recently logged new_value for the "title" field.
        let dir = tempfile::tempdir().unwrap();
        let jsonl_path = dir.path().join("task.jsonl");
        let entry = r#"{"id":"01AAA000000000000000000001","timestamp":"2026-03-01T12:00:00Z","op":"update","entity_type":"task","entity_id":"abc","changes":[["title",{"kind":"changed","old_value":"Original","new_value":"TheirsTitle"}]]}"#;
        let mut f = std::fs::File::create(&jsonl_path).unwrap();
        writeln!(f, "{entry}").unwrap();

        let base = "id: abc\ntitle: Original\n";
        let ours = "id: abc\ntitle: OursTitle\n";
        let theirs = "id: abc\ntitle: TheirsTitle\n";

        let opts = MergeOpts {
            jsonl_path: Some(jsonl_path),
            fallback_precedence: Precedence::Ours, // without JSONL, ours would win
        };

        let merged = merge_yaml(base, ours, theirs, &opts).unwrap();
        let map = parse(&merged);
        // The changelog's most recent entry set title to "TheirsTitle", so theirs wins.
        assert_eq!(
            map["title"], "TheirsTitle",
            "JSONL shows TheirsTitle is the most recently logged value, so theirs should win"
        );
    }

    /// Conflicting field resolved by JSONL changelog — ours value matches log, so ours wins
    /// even though fallback_precedence = Theirs.
    #[test]
    fn conflict_with_jsonl_ours_value_matches_log() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let jsonl_path = dir.path().join("task.jsonl");
        let entry = r#"{"id":"01AAA000000000000000000001","timestamp":"2026-03-01T12:00:00Z","op":"update","entity_type":"task","entity_id":"abc","changes":[["title",{"kind":"changed","old_value":"Original","new_value":"OursTitle"}]]}"#;
        let mut f = std::fs::File::create(&jsonl_path).unwrap();
        writeln!(f, "{entry}").unwrap();

        let base = "id: abc\ntitle: Original\n";
        let ours = "id: abc\ntitle: OursTitle\n";
        let theirs = "id: abc\ntitle: TheirsTitle\n";

        let opts = MergeOpts {
            jsonl_path: Some(jsonl_path),
            fallback_precedence: Precedence::Theirs, // without JSONL, theirs would win
        };

        let merged = merge_yaml(base, ours, theirs, &opts).unwrap();
        let map = parse(&merged);
        // The changelog's most recent entry set title to "OursTitle", so ours wins.
        assert_eq!(
            map["title"], "OursTitle",
            "JSONL shows OursTitle is the most recently logged value, so ours should win"
        );
    }

    /// When neither side's value matches the changelog, fallback precedence applies.
    #[test]
    fn conflict_with_jsonl_neither_matches_uses_fallback() {
        use std::io::Write;

        // Changelog shows a completely different value for title (neither ours nor theirs).
        let dir = tempfile::tempdir().unwrap();
        let jsonl_path = dir.path().join("task.jsonl");
        let entry = r#"{"id":"01AAA000000000000000000001","timestamp":"2026-03-01T12:00:00Z","op":"update","entity_type":"task","entity_id":"abc","changes":[["title",{"kind":"changed","old_value":"Original","new_value":"SomethingElse"}]]}"#;
        let mut f = std::fs::File::create(&jsonl_path).unwrap();
        writeln!(f, "{entry}").unwrap();

        let base = "id: abc\ntitle: Original\n";
        let ours = "id: abc\ntitle: OursTitle\n";
        let theirs = "id: abc\ntitle: TheirsTitle\n";

        let opts = MergeOpts {
            jsonl_path: Some(jsonl_path),
            fallback_precedence: Precedence::Ours,
        };

        let merged = merge_yaml(base, ours, theirs, &opts).unwrap();
        let map = parse(&merged);
        // Neither side's value is in the changelog → fall back to Precedence::Ours.
        assert_eq!(
            map["title"], "OursTitle",
            "neither side matches changelog, so fallback_precedence=Ours should apply"
        );
    }

    /// Field added on one side only is taken.
    #[test]
    fn field_addition_one_side() {
        let base = "id: abc\n";
        let ours = "id: abc\ntitle: NewTitle\n";
        let theirs = "id: abc\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(map["id"], "abc");
        assert_eq!(map["title"], "NewTitle", "ours addition should be taken");
    }

    /// Field removed on one side only is removed from the result.
    #[test]
    fn field_removal_one_side() {
        let base = "id: abc\ntitle: OldTitle\n";
        // Ours removed title; theirs kept it unchanged.
        let ours = "id: abc\n";
        let theirs = "id: abc\ntitle: OldTitle\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(map["id"], "abc");
        assert!(
            !map.contains_key("title"),
            "title removed by ours should be absent"
        );
    }

    /// Empty inputs: base and ours and theirs all empty → empty output.
    #[test]
    fn empty_inputs() {
        let merged = merge_yaml("", "", "", &opts_no_jsonl()).unwrap();
        assert_eq!(merged.trim(), "");
    }

    /// When all three agree, the result matches base.
    #[test]
    fn no_changes_returns_base_content() {
        let base = "id: abc\ntitle: Stable\n";
        let merged = merge_yaml(base, base, base, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(map["id"], "abc");
        assert_eq!(map["title"], "Stable");
    }

    // --- Direct unit tests for parse_changelog ---

    /// Empty string input returns no entries.
    #[test]
    fn parse_changelog_empty_input() {
        let entries = parse_changelog("");
        assert!(entries.is_empty(), "empty input should yield no entries");
    }

    /// Input with only whitespace and blank lines returns no entries.
    #[test]
    fn parse_changelog_whitespace_only() {
        let entries = parse_changelog("   \n\n   \n");
        assert!(
            entries.is_empty(),
            "whitespace-only input should yield no entries"
        );
    }

    /// Invalid JSON lines are silently skipped.
    #[test]
    fn parse_changelog_invalid_json_skipped() {
        let content = "not json at all\n{also not json\n";
        let entries = parse_changelog(content);
        assert!(
            entries.is_empty(),
            "invalid JSON lines should be silently skipped"
        );
    }

    /// A line with valid JSON but missing `timestamp` field is skipped.
    #[test]
    fn parse_changelog_missing_timestamp_skipped() {
        let content =
            r#"{"op":"update","changes":[["title",{"kind":"changed","new_value":"New"}]]}"#;
        let entries = parse_changelog(content);
        assert!(
            entries.is_empty(),
            "entry missing `timestamp` field should be skipped"
        );
    }

    /// A line with valid JSON but missing `changes` field produces an entry with empty changes vec.
    #[test]
    fn parse_changelog_missing_changes_produces_empty_vec() {
        let content = r#"{"timestamp":"2026-03-01T12:00:00Z","op":"update"}"#;
        let entries = parse_changelog(content);
        assert_eq!(
            entries.len(),
            1,
            "entry with missing `changes` should still be parsed"
        );
        assert_eq!(
            entries[0].timestamp, "2026-03-01T12:00:00Z",
            "timestamp should be captured"
        );
        assert!(
            entries[0].changes.is_empty(),
            "missing `changes` field should produce empty changes vec"
        );
    }

    /// A valid single-entry changelog is parsed correctly.
    #[test]
    fn parse_changelog_single_valid_entry() {
        let content = r#"{"timestamp":"2026-03-01T12:00:00Z","op":"update","changes":[["title",{"kind":"changed","old_value":"Old","new_value":"New"}]]}"#;
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, "2026-03-01T12:00:00Z");
        assert_eq!(entries[0].changes.len(), 1);
        assert_eq!(entries[0].changes[0].field, "title");
        assert_eq!(
            entries[0].changes[0].new_value,
            Some(serde_json::Value::String("New".to_owned()))
        );
    }

    /// A change entry without `new_value` field produces a FieldChange with None new_value.
    #[test]
    fn parse_changelog_change_without_new_value() {
        let content = r#"{"timestamp":"2026-03-01T12:00:00Z","op":"delete","changes":[["title",{"kind":"removed","old_value":"Old"}]]}"#;
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].changes.len(), 1);
        assert_eq!(entries[0].changes[0].field, "title");
        assert!(
            entries[0].changes[0].new_value.is_none(),
            "change without `new_value` should produce None"
        );
    }

    /// Multiple entries are all parsed and returned in file order.
    #[test]
    fn parse_changelog_multiple_entries_in_order() {
        let content = concat!(
            r#"{"timestamp":"2026-03-01T10:00:00Z","op":"update","changes":[["color",{"kind":"changed","new_value":"red"}]]}"#,
            "\n",
            r#"{"timestamp":"2026-03-01T11:00:00Z","op":"update","changes":[["color",{"kind":"changed","new_value":"blue"}],["title",{"kind":"changed","new_value":"Updated"}]]}"#,
            "\n",
            r#"{"timestamp":"2026-03-01T12:00:00Z","op":"update","changes":[["title",{"kind":"changed","new_value":"Final"}]]}"#,
        );
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 3, "all three entries should be parsed");
        assert_eq!(entries[0].timestamp, "2026-03-01T10:00:00Z");
        assert_eq!(entries[1].timestamp, "2026-03-01T11:00:00Z");
        assert_eq!(entries[2].timestamp, "2026-03-01T12:00:00Z");
        // Second entry has two changes.
        assert_eq!(entries[1].changes.len(), 2);
        assert_eq!(entries[1].changes[0].field, "color");
        assert_eq!(entries[1].changes[1].field, "title");
        // Third entry has one change.
        assert_eq!(entries[2].changes.len(), 1);
        assert_eq!(entries[2].changes[0].field, "title");
        assert_eq!(
            entries[2].changes[0].new_value,
            Some(serde_json::Value::String("Final".to_owned()))
        );
    }

    /// Mixed valid and invalid lines — valid ones are parsed, invalid ones skipped.
    #[test]
    fn parse_changelog_mixed_valid_and_invalid_lines() {
        let content = concat!(
            "this is garbage\n",
            r#"{"timestamp":"2026-03-01T10:00:00Z","op":"update","changes":[["title",{"kind":"changed","new_value":"A"}]]}"#,
            "\n",
            "{broken json\n",
            r#"{"timestamp":"2026-03-01T11:00:00Z","op":"update","changes":[["title",{"kind":"changed","new_value":"B"}]]}"#,
            "\n",
        );
        let entries = parse_changelog(content);
        assert_eq!(entries.len(), 2, "only valid entries should be returned");
        assert_eq!(entries[0].timestamp, "2026-03-01T10:00:00Z");
        assert_eq!(entries[1].timestamp, "2026-03-01T11:00:00Z");
    }

    // --- Direct unit tests for latest_value_timestamps ---

    /// An empty entries slice returns an empty map.
    #[test]
    fn latest_value_timestamps_empty_entries() {
        let result = latest_value_timestamps(&[]);
        assert!(result.is_empty(), "empty entries should produce empty map");
    }

    /// A single entry with a single field change is captured correctly.
    #[test]
    fn latest_value_timestamps_single_entry_single_field() {
        let entries = vec![ChangelogEntry {
            timestamp: "2026-01-01T00:00:00Z".to_owned(),
            changes: vec![FieldChange {
                field: "title".to_owned(),
                new_value: Some(serde_json::Value::String("Hello".to_owned())),
            }],
        }];
        let result = latest_value_timestamps(&entries);
        assert_eq!(result.len(), 1);
        let (val, ts) = &result["title"];
        assert_eq!(val, "Hello");
        assert_eq!(ts, "2026-01-01T00:00:00Z");
    }

    /// When multiple entries touch the same field, the one with the later timestamp wins.
    #[test]
    fn latest_value_timestamps_later_timestamp_wins() {
        let entries = vec![
            ChangelogEntry {
                timestamp: "2026-01-01T00:00:00Z".to_owned(),
                changes: vec![FieldChange {
                    field: "color".to_owned(),
                    new_value: Some(serde_json::Value::String("red".to_owned())),
                }],
            },
            ChangelogEntry {
                timestamp: "2026-06-01T00:00:00Z".to_owned(),
                changes: vec![FieldChange {
                    field: "color".to_owned(),
                    new_value: Some(serde_json::Value::String("blue".to_owned())),
                }],
            },
            ChangelogEntry {
                timestamp: "2026-03-01T00:00:00Z".to_owned(),
                changes: vec![FieldChange {
                    field: "color".to_owned(),
                    new_value: Some(serde_json::Value::String("green".to_owned())),
                }],
            },
        ];
        let result = latest_value_timestamps(&entries);
        let (val, ts) = &result["color"];
        assert_eq!(
            val, "blue",
            "the entry with the latest timestamp should win"
        );
        assert_eq!(ts, "2026-06-01T00:00:00Z");
    }

    /// Multiple distinct fields across entries are all recorded independently.
    #[test]
    fn latest_value_timestamps_multiple_fields() {
        let entries = vec![
            ChangelogEntry {
                timestamp: "2026-01-01T00:00:00Z".to_owned(),
                changes: vec![
                    FieldChange {
                        field: "title".to_owned(),
                        new_value: Some(serde_json::Value::String("First".to_owned())),
                    },
                    FieldChange {
                        field: "color".to_owned(),
                        new_value: Some(serde_json::Value::String("red".to_owned())),
                    },
                ],
            },
            ChangelogEntry {
                timestamp: "2026-02-01T00:00:00Z".to_owned(),
                changes: vec![FieldChange {
                    field: "title".to_owned(),
                    new_value: Some(serde_json::Value::String("Second".to_owned())),
                }],
            },
        ];
        let result = latest_value_timestamps(&entries);
        assert_eq!(result.len(), 2, "two distinct fields should be present");
        assert_eq!(result["title"].0, "Second", "title: later entry should win");
        assert_eq!(
            result["color"].0, "red",
            "color: only one entry, should be red"
        );
    }

    /// Entries whose changes have `new_value = None` are ignored for that field.
    #[test]
    fn latest_value_timestamps_ignores_changes_without_new_value() {
        let entries = vec![ChangelogEntry {
            timestamp: "2026-01-01T00:00:00Z".to_owned(),
            changes: vec![
                FieldChange {
                    field: "title".to_owned(),
                    new_value: None, // no new_value → should not appear in map
                },
                FieldChange {
                    field: "color".to_owned(),
                    new_value: Some(serde_json::Value::String("green".to_owned())),
                },
            ],
        }];
        let result = latest_value_timestamps(&entries);
        assert!(
            !result.contains_key("title"),
            "field with no new_value should be absent from map"
        );
        assert!(
            result.contains_key("color"),
            "field with new_value should be present"
        );
    }

    /// Non-string JSON values (numbers, booleans) are serialised to their JSON string form.
    #[test]
    fn latest_value_timestamps_non_string_values_serialised() {
        let entries = vec![ChangelogEntry {
            timestamp: "2026-01-01T00:00:00Z".to_owned(),
            changes: vec![
                FieldChange {
                    field: "count".to_owned(),
                    new_value: Some(serde_json::Value::Number(42.into())),
                },
                FieldChange {
                    field: "active".to_owned(),
                    new_value: Some(serde_json::Value::Bool(true)),
                },
            ],
        }];
        let result = latest_value_timestamps(&entries);
        assert_eq!(result["count"].0, "42");
        assert_eq!(result["active"].0, "true");
    }

    // --- ParseFailure path tests for merge_yaml ---

    /// ParseFailure when base is a YAML sequence (not a mapping).
    #[test]
    fn parse_failure_base_not_a_mapping() {
        let base = "- item1\n- item2\n"; // YAML list, not a mapping
        let ours = "id: abc\n";
        let theirs = "id: abc\n";

        let result = merge_yaml(base, ours, theirs, &opts_no_jsonl());
        assert!(
            matches!(result, Err(MergeError::ParseFailure(_))),
            "base that is not a YAML mapping should return ParseFailure, got: {result:?}"
        );
    }

    /// ParseFailure when ours is a YAML sequence (not a mapping).
    #[test]
    fn parse_failure_ours_not_a_mapping() {
        let base = "id: abc\n";
        let ours = "- item1\n- item2\n"; // YAML list, not a mapping
        let theirs = "id: abc\n";

        let result = merge_yaml(base, ours, theirs, &opts_no_jsonl());
        assert!(
            matches!(result, Err(MergeError::ParseFailure(_))),
            "ours that is not a YAML mapping should return ParseFailure, got: {result:?}"
        );
    }

    /// ParseFailure when theirs is a YAML sequence (not a mapping).
    #[test]
    fn parse_failure_theirs_not_a_mapping() {
        let base = "id: abc\n";
        let ours = "id: abc\n";
        let theirs = "- item1\n- item2\n"; // YAML list, not a mapping

        let result = merge_yaml(base, ours, theirs, &opts_no_jsonl());
        assert!(
            matches!(result, Err(MergeError::ParseFailure(_))),
            "theirs that is not a YAML mapping should return ParseFailure, got: {result:?}"
        );
    }

    /// ParseFailure when base contains structurally invalid YAML (syntax error).
    #[test]
    fn parse_failure_base_invalid_yaml_syntax() {
        // Intentionally malformed: unclosed bracket makes the parser fail.
        let base = "id: [unclosed";
        let ours = "id: abc\n";
        let theirs = "id: abc\n";

        let result = merge_yaml(base, ours, theirs, &opts_no_jsonl());
        assert!(
            matches!(result, Err(MergeError::ParseFailure(_))),
            "base with invalid YAML syntax should return ParseFailure, got: {result:?}"
        );
    }

    /// Non-string YAML keys (e.g. integer keys) are coerced to a string via Debug format.
    ///
    /// This exercises the `format!("{k:?}")` branch inside `parse_mapping`.
    #[test]
    fn non_string_keys_are_coerced_to_string() {
        // Integer keys are valid YAML but represented as Number values, not String values.
        let base = "1: one\n2: two\n";
        let ours = "1: one\n2: two\n";
        let theirs = "1: one\n2: two\n";

        // Should succeed and produce a result with the keys serialised as strings.
        let result = merge_yaml(base, ours, theirs, &opts_no_jsonl());
        assert!(
            result.is_ok(),
            "non-string YAML keys should be handled without error, got: {result:?}"
        );
    }

    /// A YAML null document (`~`) is treated as an empty mapping, not a ParseFailure.
    ///
    /// This exercises the `serde_yaml_ng::Value::Null => Ok(BTreeMap::new())` branch.
    #[test]
    fn null_document_treated_as_empty_mapping() {
        // "~" is the YAML null literal — parse_mapping should return an empty BTreeMap.
        let base = "~";
        let ours = "id: abc\n";
        let theirs = "~";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        // Ours added "id: abc" over a null base; theirs stayed null → take ours' addition.
        assert_eq!(
            map.get("id").map(String::as_str),
            Some("abc"),
            "field added over a null base should appear in the merge result"
        );
    }

    // --- Direct unit tests for resolve_conflict ---

    /// Helper to build a value_timestamps map with a single entry.
    fn single_ts(key: &str, val: &str, ts: &str) -> HashMap<String, (String, String)> {
        let mut m = HashMap::new();
        m.insert(key.to_owned(), (val.to_owned(), ts.to_owned()));
        m
    }

    /// When ours matches the logged value and theirs does not, ours wins regardless of
    /// fallback precedence.
    #[test]
    fn resolve_conflict_ours_matches_log_wins() {
        let ours = serde_yaml_ng::Value::String("OursValue".to_owned());
        let theirs = serde_yaml_ng::Value::String("TheirsValue".to_owned());
        let ts = single_ts("title", "OursValue", "2026-01-01T00:00:00Z");

        // Even with Precedence::Theirs, the changelog should cause ours to win.
        let winner = resolve_conflict("title", &ours, &theirs, &ts, Precedence::Theirs);
        assert_eq!(
            winner, &ours,
            "ours matches the logged value so ours should win"
        );
    }

    /// When theirs matches the logged value and ours does not, theirs wins regardless of
    /// fallback precedence.
    #[test]
    fn resolve_conflict_theirs_matches_log_wins() {
        let ours = serde_yaml_ng::Value::String("OursValue".to_owned());
        let theirs = serde_yaml_ng::Value::String("TheirsValue".to_owned());
        let ts = single_ts("title", "TheirsValue", "2026-01-01T00:00:00Z");

        // Even with Precedence::Ours, the changelog should cause theirs to win.
        let winner = resolve_conflict("title", &ours, &theirs, &ts, Precedence::Ours);
        assert_eq!(
            winner, &theirs,
            "theirs matches the logged value so theirs should win"
        );
    }

    /// When neither side matches the logged value, fallback_precedence = Ours applies.
    #[test]
    fn resolve_conflict_neither_matches_fallback_ours() {
        let ours = serde_yaml_ng::Value::String("OursValue".to_owned());
        let theirs = serde_yaml_ng::Value::String("TheirsValue".to_owned());
        // Changelog has a completely different value.
        let ts = single_ts("title", "SomethingElse", "2026-01-01T00:00:00Z");

        let winner = resolve_conflict("title", &ours, &theirs, &ts, Precedence::Ours);
        assert_eq!(
            winner, &ours,
            "neither side matches the log so fallback_precedence=Ours should win"
        );
    }

    /// When neither side matches the logged value, fallback_precedence = Theirs applies.
    #[test]
    fn resolve_conflict_neither_matches_fallback_theirs() {
        let ours = serde_yaml_ng::Value::String("OursValue".to_owned());
        let theirs = serde_yaml_ng::Value::String("TheirsValue".to_owned());
        // Changelog has a completely different value.
        let ts = single_ts("title", "SomethingElse", "2026-01-01T00:00:00Z");

        let winner = resolve_conflict("title", &ours, &theirs, &ts, Precedence::Theirs);
        assert_eq!(
            winner, &theirs,
            "neither side matches the log so fallback_precedence=Theirs should win"
        );
    }

    /// When no changelog entry exists for the key, fallback_precedence decides.
    #[test]
    fn resolve_conflict_no_entry_for_key_fallback_ours() {
        let ours = serde_yaml_ng::Value::String("OursValue".to_owned());
        let theirs = serde_yaml_ng::Value::String("TheirsValue".to_owned());
        // Changelog has an entry for a *different* key.
        let ts = single_ts("color", "red", "2026-01-01T00:00:00Z");

        let winner = resolve_conflict("title", &ours, &theirs, &ts, Precedence::Ours);
        assert_eq!(
            winner, &ours,
            "no changelog entry for this key — fallback_precedence=Ours should win"
        );
    }

    /// When no changelog entry exists for the key and fallback is Theirs, theirs wins.
    #[test]
    fn resolve_conflict_no_entry_for_key_fallback_theirs() {
        let ours = serde_yaml_ng::Value::String("OursValue".to_owned());
        let theirs = serde_yaml_ng::Value::String("TheirsValue".to_owned());
        let ts: HashMap<String, (String, String)> = HashMap::new(); // completely empty

        let winner = resolve_conflict("title", &ours, &theirs, &ts, Precedence::Theirs);
        assert_eq!(
            winner, &theirs,
            "empty changelog — fallback_precedence=Theirs should win"
        );
    }

    /// Field removed by theirs only (ours unchanged from base).
    ///
    /// This exercises the `(Some(b), Some(o), None) if o == b => None` arm.
    #[test]
    fn field_removal_by_theirs_only() {
        let base = "id: abc\ntitle: OldTitle\n";
        // Theirs removed title; ours kept it unchanged.
        let ours = "id: abc\ntitle: OldTitle\n";
        let theirs = "id: abc\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(map["id"], "abc");
        assert!(
            !map.contains_key("title"),
            "title removed by theirs should be absent"
        );
    }

    /// Both sides independently add the same field with the same value (base has no such
    /// field).
    ///
    /// This exercises the `(_, Some(o), Some(t)) if o == t` arm with base=None.
    #[test]
    fn both_add_same_field_same_value() {
        let base = "id: abc\n";
        let ours = "id: abc\ntitle: SameTitle\n";
        let theirs = "id: abc\ntitle: SameTitle\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(
            map["title"], "SameTitle",
            "both sides added same value — should keep it"
        );
    }

    /// Both sides removed a field that was present in base.
    ///
    /// This exercises the `(_, None, None) => None` arm with base=Some.
    #[test]
    fn both_removed_field() {
        let base = "id: abc\ntitle: OldTitle\n";
        let ours = "id: abc\n";
        let theirs = "id: abc\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert!(
            !map.contains_key("title"),
            "field removed by both sides should be absent"
        );
    }

    /// Ours removed a field but theirs changed it from base — catch-all arm takes theirs.
    ///
    /// This exercises the catch-all `(_, None, Some(t))` arm (base present, ours absent,
    /// theirs changed from base so the guard `t == b` on the earlier arm fails).
    #[test]
    fn ours_removed_theirs_changed_catch_all() {
        let base = "id: abc\ntitle: Original\n";
        // Ours removed title entirely.
        let ours = "id: abc\n";
        // Theirs changed title to a new value.
        let theirs = "id: abc\ntitle: Changed\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(
            map["title"], "Changed",
            "catch-all should keep theirs' changed value when ours removed"
        );
    }

    /// Theirs removed a field but ours changed it from base — catch-all arm takes ours.
    ///
    /// This exercises the catch-all `(_, Some(o), None)` arm (base present, theirs absent,
    /// ours changed from base so the guard `o == b` on the earlier arm fails).
    #[test]
    fn theirs_removed_ours_changed_catch_all() {
        let base = "id: abc\ntitle: Original\n";
        // Ours changed title.
        let ours = "id: abc\ntitle: Changed\n";
        // Theirs removed title entirely.
        let theirs = "id: abc\n";

        let merged = merge_yaml(base, ours, theirs, &opts_no_jsonl()).unwrap();
        let map = parse(&merged);
        assert_eq!(
            map["title"], "Changed",
            "catch-all should keep ours' changed value when theirs removed"
        );
    }

    /// Resolve conflict with numeric YAML values exercises the Number branch of
    /// `yaml_val_to_string`.
    #[test]
    fn resolve_conflict_with_numeric_values() {
        let ours = serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(42));
        let theirs = serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(99));
        // Changelog says the field was last set to "42" (matches ours).
        let ts = single_ts("count", "42", "2026-01-01T00:00:00Z");

        let winner = resolve_conflict("count", &ours, &theirs, &ts, Precedence::Theirs);
        assert_eq!(
            winner, &ours,
            "ours numeric value matches the log so ours should win"
        );
    }

    /// Resolve conflict with boolean YAML values exercises the Bool branch of
    /// `yaml_val_to_string`.
    #[test]
    fn resolve_conflict_with_bool_values() {
        let ours = serde_yaml_ng::Value::Bool(true);
        let theirs = serde_yaml_ng::Value::Bool(false);
        // Changelog says the field was last set to "false" (matches theirs).
        let ts = single_ts("active", "false", "2026-01-01T00:00:00Z");

        let winner = resolve_conflict("active", &ours, &theirs, &ts, Precedence::Ours);
        assert_eq!(
            winner, &theirs,
            "theirs bool value matches the log so theirs should win"
        );
    }

    /// Resolve conflict with null YAML value exercises the Null branch of
    /// `yaml_val_to_string`.
    #[test]
    fn resolve_conflict_with_null_value() {
        let ours = serde_yaml_ng::Value::Null;
        let theirs = serde_yaml_ng::Value::String("something".to_owned());
        // Changelog says the field was last set to "null" (matches ours).
        let ts = single_ts("optional", "null", "2026-01-01T00:00:00Z");

        let winner = resolve_conflict("optional", &ours, &theirs, &ts, Precedence::Theirs);
        assert_eq!(
            winner, &ours,
            "ours null value matches the log so ours should win"
        );
    }

    /// Resolve conflict with a complex YAML value (sequence) exercises the catch-all
    /// `other => format!("{other:?}")` branch of `yaml_val_to_string`.
    #[test]
    fn resolve_conflict_with_complex_value_debug_format() {
        // A YAML sequence value hits the catch-all Debug format branch.
        let ours =
            serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::String("a".to_owned())]);
        let theirs =
            serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::String("b".to_owned())]);
        // No changelog entry for this field — fallback applies.
        let ts: HashMap<String, (String, String)> = HashMap::new();

        let winner = resolve_conflict("tags", &ours, &theirs, &ts, Precedence::Ours);
        assert_eq!(
            winner, &ours,
            "no log match, fallback=Ours, so ours should win"
        );
    }

    /// When both ours and theirs have the same value as the logged entry, neither branch
    /// fires and fallback_precedence decides.  This exercises the `_ =>` arm.
    #[test]
    fn resolve_conflict_both_match_same_log_value_fallback_applies() {
        // This is the degenerate case where ours == theirs == logged_val.
        // The function is only called for conflicting keys (ours != theirs in practice),
        // but the code handles it defensively by falling through to the fallback.
        let shared = serde_yaml_ng::Value::String("SameValue".to_owned());
        let ts = single_ts("title", "SameValue", "2026-01-01T00:00:00Z");

        // Since both match, the (true, true) arm falls through to the fallback.
        let winner = resolve_conflict("title", &shared, &shared, &ts, Precedence::Theirs);
        // Both are identical so either pointer is correct; we just verify no panic and
        // that the result equals the shared value.
        assert_eq!(
            winner.as_str().unwrap(),
            "SameValue",
            "both match the same logged value — fallback applies but result is either side"
        );
    }
}
