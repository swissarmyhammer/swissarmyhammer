//! Test helpers for git operations
//!
//! This module provides utilities for setting up test git repositories using libgit2
//! instead of shell commands. This makes tests more reliable and portable.

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A test git repository with helpers for common operations
pub struct TestGitRepo {
    /// Temporary directory containing the repository
    _temp_dir: TempDir,
    /// Path to the repository
    pub path: PathBuf,
    /// Low-level git2 repository handle
    repo: git2::Repository,
}

impl TestGitRepo {
    /// Create a new test repository with initial configuration
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let path = temp_dir.path().to_path_buf();

        // Initialize repository
        let repo = git2::Repository::init(&path).expect("Failed to init repository");

        // Configure git for testing
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user.email");

        Self {
            _temp_dir: temp_dir,
            path,
            repo,
        }
    }

    /// Get the repository path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create a file with the given content
    pub fn create_file(&self, filename: &str, content: &str) {
        let file_path = self.path.join(filename);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent directory");
        }
        std::fs::write(&file_path, content).expect("Failed to write file");
    }

    /// Add all files to the index
    pub fn add_all(&self) {
        let mut index = self.repo.index().expect("Failed to get index");
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("Failed to add files");
        index.write().expect("Failed to write index");
    }

    /// Create a commit with the given message
    pub fn commit(&self, message: &str) -> git2::Oid {
        let mut index = self.repo.index().expect("Failed to get index");
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = self.repo.find_tree(tree_id).expect("Failed to find tree");

        let signature =
            git2::Signature::now("Test User", "test@example.com").expect("Failed to create signature");

        // Get parent commit if this isn't the first commit
        let parent_commit = match self.repo.head() {
            Ok(head) => {
                let parent_oid = head.target().expect("Failed to get head target");
                Some(self.repo.find_commit(parent_oid).expect("Failed to find parent commit"))
            }
            Err(_) => None, // First commit
        };

        let parents: Vec<&git2::Commit> = parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();

        self.repo
            .commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
            .expect("Failed to create commit")
    }

    /// Create a file and commit it in one operation
    pub fn commit_file(&self, filename: &str, content: &str, message: &str) -> git2::Oid {
        self.create_file(filename, content);
        self.add_all();
        self.commit(message)
    }

    /// Create a new branch from HEAD
    pub fn create_branch(&self, branch_name: &str) {
        let head_commit = self
            .repo
            .head()
            .and_then(|head| head.peel_to_commit())
            .expect("Failed to get head commit");

        self.repo
            .branch(branch_name, &head_commit, false)
            .expect("Failed to create branch");
    }

    /// Checkout a branch
    pub fn checkout_branch(&self, branch_name: &str) {
        let branch_ref_name = format!("refs/heads/{}", branch_name);
        let obj = self
            .repo
            .revparse_single(&branch_ref_name)
            .expect("Failed to resolve branch");

        self.repo
            .checkout_tree(&obj, None)
            .expect("Failed to checkout tree");

        self.repo
            .set_head(&branch_ref_name)
            .expect("Failed to set head");
    }

    /// Create and checkout a branch in one operation
    pub fn create_and_checkout_branch(&self, branch_name: &str) {
        self.create_branch(branch_name);
        self.checkout_branch(branch_name);
    }

    /// Get the underlying git2::Repository
    pub fn repo(&self) -> &git2::Repository {
        &self.repo
    }
}

impl Default for TestGitRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_repo() {
        let repo = TestGitRepo::new();
        assert!(repo.path().exists());
        assert!(repo.path().join(".git").exists());
    }

    #[test]
    fn test_commit_file() {
        let repo = TestGitRepo::new();
        let commit_id = repo.commit_file("test.txt", "content", "Initial commit");
        assert!(!commit_id.is_zero());
    }

    #[test]
    fn test_create_branch() {
        let repo = TestGitRepo::new();
        repo.commit_file("test.txt", "content", "Initial commit");
        repo.create_and_checkout_branch("test-branch");

        let head = repo.repo().head().expect("Failed to get head");
        assert_eq!(head.shorthand(), Some("test-branch"));
    }
}
