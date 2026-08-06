//! Git-facing data types shared by the semantic diff pipeline: the scope a
//! diff targets, per-file change records, and commit metadata.

use serde::{Deserialize, Serialize};

/// The git scope a diff targets: the working tree, the staged index, one
/// commit, or a commit range.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffScope {
    /// Uncommitted changes in the working tree, compared against `HEAD`.
    Working,
    /// Changes staged in the index, compared against `HEAD`.
    Staged,
    /// The changes introduced by one commit.
    Commit {
        /// The sha of the commit to diff.
        sha: String,
    },
    /// The changes between two commits.
    Range {
        /// The sha the range starts from (exclusive).
        from: String,
        /// The sha the range ends at (inclusive).
        to: String,
    },
}

/// What happened to a file in a diff.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// The file is new in the diff.
    Added,
    /// The file exists on both sides with different content.
    Modified,
    /// The file was removed.
    Deleted,
    /// The file moved to a new path.
    Renamed,
}

/// One file's change in a diff: its path, status, and the content on each
/// side.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// The file's path, relative to the repository root.
    pub file_path: String,
    /// What happened to the file.
    pub status: FileStatus,
    /// The path the file had before a rename, when [`FileStatus::Renamed`].
    #[serde(default)]
    pub old_file_path: Option<String>,
    /// The file content on the old side of the diff; `None` for an added
    /// file.
    #[serde(default)]
    pub before_content: Option<String>,
    /// The file content on the new side of the diff; `None` for a deleted
    /// file.
    #[serde(default)]
    pub after_content: Option<String>,
}

/// Metadata for one commit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    /// The full commit sha.
    pub sha: String,
    /// The abbreviated commit sha.
    pub short_sha: String,
    /// The commit author's name.
    pub author: String,
    /// The commit date, as git formats it.
    pub date: String,
    /// The full commit message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Compile-time check that a type implements `Hash`.
    fn assert_hash<T: std::hash::Hash>() {}

    #[test]
    fn diff_scope_serde_round_trip() {
        let scopes = vec![
            DiffScope::Working,
            DiffScope::Staged,
            DiffScope::Commit {
                sha: "abc123".to_string(),
            },
            DiffScope::Range {
                from: "abc123".to_string(),
                to: "def456".to_string(),
            },
        ];
        for scope in scopes {
            let json = serde_json::to_string(&scope).expect("serialize DiffScope");
            let back: DiffScope = serde_json::from_str(&json).expect("deserialize DiffScope");
            assert_eq!(scope, back);
        }
    }

    #[test]
    fn diff_scope_unit_variants_serialize_lowercase() {
        // Enum variants follow the module convention FileStatus set:
        // lowercase on the wire.
        assert_eq!(
            serde_json::to_string(&DiffScope::Working).expect("serialize"),
            "\"working\""
        );
        assert_eq!(
            serde_json::to_string(&DiffScope::Staged).expect("serialize"),
            "\"staged\""
        );
    }

    #[test]
    fn file_status_is_usable_in_hash_collections() {
        let mut set = HashSet::new();
        set.insert(FileStatus::Added);
        set.insert(FileStatus::Added);
        set.insert(FileStatus::Modified);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn every_type_implements_hash() {
        assert_hash::<DiffScope>();
        assert_hash::<FileStatus>();
        assert_hash::<FileChange>();
        assert_hash::<CommitInfo>();
    }

    #[test]
    fn commit_info_serializes_fields_as_camel_case() {
        let info = CommitInfo {
            sha: "abc123def456".to_string(),
            short_sha: "abc123d".to_string(),
            author: "A. Author".to_string(),
            date: "2026-08-06".to_string(),
            message: "a commit".to_string(),
        };
        let json = serde_json::to_value(&info).expect("serialize CommitInfo");
        assert_eq!(json["shortSha"], "abc123d");
        assert!(
            json.get("short_sha").is_none(),
            "snake_case key must not appear on the wire"
        );
        let back: CommitInfo = serde_json::from_value(json).expect("deserialize CommitInfo");
        assert_eq!(info, back);
    }
}
