//! The review scope's ignore layer: `.reviewignore` (auto-generated) plus the
//! repo's own `.gitignore`.
//!
//! The scope stage ([`crate::review::scope`]) resolves a [`Scope`] to a changed-
//! file set and then filters it through the matcher this module builds, so
//! non-source artifacts never enter review. Two ignore sources are layered, both
//! in full gitignore syntax:
//!
//! - **`.reviewignore`** — review-only exclusions at the repo root. Auto-created
//!   on the first review with a default that ignores the kanban board directory
//!   (the finish loop rewrites those files constantly, and they are not code).
//!   A user's edits are authoritative: an existing file is never rewritten.
//! - **`.gitignore`** — anything git ignores is not source and must never be
//!   reviewed, so the repo's own ignore chain is honored too.
//!
//! The semantics are git's own, via the `ignore` crate — `!` negation and
//! directory patterns included — never a reimplementation.
//!
//! [`Scope`]: crate::review::scope::Scope

use std::path::{Path, PathBuf};

use ::ignore::gitignore::{Gitignore, GitignoreBuilder, Glob};
use ::ignore::Match;

use crate::error::AvpError;

/// The review-scope ignore file at the repo root.
const REVIEWIGNORE_FILE: &str = ".reviewignore";

/// The repo's own ignore file, layered under [`REVIEWIGNORE_FILE`].
const GITIGNORE_FILE: &str = ".gitignore";

/// The default `.reviewignore` written on the first review of a repo that has
/// none. A short comment header explains the file, followed by the one default
/// exclusion: the kanban board directory, which the finish loop rewrites on every
/// comment and which is not source to review.
const DEFAULT_REVIEWIGNORE: &str = "\
# .reviewignore — paths the review engine must never review.
#
# gitignore syntax (same as .gitignore, including `!` negation and directory
# patterns). Consumed by the `review` MCP ops when resolving a scope: any path
# matched here is dropped from the reviewed file set. Your repo's .gitignore is
# honored on top of this, so gitignored files are never reviewed either.
#
# Auto-generated on the first review; edit freely — it is never overwritten.
.kanban/
";

/// Ensure `<repo_path>/.reviewignore` exists, writing the default template when
/// it is absent, and return its path.
///
/// An existing file is never overwritten or appended to — user edits are
/// authoritative — so the default is written exactly once, on the first review.
///
/// # Errors
///
/// Returns [`AvpError::Io`] when the file is absent and cannot be written.
pub fn ensure_reviewignore(repo_path: &Path) -> Result<PathBuf, AvpError> {
    let path = repo_path.join(REVIEWIGNORE_FILE);
    if !path.exists() {
        std::fs::write(&path, DEFAULT_REVIEWIGNORE)?;
    }
    Ok(path)
}

/// Build the review-scope ignore matcher from `<repo_path>/.gitignore` layered
/// under `<repo_path>/.reviewignore`.
///
/// Both files are optional: a missing one contributes no patterns, so a repo
/// without either yields a matcher that excludes nothing. `.reviewignore` is
/// added last so its `!` negations win over a broader `.gitignore` exclusion
/// (gitignore's last-match-wins precedence).
///
/// # Errors
///
/// Returns [`AvpError::Context`] when the accumulated globs fail to compile into
/// a matcher.
pub fn load_review_ignore_matcher(repo_path: &Path) -> Result<Gitignore, AvpError> {
    let mut builder = GitignoreBuilder::new(repo_path);
    // Order is deliberate: `.gitignore` first, `.reviewignore` last. Gitignore
    // precedence is last-match-wins, so review-only rules — including `!`
    // re-includes — take precedence over the repo's own ignore chain.
    add_if_present(&mut builder, &repo_path.join(GITIGNORE_FILE));
    add_if_present(&mut builder, &repo_path.join(REVIEWIGNORE_FILE));
    builder
        .build()
        .map_err(|e| AvpError::Context(format!("failed to build review ignore matcher: {e}")))
}

/// Add a gitignore-syntax file to `builder` only when it exists on disk.
///
/// The `ignore` crate's [`GitignoreBuilder::add`] reports a missing file as an
/// I/O error; guarding on existence keeps a repo without a `.gitignore` (or a
/// first run before `.reviewignore` is written) from surfacing that as an error,
/// contributing no patterns instead. A malformed glob inside a present file is a
/// non-fatal partial error the builder collects and `build()` surfaces.
fn add_if_present(builder: &mut GitignoreBuilder, path: &Path) {
    if path.exists() {
        let _ = builder.add(path);
    }
}

