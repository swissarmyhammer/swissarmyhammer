//! Git2 utility functions compatible with the original implementation
//!
//! This module provides utility functions that match the original git2_utils
//! interface for backward compatibility with existing test code.

use crate::error::{convert_git2_error, GitResult};
use git2::Repository;
use std::path::Path;
use swissarmyhammer_common::Pretty;
use tracing::debug;

/// Add files to the index (matches original git2_utils interface)
///
/// # Arguments
/// * `repo` - Git repository reference
/// * `paths` - Array of file paths to add
///
/// # Returns
/// * `Ok(())` - Files added successfully
/// * `Err(GitError)` - Add operation failed
pub fn add_files(repo: &Repository, paths: &[&str]) -> GitResult<()> {
    debug!("Adding files to index: {}", Pretty(&paths));

    let mut index = repo
        .index()
        .map_err(|e| convert_git2_error("get_index", e))?;

    for path in paths {
        index
            .add_path(Path::new(path))
            .map_err(|e| convert_git2_error("add_path", e))?;
    }

    index
        .write()
        .map_err(|e| convert_git2_error("write_index", e))?;

    Ok(())
}

/// Create a commit (matches original git2_utils interface)
///
/// # Arguments
/// * `repo` - Git repository reference
/// * `message` - Commit message
/// * `author_name` - Author name (None uses config)
/// * `author_email` - Author email (None uses config)
///
/// # Returns
/// * `Ok(String)` - Commit hash
/// * `Err(GitError)` - Commit creation failed
pub fn create_commit(
    repo: &Repository,
    message: &str,
    author_name: Option<&str>,
    author_email: Option<&str>,
) -> GitResult<String> {
    debug!("Creating commit with message: {}", message);

    // Get signature (author and committer)
    let signature = if let (Some(name), Some(email)) = (author_name, author_email) {
        git2::Signature::now(name, email).map_err(|e| convert_git2_error("create_signature", e))?
    } else {
        // Use repository config
        repo.signature()
            .map_err(|e| convert_git2_error("get_signature", e))?
    };

    // Get the index and write tree
    let mut index = repo
        .index()
        .map_err(|e| convert_git2_error("get_index", e))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| convert_git2_error("write_tree", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| convert_git2_error("find_tree", e))?;

    // Get parent commit(s)
    let parents: Vec<git2::Commit> = match repo.head() {
        Ok(head) => {
            let commit = head
                .peel_to_commit()
                .map_err(|e| convert_git2_error("peel_to_commit", e))?;
            vec![commit]
        }
        Err(_) => Vec::new(), // Initial commit
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    // Create the commit
    let commit_oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .map_err(|e| convert_git2_error("create_commit", e))?;

    Ok(commit_oid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use tempfile::TempDir;

    /// Set up a minimal git repository in a temp directory.
    ///
    /// Returns the TempDir (kept alive for the test) and the opened Repository.
    /// The repository is configured with a test user name and email so that
    /// `repo.signature()` succeeds, which is required by `create_commit` when
    /// called without explicit author details.
    fn setup_repo() -> (TempDir, Repository) {
        let tmp = TempDir::new().expect("tempdir");
        let repo = Repository::init(tmp.path()).expect("git init");

        // Set local user config so repo.signature() works.
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("user.name");
        cfg.set_str("user.email", "test@example.com")
            .expect("user.email");

        (tmp, repo)
    }

    /// Create an initial commit so subsequent commits have a parent.
    ///
    /// Writes a single file, stages it, and calls `create_commit` with an
    /// explicit author so that the helper is exercised even before we test the
    /// two paths explicitly.
    fn make_initial_commit(tmp: &TempDir, repo: &Repository) {
        let file_path = tmp.path().join("init.txt");
        std::fs::write(&file_path, "init\n").expect("write init.txt");

        // Stage the file via add_files.
        add_files(repo, &["init.txt"]).expect("add init.txt");

        // Commit with an explicit author (tests this path implicitly).
        create_commit(
            repo,
            "Initial commit",
            Some("Test User"),
            Some("test@example.com"),
        )
        .expect("initial commit");
    }

    // -----------------------------------------------------------------------
    // add_files tests
    // -----------------------------------------------------------------------

    /// add_files stages a newly created file so it appears in the index.
    #[test]
    fn test_add_files_stages_new_file() {
        let (tmp, repo) = setup_repo();

        let file_path = tmp.path().join("hello.txt");
        std::fs::write(&file_path, "hello\n").expect("write");

        add_files(&repo, &["hello.txt"]).expect("add_files");

        // The file should now be in the index.
        let index = repo.index().expect("index");
        let entry = index.get_path(std::path::Path::new("hello.txt"), 0);
        assert!(
            entry.is_some(),
            "hello.txt should be present in the index after add_files"
        );
    }

    /// add_files can stage multiple files in a single call.
    #[test]
    fn test_add_files_stages_multiple_files() {
        let (tmp, repo) = setup_repo();

        std::fs::write(tmp.path().join("a.txt"), "a\n").expect("write a");
        std::fs::write(tmp.path().join("b.txt"), "b\n").expect("write b");

        add_files(&repo, &["a.txt", "b.txt"]).expect("add_files");

        let index = repo.index().expect("index");
        assert!(
            index.get_path(std::path::Path::new("a.txt"), 0).is_some(),
            "a.txt should be staged"
        );
        assert!(
            index.get_path(std::path::Path::new("b.txt"), 0).is_some(),
            "b.txt should be staged"
        );
    }

    /// add_files returns an error when a path does not exist on disk.
    #[test]
    fn test_add_files_error_on_missing_file() {
        let (_tmp, repo) = setup_repo();

        let result = add_files(&repo, &["does_not_exist.txt"]);
        assert!(
            result.is_err(),
            "add_files should fail for a non-existent path"
        );
    }

    // -----------------------------------------------------------------------
    // create_commit tests
    // -----------------------------------------------------------------------

    /// create_commit with explicit author/email creates the initial commit and
    /// returns a non-empty OID string.
    #[test]
    fn test_create_commit_explicit_author_initial() {
        let (tmp, repo) = setup_repo();

        std::fs::write(tmp.path().join("file.txt"), "content\n").expect("write");
        add_files(&repo, &["file.txt"]).expect("add_files");

        let oid = create_commit(
            &repo,
            "First commit",
            Some("Alice"),
            Some("alice@example.com"),
        )
        .expect("create_commit");

        assert!(!oid.is_empty(), "commit OID should not be empty");

        // The commit should be reachable via HEAD.
        let head_commit = repo.head().expect("HEAD").peel_to_commit().expect("peel");
        assert_eq!(head_commit.id().to_string(), oid);
        assert_eq!(head_commit.message().unwrap(), "First commit");
        assert_eq!(head_commit.author().name().unwrap(), "Alice");
        assert_eq!(head_commit.author().email().unwrap(), "alice@example.com");
    }

    /// create_commit with None author/email falls back to repo config and
    /// creates a commit whose author matches user.name / user.email.
    #[test]
    fn test_create_commit_default_signature() {
        let (tmp, repo) = setup_repo();

        // Make an initial commit first (explicit author path).
        make_initial_commit(&tmp, &repo);

        // Now create a second commit using default signature.
        std::fs::write(tmp.path().join("second.txt"), "second\n").expect("write");
        add_files(&repo, &["second.txt"]).expect("add_files");

        let oid = create_commit(&repo, "Second commit", None, None)
            .expect("create_commit with default signature");

        assert!(!oid.is_empty());

        let head_commit = repo.head().expect("HEAD").peel_to_commit().expect("peel");
        assert_eq!(head_commit.id().to_string(), oid);
        // Author should come from config.
        assert_eq!(head_commit.author().name().unwrap(), "Test User");
        assert_eq!(head_commit.author().email().unwrap(), "test@example.com");
        // There should be exactly one parent (the initial commit).
        assert_eq!(head_commit.parent_count(), 1);
    }

    /// create_commit with explicit author creates a commit that is a child of
    /// the previous HEAD (non-initial commit case).
    #[test]
    fn test_create_commit_explicit_author_with_parent() {
        let (tmp, repo) = setup_repo();

        make_initial_commit(&tmp, &repo);

        let parent_oid = repo
            .head()
            .expect("HEAD")
            .peel_to_commit()
            .expect("peel")
            .id()
            .to_string();

        // Second commit with explicit author.
        std::fs::write(tmp.path().join("change.txt"), "change\n").expect("write");
        add_files(&repo, &["change.txt"]).expect("add_files");

        let child_oid = create_commit(&repo, "Child commit", Some("Bob"), Some("bob@example.com"))
            .expect("create_commit");

        assert_ne!(child_oid, parent_oid);

        let commit = repo.head().expect("HEAD").peel_to_commit().expect("peel");
        assert_eq!(
            commit.parent(0).expect("parent").id().to_string(),
            parent_oid
        );
        assert_eq!(commit.author().name().unwrap(), "Bob");
    }
}
