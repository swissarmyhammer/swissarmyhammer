//! Validator-fixture exclusion — the scope stage drops a changed file that is a
//! validator set's own fixture data instead of reviewing it as source.
//!
//! A tool rule's fail fixture holds the very defect the rule reports, so an
//! engine that reviews it as ordinary source makes every matching rule fire on
//! the file built to make it fire. Documenting or repairing the fixture then
//! breaks the fixture contract `builtin/validators/README.md` states, and the
//! next fixture edit raises the same findings again.
//!
//! The exclusion is derived from the validator STORE, never from a path pattern
//! written here: [`ValidatorLoader::fixture_dirs`] names the `fixtures/`
//! directory of every loaded set across all three layers (builtin, user
//! `~/.validators/`, project `./.validators/`), and a changed file under any of
//! them leaves the work-list. A fixture is therefore not test code excluded by a
//! glob — it is data the store itself declares, and `sah doctor` remains its
//! gate: doctor runs every tool rule against these files on each health check.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::validators::ValidatorLoader;

use super::resolve::{normalize_lexically, retain_scope_files, ResolvedScope};

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
}

impl ExcludedFile {
    /// The file dropped because it lives under a validator set's `fixtures/`
    /// directory — the one exclusion the scope stage makes, and the one way to
    /// name it.
    pub(crate) fn validator_fixture(path: &str) -> Self {
        Self {
            path: path.to_string(),
            reason: VALIDATOR_FIXTURE_REASON.to_string(),
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
}

/// Split every changed file that lives under a loaded validator set's
/// `fixtures/` directory out of `resolved`.
///
/// Returns the narrowed scope and one [`ExcludedFile`] for each dropped path, in
/// the resolved order. Each drop is logged at INFO with its FULL path and its
/// reason, never truncated, so a run that reviewed fewer files than it resolved
/// says which files and why.
pub(super) fn split_validator_fixtures(
    resolved: ResolvedScope,
    repo_path: &Path,
    loader: &ValidatorLoader,
) -> (ResolvedScope, Vec<ExcludedFile>) {
    let roots = fixture_roots(loader);
    if roots.is_empty() {
        return (resolved, Vec::new());
    }

    let repo_root = absolute_form(repo_path);
    let mut kept: Vec<String> = Vec::with_capacity(resolved.files.len());
    let mut excluded: Vec<ExcludedFile> = Vec::new();
    for path in &resolved.files {
        if is_validator_fixture(&repo_root, path, &roots) {
            tracing::info!(
                path = %path,
                reason = %VALIDATOR_FIXTURE_REASON,
                "review scope: excluded a validator set's own fixture data"
            );
            excluded.push(ExcludedFile::validator_fixture(path));
        } else {
            kept.push(path.clone());
        }
    }

    (retain_scope_files(resolved, kept), excluded)
}

/// Every loaded validator set's `fixtures/` directory, in the form the
/// containment test below compares against.
fn fixture_roots(loader: &ValidatorLoader) -> Vec<PathBuf> {
    loader
        .fixture_dirs()
        .iter()
        .map(|dir| absolute_form(dir))
        .collect()
}

/// Whether the repo-relative `rel_path` lives under one of the fixture `roots`.
///
/// The test is per path COMPONENT ([`Path::starts_with`]), so a directory named
/// `fixtures-old` beside a `fixtures` root is not swept in by a prefix match.
fn is_validator_fixture(repo_root: &Path, rel_path: &str, roots: &[PathBuf]) -> bool {
    let absolute = absolute_form(&repo_root.join(rel_path));
    roots.iter().any(|root| absolute.starts_with(root))
}

/// Resolve `path` to the form the containment test compares: the canonical path
/// when it exists on disk, and the lexically normalized path when it does not.
///
/// Canonicalizing matters because both sides must agree about symbolic links: a
/// temporary directory reaches a process as `/var/...` and resolves to
/// `/private/var/...` on macOS, so one side canonicalized and the other not
/// would never match. A path that does not exist — a deleted file, or the
/// fixtures directory of a set that ships none — cannot be canonicalized, so it
/// normalizes lexically instead.
fn absolute_form(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| normalize_lexically(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    use swissarmyhammer_sem::git_types::{FileChange as SemFileChange, FileStatus};

    use crate::review::test_support::{builtin_loader, loader_with};

    /// The change purpose the fixtures below carry. The split never reads it;
    /// it is carried through unchanged.
    const TEST_CHANGE_PURPOSE: &str = "a change touching a validator fixture";

    /// The repository root, from this crate's manifest directory.
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repository root above crates/<crate>")
            .to_path_buf()
    }

    /// A resolved scope over `files`, with each file's after-content and
    /// sem-diff entry present so the split's three-view consistency is
    /// observable.
    fn resolved_over(files: &[&str]) -> ResolvedScope {
        ResolvedScope {
            files: files.iter().map(|file| (*file).to_string()).collect(),
            file_changes: files
                .iter()
                .map(|file| SemFileChange {
                    file_path: (*file).to_string(),
                    status: FileStatus::Modified,
                    old_file_path: None,
                    before_content: None,
                    after_content: Some(String::new()),
                })
                .collect(),
            after_content: files
                .iter()
                .map(|file| ((*file).to_string(), String::new()))
                .collect::<BTreeMap<_, _>>(),
            change_purpose: TEST_CHANGE_PURPOSE.to_string(),
            blame_at: None,
        }
    }

    /// A changed file under a BUILTIN set's `fixtures/` directory leaves the
    /// scope, and a changed source file beside it stays.
    ///
    /// The loader is the real builtin one and the root is the real repository,
    /// so the fixture path is the shipped one this card names: a fail fixture of
    /// the `code-hygiene` set.
    #[test]
    fn a_changed_builtin_fixture_leaves_the_scope_and_source_stays() {
        let loader = builtin_loader();
        let root = repo_root();
        let fixture = "builtin/validators/code-hygiene/fixtures/missing-docs-rust.fail.rs.tmpl";
        let source = "crates/swissarmyhammer-validators/src/lib.rs";
        assert!(
            root.join(fixture).exists(),
            "the shipped fail fixture must exist for this test to mean anything"
        );

        let (scope, excluded) =
            split_validator_fixtures(resolved_over(&[fixture, source]), &root, &loader);

        assert_eq!(
            scope.files,
            vec![source.to_string()],
            "the fixture leaves the work-list and the source file stays"
        );
        assert_eq!(
            scope.file_changes.len(),
            1,
            "the sem-diff inputs narrow with the file list"
        );
        assert!(
            !scope.after_content.contains_key(fixture),
            "the fixture's content must not reach the review agent"
        );
        assert_eq!(excluded.len(), 1, "one file was excluded: {excluded:?}");
        assert_eq!(excluded[0].path(), fixture);
        assert_eq!(excluded[0].reason(), "validator fixture");
        assert_eq!(
            scope.change_purpose, TEST_CHANGE_PURPOSE,
            "the change purpose is carried through the split unchanged"
        );
    }

    /// A set whose `fixtures/` directory stands nowhere near the repository
    /// excludes nothing, so an ordinary review is untouched.
    #[test]
    fn a_scope_with_no_fixture_under_any_set_keeps_every_file() {
        let loader = loader_with("scoped", "*.rs", &[]);
        let root = repo_root();
        let source = "crates/swissarmyhammer-validators/src/lib.rs";

        let (scope, excluded) = split_validator_fixtures(resolved_over(&[source]), &root, &loader);

        assert_eq!(scope.files, vec![source.to_string()]);
        assert!(excluded.is_empty(), "nothing was excluded: {excluded:?}");
    }
}
