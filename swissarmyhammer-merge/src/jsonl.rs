use std::collections::{BTreeMap, HashSet};

use crate::{MergeConflict, MergeError};

/// Merge three JSONL inputs (base, ours, theirs) using union-by-id strategy.
///
/// Each line must be a JSON object with an `"id"` field (ULID string).
/// Lines are deduplicated by id and sorted lexicographically (ULID = chronological).
///
/// # Arguments
/// - `base` — the common ancestor content
/// - `ours` — our branch's content
/// - `theirs` — their branch's content
///
/// # Returns
/// - `Ok(merged_string)` on success.
/// - `Err(MergeError::Conflict)` if the same id appears in both ours and theirs with
///   different content (both added independently from base).
///
/// Note: JSONL lines that fail JSON parsing are silently skipped (no `ParseFailure` is
/// raised) because a corrupt changelog entry should not prevent the merge from succeeding.
pub fn merge_jsonl(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError> {
    /// Extract the `"id"` field from a JSON line, returning `None` if missing or invalid.
    fn extract_id(line: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        value["id"].as_str().map(|s| s.to_owned())
    }

    // BTreeMap keyed by id preserves ULID lexicographic (= chronological) order.
    let mut merged: BTreeMap<String, String> = BTreeMap::new();

    // Track ids that came from base.
    let mut base_ids: HashSet<String> = HashSet::new();

    // Insert base lines first.
    for line in base.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(id) = extract_id(line) {
            base_ids.insert(id.clone());
            merged.insert(id, line.to_owned());
        }
    }

    // Track ids that are new from ours (not present in base).
    let mut ours_new_ids: HashSet<String> = HashSet::new();

    // Insert ours lines.
    for line in ours.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(id) = extract_id(line) {
            if !base_ids.contains(&id) {
                ours_new_ids.insert(id.clone());
            }
            merged.insert(id, line.to_owned());
        }
    }

    // Collect conflicts before inserting theirs.
    let mut conflicting_ids: Vec<String> = Vec::new();

    // Insert theirs lines, detecting conflicts.
    for line in theirs.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(id) = extract_id(line) {
            if ours_new_ids.contains(&id) {
                // Both ours and theirs added this id independently — check for content diff.
                if let Some(existing) = merged.get(&id) {
                    if existing.as_str() != line {
                        conflicting_ids.push(id.clone());
                    }
                }
            }
            merged.insert(id, line.to_owned());
        }
    }

    if !conflicting_ids.is_empty() {
        conflicting_ids.sort();
        return Err(MergeError::Conflict(MergeConflict { conflicting_ids }));
    }

    if merged.is_empty() {
        return Ok(String::new());
    }

    let mut output = merged.values().cloned().collect::<Vec<_>>().join("\n");
    output.push('\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Realistic JSONL entries using ULID-style ids.
    const ENTRY_A: &str = r#"{"id":"01AAA000000000000000000000","timestamp":"2026-01-01T00:00:00Z","op":"create","changes":[]}"#;
    const ENTRY_B: &str = r#"{"id":"01AAA000000000000000000001","timestamp":"2026-01-01T01:00:00Z","op":"update","changes":[]}"#;
    const ENTRY_C: &str = r#"{"id":"01AAA000000000000000000002","timestamp":"2026-01-01T02:00:00Z","op":"create","changes":[]}"#;
    const ENTRY_D: &str = r#"{"id":"01AAA000000000000000000003","timestamp":"2026-01-01T03:00:00Z","op":"delete","changes":[]}"#;
    // Two entries with the same id X but different content.
    const ENTRY_X1: &str = r#"{"id":"01BBB000000000000000000000","timestamp":"2026-01-02T00:00:00Z","op":"create","changes":["v1"]}"#;
    const ENTRY_X2: &str = r#"{"id":"01BBB000000000000000000000","timestamp":"2026-01-02T00:00:00Z","op":"create","changes":["v2"]}"#;

    fn lines(entries: &[&str]) -> String {
        let mut s = entries.join("\n");
        if !s.is_empty() {
            s.push('\n');
        }
        s
    }

    /// Disjoint appends: base has A,B; ours adds C; theirs adds D.
    /// Result should contain A,B,C,D sorted by id.
    #[test]
    fn disjoint_appends() {
        let base = lines(&[ENTRY_A, ENTRY_B]);
        let ours = lines(&[ENTRY_A, ENTRY_B, ENTRY_C]);
        let theirs = lines(&[ENTRY_A, ENTRY_B, ENTRY_D]);

        let result = merge_jsonl(&base, &ours, &theirs).expect("no conflict expected");
        let result_lines: Vec<&str> = result.lines().collect();

        assert_eq!(result_lines.len(), 4, "should have 4 entries");
        assert!(result.contains(ENTRY_A));
        assert!(result.contains(ENTRY_B));
        assert!(result.contains(ENTRY_C));
        assert!(result.contains(ENTRY_D));

        // Verify sorted order (C id < D id by construction).
        let c_pos = result.find(ENTRY_C).unwrap();
        let d_pos = result.find(ENTRY_D).unwrap();
        assert!(c_pos < d_pos, "C should appear before D");
    }

    /// Identical overlap: base has A; ours has A,B; theirs has A,C.
    /// Result: A,B,C (A deduplicated).
    #[test]
    fn identical_overlap() {
        let base = lines(&[ENTRY_A]);
        let ours = lines(&[ENTRY_A, ENTRY_B]);
        let theirs = lines(&[ENTRY_A, ENTRY_C]);

        let result = merge_jsonl(&base, &ours, &theirs).expect("no conflict expected");
        let result_lines: Vec<&str> = result.lines().collect();

        assert_eq!(result_lines.len(), 3, "should have 3 entries");
        assert!(result.contains(ENTRY_A));
        assert!(result.contains(ENTRY_B));
        assert!(result.contains(ENTRY_C));
    }

    /// Conflict: ours adds X with content-1, theirs adds X with content-2.
    /// Returns Err(MergeError::Conflict).
    #[test]
    fn conflict_different_content() {
        let base = lines(&[ENTRY_A]);
        let ours = lines(&[ENTRY_A, ENTRY_X1]);
        let theirs = lines(&[ENTRY_A, ENTRY_X2]);

        let err = merge_jsonl(&base, &ours, &theirs).expect_err("conflict expected");
        let conflict = match err {
            crate::MergeError::Conflict(c) => c,
            other => panic!("expected MergeError::Conflict, got: {other:?}"),
        };
        assert_eq!(conflict.conflicting_ids, vec!["01BBB000000000000000000000"]);
        assert!(conflict.to_string().contains("01BBB000000000000000000000"));
    }

    /// Empty inputs: all empty → empty output.
    /// Base with content, ours/theirs empty → base preserved.
    #[test]
    fn empty_inputs() {
        // All empty.
        let result = merge_jsonl("", "", "").expect("no conflict");
        assert_eq!(result, "", "all empty should produce empty");

        // Base with content, ours and theirs empty.
        let base = lines(&[ENTRY_A, ENTRY_B]);
        let result = merge_jsonl(&base, "", "").expect("no conflict");
        let result_lines: Vec<&str> = result.lines().collect();
        assert_eq!(result_lines.len(), 2);
        assert!(result.contains(ENTRY_A));
        assert!(result.contains(ENTRY_B));
    }

    /// Entries inserted out of ULID order come out sorted lexicographically.
    #[test]
    fn sorting() {
        // Insert D, B, A, C in that order across ours/theirs.
        let base = lines(&[ENTRY_D, ENTRY_B]);
        let ours = lines(&[ENTRY_D, ENTRY_B, ENTRY_A]);
        let theirs = lines(&[ENTRY_D, ENTRY_B, ENTRY_C]);

        let result = merge_jsonl(&base, &ours, &theirs).expect("no conflict");
        let result_lines: Vec<&str> = result.lines().collect();

        assert_eq!(result_lines.len(), 4);
        // ULIDs sort lexicographically: A < B < C < D by suffix 0,1,2,3.
        assert_eq!(result_lines[0], ENTRY_A);
        assert_eq!(result_lines[1], ENTRY_B);
        assert_eq!(result_lines[2], ENTRY_C);
        assert_eq!(result_lines[3], ENTRY_D);
    }
}
