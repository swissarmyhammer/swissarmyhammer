//! Managed directory structure for configuration directories.
//!
//! This module provides `ManagedDirectory<C>`, a generic struct that manages
//! directories like `.swissarmyhammer` or `.avp` with support for different
//! root locations (git root, user home, custom paths).

use crate::config::DirectoryConfig;
use crate::error::{DirectoryError, Result};
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// Represents the type of root location for a managed directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryRootType {
    /// In user's home directory (e.g., ~/.swissarmyhammer)
    UserHome,

    /// At Git repository root (e.g., ./.swissarmyhammer)
    GitRoot,

    /// Custom path (for testing or special cases)
    Custom(PathBuf),
}

impl std::fmt::Display for DirectoryRootType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserHome => write!(f, "user home"),
            Self::GitRoot => write!(f, "git repository root"),
            Self::Custom(path) => write!(f, "custom path: {}", path.display()),
        }
    }
}

/// A managed directory with automatic creation and gitignore handling.
///
/// `ManagedDirectory<C>` provides a centralized, type-safe way to access
/// configuration directories. It supports multiple root locations and
/// provides consistent path resolution.
///
/// # Type Parameters
///
/// * `C` - A type implementing `DirectoryConfig` that specifies the directory
///   name, gitignore content, and initialization settings.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_directory::{ManagedDirectory, SwissarmyhammerConfig};
///
/// // Create from Git repository root
/// let dir = ManagedDirectory::<SwissarmyhammerConfig>::from_git_root()?;
///
/// // Get a subdirectory, creating it if needed
/// let rules_dir = dir.ensure_subdir("rules")?;
///
/// // Just get the path without creating
/// let todo_path = dir.subdir("todo");
/// # Ok::<(), swissarmyhammer_directory::DirectoryError>(())
/// ```
#[derive(Debug, Clone)]
pub struct ManagedDirectory<C: DirectoryConfig> {
    /// Root path of the managed directory.
    root: PathBuf,

    /// The root location type (for debugging/logging).
    root_type: DirectoryRootType,

    /// Phantom data for the configuration type.
    _phantom: PhantomData<C>,
}

impl<C: DirectoryConfig> ManagedDirectory<C> {
    /// Create a new ManagedDirectory with the given root path.
    ///
    /// This private helper handles the common logic of ensuring the directory
    /// exists and constructing the struct.
    fn new(root: PathBuf, root_type: DirectoryRootType) -> Result<Self> {
        if !root.exists() {
            fs::create_dir_all(&root).map_err(|e| DirectoryError::directory_creation(&root, e))?;
        }

        let instance = Self {
            root,
            root_type,
            _phantom: PhantomData,
        };

        // Ensure .gitignore exists
        instance.write_gitignore_if_needed()?;

        // Create initialization subdirectories
        for subdir in C::init_subdirs() {
            instance.ensure_subdir(subdir)?;
        }

        Ok(instance)
    }

