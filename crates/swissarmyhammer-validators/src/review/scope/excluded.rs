//! The files the scope stage drops before any validator pairs with them, and
//! the two deliberate reasons it drops one.
//!
//! An exclusion is never silent. A run that reviewed fewer files than it
//! resolved says which files and why, so a reader can tell a deliberate
//! exclusion from a file the run missed. The report renders both kinds, and a
//! scope every one of whose files was excluded is a clean review that states
//! the exclusion — never an empty scope.
//!
//! The two kinds arrive from different stages and must never be conflated:
//! [`ExclusionKind::ReviewIgnore`] comes from the `.reviewignore` /
//! `.gitignore` filter in [`super::resolve`], and
//! [`ExclusionKind::ValidatorFixture`] from the fixture split in
//! [`super::fixtures`].

use serde::Serialize;

/// Why the scope stage dropped a file.
///
/// The report renders the two kinds differently — an ignore exclusion is
/// grouped under the pattern that excluded it, a fixture exclusion is named
/// per file — so a reader sees at a glance whether a configuration or the
/// validator store took the file out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ExclusionKind {
    /// A pattern in `.reviewignore` or `.gitignore` matched the file.
    ReviewIgnore,
    /// The file is a validator set's own fixture data.
    ValidatorFixture,
}

/// The reason recorded for a file dropped because it is a validator set's own
/// fixture data.
const VALIDATOR_FIXTURE_REASON: &str = "validator fixture";

/// A changed file the scope stage dropped before any validator paired with it,
/// carrying the reason it was dropped.
///
/// An excluded file is never reviewed: it becomes no LLM (validator, file) pair
/// and is never an argument to a tool rule's `run` script. It is reported rather
/// than dropped in silence — the report names every one of them and its reason,
/// so a reader can tell a deliberate exclusion from a file the run missed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExcludedFile {
    /// The excluded file's repo-relative path.
    path: String,
    /// Why the scope stage dropped it, in the reader's words.
    reason: String,
    /// Which stage dropped it.
    kind: ExclusionKind,
}

impl ExcludedFile {
    /// The file dropped because it lives under a validator set's `fixtures/`
    /// directory.
    pub(crate) fn validator_fixture(path: &str) -> Self {
        Self {
            path: path.to_string(),
            reason: VALIDATOR_FIXTURE_REASON.to_string(),
            kind: ExclusionKind::ValidatorFixture,
        }
    }

    /// The file dropped because an ignore pattern matched it.
    ///
    /// `pattern` is the human string
    /// [`review_ignore_reason`](crate::review::ignore::review_ignore_reason)
    /// builds — the excluding glob and the ignore file it came from — and it is
    /// the key the report groups this kind of exclusion under.
    pub(crate) fn review_ignored(path: &str, pattern: String) -> Self {
        Self {
            path: path.to_string(),
            reason: pattern,
            kind: ExclusionKind::ReviewIgnore,
        }
    }

    /// The excluded file's repo-relative path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Why the scope stage dropped it, in the reader's words.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Which stage dropped it.
    pub fn kind(&self) -> ExclusionKind {
        self.kind
    }
}
