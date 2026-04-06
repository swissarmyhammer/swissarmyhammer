use rayon::prelude::*;
use serde::Serialize;

use crate::git_types::FileChange;
use crate::model::change::{ChangeType, SemanticChange};
use crate::model::identity::match_entities;
use crate::parser::registry::ParserRegistry;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub changes: Vec<SemanticChange>,
    pub file_count: usize,
    pub added_count: usize,
    pub modified_count: usize,
    pub deleted_count: usize,
    pub moved_count: usize,
    pub renamed_count: usize,
}

pub fn compute_semantic_diff(
    file_changes: &[FileChange],
    registry: &ParserRegistry,
    commit_sha: Option<&str>,
    author: Option<&str>,
) -> DiffResult {
    // Process files in parallel: each file's entity extraction and matching is independent
    let per_file_changes: Vec<(String, Vec<SemanticChange>)> = file_changes
        .par_iter()
        .filter_map(|file| {
            let plugin = registry.get_plugin(&file.file_path)?;

            let before_entities = if let Some(ref content) = file.before_content {
                let before_path = file.old_file_path.as_deref().unwrap_or(&file.file_path);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    plugin.extract_entities(content, before_path)
                }))
                .unwrap_or_default()
            } else {
                Vec::new()
            };

            let after_entities = if let Some(ref content) = file.after_content {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    plugin.extract_entities(content, &file.file_path)
                }))
                .unwrap_or_default()
            } else {
                Vec::new()
            };

            let sim_fn = |a: &crate::model::entity::SemanticEntity,
                          b: &crate::model::entity::SemanticEntity|
             -> f64 { plugin.compute_similarity(a, b) };

            let result = match_entities(
                &before_entities,
                &after_entities,
                &file.file_path,
                Some(&sim_fn),
                commit_sha,
                author,
            );

            if result.changes.is_empty() {
                None
            } else {
                Some((file.file_path.clone(), result.changes))
            }
        })
        .collect();

    let mut all_changes: Vec<SemanticChange> = Vec::new();
    let mut files_with_changes: HashSet<String> = HashSet::new();
    for (file_path, changes) in per_file_changes {
        files_with_changes.insert(file_path);
        all_changes.extend(changes);
    }

    // Single-pass counting
    let mut added_count = 0;
    let mut modified_count = 0;
    let mut deleted_count = 0;
    let mut moved_count = 0;
    let mut renamed_count = 0;

    for c in &all_changes {
        match c.change_type {
            ChangeType::Added => added_count += 1,
            ChangeType::Modified => modified_count += 1,
            ChangeType::Deleted => deleted_count += 1,
            ChangeType::Moved => moved_count += 1,
            ChangeType::Renamed => renamed_count += 1,
        }
    }

    DiffResult {
        changes: all_changes,
        file_count: files_with_changes.len(),
        added_count,
        modified_count,
        deleted_count,
        moved_count,
        renamed_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_types::FileStatus;
    use crate::parser::plugins::create_default_registry;

    /// Build a FileChange helper for tests.
    fn make_file_change(
        file_path: &str,
        status: FileStatus,
        before_content: Option<&str>,
        after_content: Option<&str>,
    ) -> FileChange {
        FileChange {
            file_path: file_path.to_string(),
            status,
            old_file_path: None,
            before_content: before_content.map(|s| s.to_string()),
            after_content: after_content.map(|s| s.to_string()),
        }
    }

    /// Empty input produces a zeroed-out DiffResult.
    #[test]
    fn test_empty_file_changes_returns_zero_counts() {
        let registry = create_default_registry();
        let result = compute_semantic_diff(&[], &registry, None, None);
        assert_eq!(result.file_count, 0);
        assert_eq!(result.added_count, 0);
        assert_eq!(result.modified_count, 0);
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.moved_count, 0);
        assert_eq!(result.renamed_count, 0);
        assert!(result.changes.is_empty());
    }

    /// A new JSON file (no before_content) should produce only Added changes.
    #[test]
    fn test_added_json_file_produces_added_changes() {
        let registry = create_default_registry();
        let after = r#"{
  "name": "my-app",
  "version": "1.0.0"
}
"#;
        let changes = vec![make_file_change(
            "package.json",
            FileStatus::Added,
            None,
            Some(after),
        )];
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.file_count, 1);
        assert!(
            result.added_count >= 1,
            "expected at least one added entity"
        );
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.modified_count, 0);
    }

    /// A deleted JSON file (no after_content) should produce only Deleted changes.
    #[test]
    fn test_deleted_json_file_produces_deleted_changes() {
        let registry = create_default_registry();
        let before = r#"{
  "name": "my-app",
  "version": "1.0.0"
}
"#;
        let changes = vec![make_file_change(
            "package.json",
            FileStatus::Deleted,
            Some(before),
            None,
        )];
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.file_count, 1);
        assert!(
            result.deleted_count >= 1,
            "expected at least one deleted entity"
        );
        assert_eq!(result.added_count, 0);
        assert_eq!(result.modified_count, 0);
    }

    /// Modifying a JSON value should produce a Modified change, not Add+Delete.
    #[test]
    fn test_modified_json_value_produces_modified_change() {
        let registry = create_default_registry();
        let before = "{\n  \"version\": \"1.0.0\"\n}\n";
        let after = "{\n  \"version\": \"2.0.0\"\n}\n";
        let changes = vec![make_file_change(
            "package.json",
            FileStatus::Modified,
            Some(before),
            Some(after),
        )];
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.file_count, 1);
        assert_eq!(result.modified_count, 1);
        assert_eq!(result.added_count, 0);
        assert_eq!(result.deleted_count, 0);
    }

    /// Renaming a JSON key with the same value should produce a Renamed change.
    #[test]
    fn test_renamed_json_key_produces_renamed_change() {
        let registry = create_default_registry();
        let before = "{\n  \"timeout\": 30\n}\n";
        let after = "{\n  \"request_timeout\": 30\n}\n";
        let changes = vec![make_file_change(
            "config.json",
            FileStatus::Modified,
            Some(before),
            Some(after),
        )];
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.renamed_count, 1);
        assert_eq!(result.added_count, 0);
        assert_eq!(result.deleted_count, 0);
    }

    /// Multiple files are processed independently; counts accumulate correctly.
    #[test]
    fn test_multiple_files_counts_accumulate() {
        let registry = create_default_registry();
        // File 1: add a new JSON file with 2 properties
        let after1 = "{\n  \"a\": 1,\n  \"b\": 2\n}\n";
        // File 2: add another JSON file with 1 property
        let after2 = "{\n  \"x\": 10\n}\n";
        let changes = vec![
            make_file_change("file1.json", FileStatus::Added, None, Some(after1)),
            make_file_change("file2.json", FileStatus::Added, None, Some(after2)),
        ];
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.file_count, 2);
        // Should see 3 added entities total (2 from file1, 1 from file2)
        assert_eq!(result.added_count, 3);
    }

    /// commit_sha and author are propagated to SemanticChange entries.
    #[test]
    fn test_commit_metadata_propagated_to_changes() {
        let registry = create_default_registry();
        let after = "{\n  \"key\": \"value\"\n}\n";
        let changes = vec![make_file_change(
            "meta.json",
            FileStatus::Added,
            None,
            Some(after),
        )];
        let result = compute_semantic_diff(&changes, &registry, Some("abc1234"), Some("Alice"));
        assert!(!result.changes.is_empty());
        for change in &result.changes {
            assert_eq!(change.commit_sha.as_deref(), Some("abc1234"));
            assert_eq!(change.author.as_deref(), Some("Alice"));
        }
    }

    /// A file with an unrecognised extension but no fallback match should be skipped
    /// (no changes produced for it), rather than panicking.
    #[test]
    fn test_unknown_extension_file_handled_gracefully() {
        let registry = create_default_registry();
        // .xyz is not registered; fallback plugin exists so it may or may not produce
        // changes — the important thing is no panic and the registry handles it.
        let after = "some random content\nwith multiple lines\n";
        let changes = vec![make_file_change(
            "data.xyz",
            FileStatus::Added,
            None,
            Some(after),
        )];
        // Should not panic regardless of outcome
        let _result = compute_semantic_diff(&changes, &registry, None, None);
    }

    /// YAML changes are processed correctly through the diff engine.
    #[test]
    fn test_yaml_added_file_produces_added_changes() {
        let registry = create_default_registry();
        let after = "name: my-app\nversion: 1.0.0\n";
        let changes = vec![make_file_change(
            "config.yaml",
            FileStatus::Added,
            None,
            Some(after),
        )];
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.file_count, 1);
        assert!(result.added_count >= 1);
    }

    /// Parallel processing: many files do not produce data races or inconsistent counts.
    #[test]
    fn test_parallel_processing_many_files() {
        let registry = create_default_registry();
        // Create 20 identical JSON files, each with 1 property added
        let after = "{\n  \"key\": \"value\"\n}\n";
        let changes: Vec<FileChange> = (0..20)
            .map(|i| {
                make_file_change(
                    &format!("file{i}.json"),
                    FileStatus::Added,
                    None,
                    Some(after),
                )
            })
            .collect();
        let result = compute_semantic_diff(&changes, &registry, None, None);
        assert_eq!(result.file_count, 20);
        assert_eq!(result.added_count, 20);
        assert_eq!(result.changes.len(), 20);
    }
}