    /// Write .gitignore file if it doesn't exist.
    fn write_gitignore_if_needed(&self) -> Result<()> {
        let gitignore_path = self.root.join(".gitignore");

        // Only write if doesn't exist (let user manage if it exists)
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, C::GITIGNORE_CONTENT)
                .map_err(|e| DirectoryError::file_write(&gitignore_path, e))?;
            tracing::debug!(
                "Created .gitignore in {} directory: {}",
                C::DIR_NAME,
                gitignore_path.display()
            );
        }

        Ok(())
    }

    /// Create from Git repository root.
    ///
    /// Finds the Git repository root by walking up the directory tree and
    /// creates the managed directory there.
    ///
    /// # Errors
    ///
    /// Returns `NotInGitRepository` if not currently in a Git repository.
    pub fn from_git_root() -> Result<Self> {
        let git_root = find_git_repository_root().ok_or(DirectoryError::NotInGitRepository)?;
        let root = git_root.join(C::DIR_NAME);
        Self::new(root, DirectoryRootType::GitRoot)
    }

    /// Create from user's home directory.
    ///
    /// Creates the managed directory in the user's home directory.
    ///
    /// # Errors
    ///
    /// Returns `NoHomeDirectory` if the home directory cannot be determined.
    pub fn from_user_home() -> Result<Self> {
        let home = dirs::home_dir().ok_or(DirectoryError::NoHomeDirectory)?;
        let root = home.join(C::DIR_NAME);
        Self::new(root, DirectoryRootType::UserHome)
    }

    /// Create from a custom root path.
    ///
    /// Creates the managed directory under the specified custom root.
    /// This is primarily useful for testing with temporary directories.
    pub fn from_custom_root(custom_root: PathBuf) -> Result<Self> {
        let root = custom_root.join(C::DIR_NAME);
        Self::new(root, DirectoryRootType::Custom(custom_root))
    }

    /// Get the root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get a subdirectory path (does not create it).
    pub fn subdir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Get a subdirectory path, creating it if it doesn't exist.
    pub fn ensure_subdir(&self, name: &str) -> Result<PathBuf> {
        let path = self.root.join(name);
        fs::create_dir_all(&path).map_err(|e| DirectoryError::directory_creation(&path, e))?;
        Ok(path)
    }

    /// Get the root type.
    pub fn root_type(&self) -> &DirectoryRootType {
        &self.root_type
    }

    /// Get the directory name for this configuration.
    pub fn dir_name() -> &'static str {
        C::DIR_NAME
    }

    /// Check if a path is within the managed directory.
    pub fn contains_path(&self, path: &Path) -> bool {
        // Try canonicalized comparison first (handles symlinks)
        if let (Ok(canonical_path), Ok(canonical_root)) =
            (path.canonicalize(), self.root.canonicalize())
        {
            canonical_path.starts_with(canonical_root)
        } else {
            // Fallback to non-canonical comparison
            path.starts_with(&self.root)
        }
    }
}

/// Find the nearest git repository root by walking up the directory tree.
///
/// A `.git` entry (directory or file) marks a git root. Worktrees have a
/// `.git` file instead of a directory, but are still valid git roots.
///
/// Returns `Some(path)` if a git repository is found, `None` otherwise.
pub fn find_git_repository_root() -> Option<PathBuf> {
    find_git_repository_root_from(&std::env::current_dir().ok()?)
}

