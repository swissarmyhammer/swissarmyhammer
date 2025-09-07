//! Git repository management and utilities
//!
//! This module provides the GitRepository wrapper around git2::Repository
//! with enhanced error handling and convenience methods.

use crate::error::{convert_git2_error, GitError, GitResult};
use git2::Repository;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Wrapper around git2::Repository with enhanced functionality
pub struct GitRepository {
    /// The underlying git2 repository
    repo: Repository,
    /// Path to the repository root
    path: PathBuf,
}

impl std::fmt::Debug for GitRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitRepository")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl GitRepository {
    /// Open a git repository at the specified path
    ///
    /// This method will search for a git repository starting from the given path
    /// and walking up the directory tree until it finds one.
    pub fn open<P: AsRef<Path>>(path: P) -> GitResult<Self> {
        let path = path.as_ref();
        debug!("Opening git repository at: {}", path.display());

        let repo = Repository::discover(path).map_err(|e| {
            let error_msg = match e.code() {
                git2::ErrorCode::NotFound => {
                    format!(
                        "Git repository not found at '{}'. This may not be a git repository or the path may not exist.",
                        path.display()
                    )
                }
                git2::ErrorCode::Invalid => {
                    format!(
                        "Invalid git repository at '{}'. The repository may be corrupted.",
                        path.display()
                    )
                }
                git2::ErrorCode::Ambiguous => {
                    format!(
                        "Ambiguous git repository reference at '{}'. Multiple matches found.",
                        path.display()
                    )
                }
                _ => {
                    format!(
                        "Failed to open git repository at '{}'. Git2 error: {}",
                        path.display(),
                        e
                    )
                }
            };

            warn!("Repository open failed: {}", error_msg);
            GitError::repository_not_found(path, error_msg)
        })?;

        let repo_path = repo
            .workdir()
            .or_else(|| repo.path().parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path.to_path_buf());

        debug!("Successfully opened repository at: {}", repo_path.display());

        Ok(Self {
            repo,
            path: repo_path,
        })
    }

    /// Initialize a new git repository
    pub fn init<P: AsRef<Path>>(path: P) -> GitResult<Self> {
        let path = path.as_ref();
        debug!("Initializing git repository at: {}", path.display());

        let repo = Repository::init(path).map_err(|e| convert_git2_error("init_repository", e))?;

        let repo_path = repo
            .workdir()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path.to_path_buf());

        Ok(Self {
            repo,
            path: repo_path,
        })
    }

    /// Get the underlying git2::Repository
    pub fn inner(&self) -> &Repository {
        &self.repo
    }

    /// Get the path to the repository root
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the working directory path
    pub fn workdir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    /// Check if this is a bare repository
    pub fn is_bare(&self) -> bool {
        self.repo.is_bare()
    }

    /// Check if this is a valid repository
    pub fn is_valid(&self) -> bool {
        // Try to get the HEAD reference as a simple validity check
        self.repo.head().is_ok() || self.is_empty()
    }

    /// Check if the repository is empty (no commits)
    pub fn is_empty(&self) -> bool {
        self.repo.is_empty().unwrap_or(false)
    }

    /// Get the path to the .git directory
    pub fn git_dir(&self) -> &Path {
        self.repo.path()
    }

    /// Check if a path is inside this repository
    pub fn contains_path<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        if let Some(workdir) = self.workdir() {
            path.starts_with(workdir)
        } else {
            // For bare repositories, check against the git directory
            path.starts_with(self.git_dir())
        }
    }

    /// Get repository state (normal, merge, rebase, etc.)
    pub fn state(&self) -> git2::RepositoryState {
        self.repo.state()
    }

    /// Check if the repository is in a normal state
    pub fn is_in_normal_state(&self) -> bool {
        matches!(self.state(), git2::RepositoryState::Clean)
    }

    /// Find repository from current directory or any parent directory
    pub fn find_from_current_dir() -> GitResult<Self> {
        let current_dir = std::env::current_dir()
            .map_err(|e| GitError::from_io("get_current_dir".to_string(), e))?;
        Self::open(current_dir)
    }

    /// Check if a directory contains a git repository
    pub fn exists_at<P: AsRef<Path>>(path: P) -> bool {
        Repository::discover(path.as_ref()).is_ok()
    }
}

/// Utility functions for working with git repositories
pub mod utils {
    use super::*;

    /// Find a git repository starting from the given path and walking up
    pub fn find_repository<P: AsRef<Path>>(start_path: P) -> GitResult<GitRepository> {
        GitRepository::open(start_path)
    }

    /// Check if a path is within a git repository
    pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
        GitRepository::exists_at(path)
    }

    /// Get the root directory of the git repository containing the given path
    pub fn get_repository_root<P: AsRef<Path>>(path: P) -> GitResult<PathBuf> {
        let repo = GitRepository::open(path)?;
        Ok(repo.path().to_path_buf())
    }

    /// Get the .git directory for the repository containing the given path
    pub fn get_git_dir<P: AsRef<Path>>(path: P) -> GitResult<PathBuf> {
        let repo = GitRepository::open(path)?;
        Ok(repo.git_dir().to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, GitRepository) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let repo = GitRepository::init(temp_dir.path()).expect("Failed to initialize repository");
        (temp_dir, repo)
    }

    #[test]
    fn test_repository_init_and_open() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Test initialization
        let repo = GitRepository::init(repo_path).unwrap();
        assert!(repo.is_valid());
        assert!(repo.is_empty());
        assert!(!repo.is_bare());

        // Test opening existing repository
        let repo2 = GitRepository::open(repo_path).unwrap();
        assert_eq!(repo.path(), repo2.path());
    }

    #[test]
    fn test_repository_discovery() {
        let (_temp_dir, repo) = setup_test_repo();

        // Create a subdirectory
        let subdir = repo.path().join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();

        // Should be able to discover repository from subdirectory
        let discovered = GitRepository::open(&subdir).unwrap();
        assert_eq!(repo.path(), discovered.path());
    }

    #[test]
    fn test_repository_validation() {
        let (_temp_dir, repo) = setup_test_repo();

        assert!(repo.is_valid());
        assert!(repo.is_empty());
        assert!(repo.is_in_normal_state());
        assert!(!repo.is_bare());
    }

    #[test]
    fn test_path_containment() {
        let (_temp_dir, repo) = setup_test_repo();

        let internal_path = repo.path().join("some_file.txt");
        let external_path = PathBuf::from("/tmp/external_file.txt");

        assert!(repo.contains_path(&internal_path));
        assert!(!repo.contains_path(&external_path));
    }

    #[test]
    fn test_utils_functions() {
        let (_temp_dir, repo) = setup_test_repo();

        // Test utility functions
        assert!(utils::is_git_repository(repo.path()));
        assert_eq!(
            utils::get_repository_root(repo.path()).unwrap(),
            repo.path()
        );

        let found_repo = utils::find_repository(repo.path()).unwrap();
        assert_eq!(found_repo.path(), repo.path());
    }

    #[test]
    fn test_nonexistent_repository() {
        let temp_dir = TempDir::new().unwrap();
        let non_repo_path = temp_dir.path().join("not_a_repo");

        // Should fail to open non-existent repository
        assert!(GitRepository::open(&non_repo_path).is_err());
        assert!(!utils::is_git_repository(&non_repo_path));
    }
}
