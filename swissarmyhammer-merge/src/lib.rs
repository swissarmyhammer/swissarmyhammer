pub mod frontmatter;
pub mod jsonl;
pub mod md;
pub mod yaml;

use std::fmt;

/// A conflict detected during merge (actual content disagreement between branches).
///
/// This is distinguished from a parse/fatal error; callers should use exit code 1
/// for conflicts and exit code 2 for fatal errors.
#[derive(Debug, Clone)]
pub struct MergeConflict {
    /// IDs or field names that have conflicting content across branches.
    pub conflicting_ids: Vec<String>,
}

impl fmt::Display for MergeConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "merge conflict on ids: {}",
            self.conflicting_ids.join(", ")
        )
    }
}

impl std::error::Error for MergeConflict {}

/// Error type for merge operations, distinguishing parse failures from true conflicts.
///
/// - `ParseFailure` indicates the input could not be parsed (YAML/JSON error). Callers
///   should treat this as a fatal error (exit code 2).
/// - `Conflict` indicates a genuine merge conflict that cannot be auto-resolved. Callers
///   should treat this as a conflict (exit code 1).
#[derive(Debug, Clone, thiserror::Error)]
pub enum MergeError {
    /// Input could not be parsed — indicates malformed YAML or JSON.
    #[error("parse failure: {0}")]
    ParseFailure(String),
    /// Actual merge conflict between the two sides.
    #[error("{0}")]
    Conflict(MergeConflict),
}

impl From<MergeConflict> for MergeError {
    /// Convert a [`MergeConflict`] into a [`MergeError::Conflict`] variant.
    fn from(c: MergeConflict) -> Self {
        MergeError::Conflict(c)
    }
}