/// Describe how `matcher` treats the repo-relative `rel_path`: `None` when the
/// path is reviewable (unmatched, or re-included by a `!` negation), or
/// `Some(pattern_source)` — a human string naming the excluding glob and the
/// ignore file it came from — when the path is excluded.
///
/// Matching walks parent directories ([`Gitignore::matched_path_or_any_parents`])
/// so a directory pattern like `.kanban/` excludes every file beneath it, not
/// only the directory entry itself.
pub fn review_ignore_reason(matcher: &Gitignore, rel_path: &str) -> Option<String> {
    // The path is repo-relative, so it never carries a root — the precondition
    // `matched_path_or_any_parents` asserts on. A `Whitelist` (a `!` negation)
    // and `None` both mean reviewable; only `Ignore` excludes.
    match matcher.matched_path_or_any_parents(rel_path, false) {
        Match::Ignore(glob) => Some(describe_glob(glob)),
        Match::Whitelist(_) | Match::None => None,
    }
}

/// A human string naming the glob that excluded a path and the ignore file it
/// came from, for the scope stage's debug log — e.g. `.kanban/ (from .reviewignore)`.
fn describe_glob(glob: &Glob) -> String {
    match glob.from() {
        Some(source) => format!("{} (from {})", glob.original(), source.display()),
        None => glob.original().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    /// A first review writes the default `.reviewignore`, with the explanatory
    /// header and the `.kanban/` default exclusion.
    #[test]
    fn ensure_reviewignore_creates_the_default_when_absent() {
        let dir = TempDir::new().unwrap();
        let path = ensure_reviewignore(dir.path()).unwrap();

        assert_eq!(path, dir.path().join(".reviewignore"));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains(".kanban/"),
            "the default must ignore the kanban board directory, got:\n{content}"
        );
        assert!(
            content.contains("gitignore syntax"),
            "the default must carry an explanatory header, got:\n{content}"
        );
    }

    /// A pre-existing `.reviewignore` is authoritative: a second run leaves it
    /// byte-identical, never rewriting user edits.
    #[test]
    fn ensure_reviewignore_preserves_an_existing_file() {
        let dir = TempDir::new().unwrap();
        let edited = "# my rules\ntarget/\n!target/keep.rs\n";
        std::fs::write(dir.path().join(".reviewignore"), edited).unwrap();

        let path = ensure_reviewignore(dir.path()).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            content, edited,
            "an existing .reviewignore must be preserved byte-for-byte"
        );
    }

    /// With neither `.reviewignore` nor `.gitignore` on disk, the matcher is
    /// empty and excludes nothing.
    #[test]
    fn matcher_with_no_ignore_files_excludes_nothing() {
        let dir = TempDir::new().unwrap();
        let matcher = load_review_ignore_matcher(dir.path()).unwrap();
        assert_eq!(review_ignore_reason(&matcher, "src/lib.rs"), None);
        assert_eq!(review_ignore_reason(&matcher, ".kanban/board.md"), None);
    }

    /// A `.reviewignore` directory pattern excludes files beneath it, and its
    /// reason names the pattern and the source file.
    #[test]
    fn matcher_excludes_files_under_a_reviewignore_directory_pattern() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".reviewignore"), ".kanban/\n").unwrap();

        let matcher = load_review_ignore_matcher(dir.path()).unwrap();

        let reason = review_ignore_reason(&matcher, ".kanban/board.md")
            .expect("a file under .kanban/ must be excluded");
        assert!(
            reason.contains(".kanban/"),
            "the reason must name the excluding pattern, got: {reason}"
        );
        assert!(
            reason.contains(".reviewignore"),
            "the reason must name the source ignore file, got: {reason}"
        );
        assert_eq!(
            review_ignore_reason(&matcher, "src/lib.rs"),
            None,
            "an unmatched path stays reviewable"
        );
    }

    /// A `!` negation re-includes a file a broader pattern excluded — full
    /// gitignore semantics.
    #[test]
    fn matcher_honors_a_negation_reincluding_a_broadly_excluded_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(".reviewignore"),
            "generated/\n!generated/keep.rs\n",
        )
        .unwrap();

        let matcher = load_review_ignore_matcher(dir.path()).unwrap();

        assert!(
            review_ignore_reason(&matcher, "generated/noise.rs").is_some(),
            "the broad directory pattern excludes ordinary files under it"
        );
        assert_eq!(
            review_ignore_reason(&matcher, "generated/keep.rs"),
            None,
            "a `!` negation re-includes the named file"
        );
    }

    /// The repo's own `.gitignore` is honored even without a `.reviewignore`.
    #[test]
    fn matcher_honors_gitignore_without_a_reviewignore() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();

        let matcher = load_review_ignore_matcher(dir.path()).unwrap();

        assert!(
            review_ignore_reason(&matcher, "run.log").is_some(),
            "a gitignored file must be excluded from review too"
        );
        assert_eq!(review_ignore_reason(&matcher, "src/lib.rs"), None);
    }
}