/// Find the nearest git repository root starting from a specific directory.
///
/// Walks up the directory tree looking for a `.git` entry (file or directory).
/// Returns the first directory that contains `.git`.
pub fn find_git_repository_root_from(start_dir: &Path) -> Option<PathBuf> {
    let mut path = start_dir;
    let max_depth = 20;
    let mut depth = 0;

    loop {
        if depth >= max_depth {
            break;
        }

        let git_path = path.join(".git");
        if git_path.exists() {
            return Some(path.to_path_buf());
        }

        match path.parent() {
            Some(parent) => {
                path = parent;
                depth += 1;
            }
            None => break,
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SwissarmyhammerConfig;
    use tempfile::TempDir;

    #[test]
    fn test_from_custom_root() {
        let temp = TempDir::new().unwrap();
        let dir =
            ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(temp.path().to_path_buf())
                .unwrap();

        assert!(dir.root().exists());
        assert!(dir.root().is_dir());
        assert_eq!(
            dir.root(),
            temp.path().join(SwissarmyhammerConfig::DIR_NAME)
        );
        assert_eq!(
            *dir.root_type(),
            DirectoryRootType::Custom(temp.path().to_path_buf())
        );
    }

    #[test]
    fn test_subdir() {
        let temp = TempDir::new().unwrap();
        let dir =
            ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(temp.path().to_path_buf())
                .unwrap();

        let todo_path = dir.subdir("todo");
        assert_eq!(todo_path, dir.root().join("todo"));
        // Directory should NOT exist yet (subdir doesn't create)
        assert!(!todo_path.exists());
    }

    #[test]
    fn test_ensure_subdir() {
        let temp = TempDir::new().unwrap();
        let dir =
            ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(temp.path().to_path_buf())
                .unwrap();

        let todo_dir = dir.ensure_subdir("todo").unwrap();
        assert_eq!(todo_dir, dir.root().join("todo"));
        // Directory SHOULD exist now
        assert!(todo_dir.exists());
        assert!(todo_dir.is_dir());
    }

    #[test]
    fn test_gitignore_created() {
        let temp = TempDir::new().unwrap();
        let dir =
            ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(temp.path().to_path_buf())
                .unwrap();

        let gitignore_path = dir.root().join(".gitignore");
        assert!(gitignore_path.exists(), ".gitignore should be created");

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("tmp/"), ".gitignore should contain tmp/");
    }

    #[test]
    fn test_init_subdirs_created() {
        let temp = TempDir::new().unwrap();
        let dir =
            ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(temp.path().to_path_buf())
                .unwrap();

        // SwissarmyhammerConfig creates "tmp" on init
        let tmp_path = dir.subdir("tmp");
        assert!(tmp_path.exists(), "tmp/ should be created on init");
    }

    #[test]
    fn test_contains_path() {
        let temp = TempDir::new().unwrap();
        let dir =
            ManagedDirectory::<SwissarmyhammerConfig>::from_custom_root(temp.path().to_path_buf())
                .unwrap();

        // Create a file inside
        let todo_dir = dir.ensure_subdir("todo").unwrap();
        let todo_file = todo_dir.join("todo.yaml");
        fs::write(&todo_file, "test").unwrap();

        assert!(dir.contains_path(&todo_file));

        // File outside should return false
        let outside_file = temp.path().join("outside.txt");
        fs::write(&outside_file, "test").unwrap();
        assert!(!dir.contains_path(&outside_file));
    }

    #[test]
    fn test_dir_name() {
        assert_eq!(
            ManagedDirectory::<SwissarmyhammerConfig>::dir_name(),
            ".swissarmyhammer"
        );
    }

    #[test]
    fn test_root_type_display() {
        assert_eq!(DirectoryRootType::UserHome.to_string(), "user home");
        assert_eq!(
            DirectoryRootType::GitRoot.to_string(),
            "git repository root"
        );

        let custom = DirectoryRootType::Custom(PathBuf::from("/tmp/test"));
        assert!(custom.to_string().contains("custom path"));
        assert!(custom.to_string().contains("/tmp/test"));
    }

    #[test]
    fn test_find_git_root_from_direct() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let result = find_git_repository_root_from(temp.path());
        assert_eq!(result, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_find_git_root_from_subdirectory() {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let subdir = temp.path().join("a").join("b").join("c");
        fs::create_dir_all(&subdir).unwrap();

        let result = find_git_repository_root_from(&subdir);
        assert_eq!(result, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_find_git_root_not_found() {
        let temp = TempDir::new().unwrap();
        // No .git anywhere
        let result = find_git_repository_root_from(temp.path());
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_git_root_returns_nearest_not_outermost() {
        let temp = TempDir::new().unwrap();
        // Outer git repo
        let outer_git = temp.path().join(".git");
        fs::create_dir_all(&outer_git).unwrap();
        // Inner git repo (nested)
        let inner = temp.path().join("inner");
        let inner_git = inner.join(".git");
        fs::create_dir_all(&inner_git).unwrap();

        // Starting from inside the inner repo should find the NEAREST root
        let result = find_git_repository_root_from(&inner);
        assert_eq!(result, Some(inner));
    }

    #[test]
    fn test_find_git_root_treats_worktree_as_root() {
        let temp = TempDir::new().unwrap();
        // Create main repo with .git directory
        let main_repo = temp.path().join("main");
        let main_git = main_repo.join(".git");
        let worktrees_dir = main_git.join("worktrees").join("feature");
        fs::create_dir_all(&worktrees_dir).unwrap();

        // Create worktree with .git file (not directory)
        let worktree = temp.path().join("feature");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}", worktrees_dir.display()),
        )
        .unwrap();

        // Worktree should be treated as its own git root
        let result = find_git_repository_root_from(&worktree);
        assert_eq!(result, Some(worktree));
    }
}
