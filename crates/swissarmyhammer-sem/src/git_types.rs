//! Git-facing data types shared by the semantic diff pipeline: the scope a
//! diff targets, per-file change records, and commit metadata.
//!
//! # Examples
//!
//! Pick the scope a diff targets and describe one changed file:
//!
//! ```
//! use swissarmyhammer_sem::git_types::{DiffScope, FileChange, FileStatus};
//!
//! let scope = DiffScope::Working;
//! assert_eq!(scope.to_string(), "working");
//!
//! let one_commit = DiffScope::Commit {
//!     sha: "abc123".to_string(),
//! };
//! assert_eq!(one_commit.to_string(), "abc123");
//!
//! let change = FileChange {
//!     file_path: "src/lib.rs".to_string(),
//!     status: FileStatus::Modified,
//!     old_file_path: None,
//!     before_content: Some("fn old() {}".to_string()),
//!     after_content: Some("fn new() {}".to_string()),
//! };
//! assert_eq!(change.status, FileStatus::Modified);
//! assert_eq!(change.status.to_string(), "modified");
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// The git scope a diff targets: the working tree, the staged index, one
/// commit, or a commit range.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// Renders the scope in git revision syntax: `working`, `staged`, the
/// commit sha, or `from..to` for a range.
impl fmt::Display for DiffScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffScope::Working => f.write_str("working"),
            DiffScope::Staged => f.write_str("staged"),
            DiffScope::Commit { sha } => f.write_str(sha),
            DiffScope::Range { from, to } => write!(f, "{from}..{to}"),
        }
    }
}

/// What happened to a file in a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// Renders the status as its lowercase variant name — `added`, `modified`,
/// `deleted`, or `renamed` — matching the serialized wire form.
impl fmt::Display for FileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            FileStatus::Added => "added",
            FileStatus::Modified => "modified",
            FileStatus::Deleted => "deleted",
            FileStatus::Renamed => "renamed",
        };
        f.write_str(name)
    }
}

/// One file's change in a diff: its path, status, and the content on each
/// side.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

    /// Compile-time check that a type implements `Ord`.
    fn assert_ord<T: Ord>() {}

    /// Compile-time check that a type implements `Copy`.
    fn assert_copy<T: Copy>() {}

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
    fn every_type_implements_ord() {
        assert_ord::<DiffScope>();
        assert_ord::<FileStatus>();
        assert_ord::<FileChange>();
        assert_ord::<CommitInfo>();
    }

    #[test]
    fn file_status_is_copy() {
        assert_copy::<FileStatus>();
    }

    #[test]
    fn file_status_displays_lowercase_variant_names() {
        assert_eq!(FileStatus::Added.to_string(), "added");
        assert_eq!(FileStatus::Modified.to_string(), "modified");
        assert_eq!(FileStatus::Deleted.to_string(), "deleted");
        assert_eq!(FileStatus::Renamed.to_string(), "renamed");
    }

    #[test]
    fn file_status_display_matches_serde_wire_form() {
        for status in [
            FileStatus::Added,
            FileStatus::Modified,
            FileStatus::Deleted,
            FileStatus::Renamed,
        ] {
            let wire = serde_json::to_string(&status).expect("serialize FileStatus");
            assert_eq!(format!("\"{status}\""), wire);
        }
    }

    #[test]
    fn diff_scope_displays_as_git_revision_syntax() {
        assert_eq!(DiffScope::Working.to_string(), "working");
        assert_eq!(DiffScope::Staged.to_string(), "staged");
        assert_eq!(
            DiffScope::Commit {
                sha: "abc123".to_string(),
            }
            .to_string(),
            "abc123"
        );
        assert_eq!(
            DiffScope::Range {
                from: "abc123".to_string(),
                to: "def456".to_string(),
            }
            .to_string(),
            "abc123..def456"
        );
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
