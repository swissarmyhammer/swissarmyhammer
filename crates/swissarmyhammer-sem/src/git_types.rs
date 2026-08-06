//! Git-facing data types shared by the semantic diff pipeline: the scope a
//! diff targets, per-file change records, and commit metadata.

use serde::{Deserialize, Serialize};

/// The git scope a diff targets: the working tree, the staged index, one
/// commit, or a commit range.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
