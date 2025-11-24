//! SwissArmyHammer directory structure management
//!
//! This module provides a centralized representation of the .swissarmyhammer directory
//! structure, supporting different root locations (Git root, user home, custom paths).

use crate::error::{Result, SwissArmyHammerError};
use crate::utils::directory_utils::find_git_repository_root;
use std::fs;
use std::path::{Path, PathBuf};

/// Content for .gitignore file in .swissarmyhammer directory
///
/// This ensures temporary files, logs, and runtime artifacts are not committed
/// while allowing important project files like rules/ and docs/ to be tracked.
const GITIGNORE_CONTENT: &str = r#"# SwissArmyHammer temporary files and logs
# This file is automatically created by swissarmyhammer-common

# Temporary files
tmp/
*.tmp

# Todo tracking (ephemeral development session tracking)
todo/

# Abort signals (workflow control files)
.abort

# Logs
*.log
mcp.log

# Workflow execution state
workflow-runs/

# Transcripts (conversation history)
transcripts/

# Question/Answer cache
questions/

# Keep these directories (they should be committed):
# - rules/      Project-specific code quality rules
# - docs/       Project documentation
"#;

/// Represents the type of root location for a SwissArmyHammer directory
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryRootType {
    /// In user's home directory (~/.swissarmyhammer)
    UserHome,

    /// At Git repository root (./.swissarmyhammer)
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

/// Represents the .swissarmyhammer directory structure
///
/// This struct provides a centralized, type-safe way to access the .swissarmyhammer
/// directory and its subdirectories. It supports multiple root locations and
/// provides consistent path resolution across the application.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_common::SwissarmyhammerDirectory;
///
/// // Create from Git repository root
/// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
///
/// // Get a subdirectory, creating it if needed
/// let todo_dir = sah_dir.ensure_subdir("todo")?;
///
/// // Just get the path without creating
/// let rules_path = sah_dir.subdir("rules");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct SwissarmyhammerDirectory {
    /// Root path of the .swissarmyhammer directory
    root: PathBuf,

    /// The root location type (for debugging/logging)
    root_type: DirectoryRootType,
}

impl SwissarmyhammerDirectory {
    /// Create a new SwissarmyhammerDirectory with the given root path
    ///
    /// This private helper handles the common logic of ensuring the directory
    /// exists and constructing the struct.
    ///
    /// # Arguments
    ///
    /// * `root` - The path to the .swissarmyhammer directory
    /// * `root_type` - The type of root location
    ///
    /// # Returns
    ///
    /// * `Ok(SwissarmyhammerDirectory)` - Successfully created/found directory
    ///
    /// # Errors
    ///
    /// * `DirectoryCreation` - If .swissarmyhammer directory cannot be created
    fn new(root: PathBuf, root_type: DirectoryRootType) -> Result<Self> {
        if !root.exists() {
            fs::create_dir_all(&root).map_err(SwissArmyHammerError::directory_creation)?;
        }

        let instance = Self { root, root_type };

        // Ensure .gitignore exists in the .swissarmyhammer directory
        instance.write_gitignore_if_needed()?;

        Ok(instance)
    }

    /// Write .gitignore file if it doesn't exist or needs updating
    ///
    /// This ensures the .swissarmyhammer directory has a .gitignore file that
    /// excludes temporary files, logs, and runtime artifacts while allowing
    /// important project files to be committed.
    ///
    /// # Errors
    ///
    /// * Returns error if .gitignore file cannot be written
    fn write_gitignore_if_needed(&self) -> Result<()> {
        let gitignore_path = self.root.join(".gitignore");

        // Always write if doesn't exist
        // If it exists, we let the user manage it (don't overwrite)
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, GITIGNORE_CONTENT).map_err(|e| {
                SwissArmyHammerError::Other {
                    message: format!(
                        "Failed to write .gitignore to {}: {}",
                        gitignore_path.display(),
                        e
                    ),
                }
            })?;
            tracing::debug!(
                "Created .gitignore in .swissarmyhammer directory: {}",
                gitignore_path.display()
            );
        }

        Ok(())
    }

    /// Create from Git repository root (default for project operations)
    ///
    /// Finds the Git repository root and creates .swissarmyhammer directory there.
    ///
    /// # Returns
    ///
    /// * `Ok(SwissarmyhammerDirectory)` - Successfully created/found directory
    ///
    /// # Errors
    ///
    /// * `NotInGitRepository` - If not currently in a Git repository
    /// * `DirectoryCreation` - If .swissarmyhammer directory cannot be created
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
    /// println!("Using .swissarmyhammer at: {}", sah_dir.root().display());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_git_root() -> Result<Self> {
        let git_root =
            find_git_repository_root().ok_or(SwissArmyHammerError::NotInGitRepository)?;
        let root = git_root.join(".swissarmyhammer");
        Self::new(root, DirectoryRootType::GitRoot)
    }

    /// Create from user's home directory (for user-level config/rules)
    ///
    /// Creates .swissarmyhammer directory in the user's home directory.
    ///
    /// # Returns
    ///
    /// * `Ok(SwissarmyhammerDirectory)` - Successfully created/found directory
    ///
    /// # Errors
    ///
    /// * Returns error if home directory cannot be determined or directory cannot be created
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_user_home()?;
    /// println!("Using .swissarmyhammer at: {}", sah_dir.root().display());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_user_home() -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| SwissArmyHammerError::Other {
            message: "Cannot determine home directory".to_string(),
        })?;

        let root = home.join(".swissarmyhammer");
        Self::new(root, DirectoryRootType::UserHome)
    }

    /// Create from custom root path (for testing)
    ///
    /// Creates .swissarmyhammer directory under the specified custom root.
    /// This is primarily useful for testing with temporary directories.
    ///
    /// # Arguments
    ///
    /// * `custom_root` - The root path where .swissarmyhammer should be created
    ///
    /// # Returns
    ///
    /// * `Ok(SwissarmyhammerDirectory)` - Successfully created/found directory
    ///
    /// # Errors
    ///
    /// * Returns error if directory cannot be created
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    /// use std::path::PathBuf;
    ///
    /// let custom_root = PathBuf::from("/tmp/test");
    /// let sah_dir = SwissarmyhammerDirectory::from_custom_root(custom_root)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_custom_root(custom_root: PathBuf) -> Result<Self> {
        let root = custom_root.join(".swissarmyhammer");
        Self::new(root, DirectoryRootType::Custom(custom_root))
    }

    /// Get the root .swissarmyhammer directory path
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
    /// println!("Root: {}", sah_dir.root().display());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get a subdirectory path (does not create it)
    ///
    /// Tools request subdirectories by name. This method returns the path
    /// but does not create the directory.
    ///
    /// # Arguments
    ///
    /// * `name` - Subdirectory name (e.g., "todo", "rules", "tmp")
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
    /// let todo_path = sah_dir.subdir("todo");
    /// // Directory may not exist yet
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn subdir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Get a subdirectory path, creating it if it doesn't exist
    ///
    /// Tools request subdirectories by name and this ensures they exist.
    ///
    /// # Arguments
    ///
    /// * `name` - Subdirectory name (e.g., "todo", "rules", "tmp")
    ///
    /// # Returns
    ///
    /// * `Ok(PathBuf)` - Path to the subdirectory (guaranteed to exist)
    ///
    /// # Errors
    ///
    /// * Returns error if directory cannot be created
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
    /// let todo_dir = sah_dir.ensure_subdir("todo")?;
    /// // Directory is guaranteed to exist now
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn ensure_subdir(&self, name: &str) -> Result<PathBuf> {
        let path = self.root.join(name);
        fs::create_dir_all(&path).map_err(SwissArmyHammerError::directory_creation)?;
        Ok(path)
    }

    /// Get the root type
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
    /// println!("Root type: {}", sah_dir.root_type());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn root_type(&self) -> &DirectoryRootType {
        &self.root_type
    }

    /// Check if a path is within the .swissarmyhammer directory
    ///
    /// Uses canonicalized path comparison for accuracy.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to check
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_common::SwissarmyhammerDirectory;
    /// use std::path::Path;
    ///
    /// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
    /// let path = Path::new(".swissarmyhammer/todo/todo.yaml");
    /// assert!(sah_dir.contains_path(path));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn contains_path(&self, path: &Path) -> bool {
        // Try canonicalized comparison first (handles symlinks)
        if let (Ok(canonical_path), Ok(canonical_root)) =
            (path.canonicalize(), self.root.canonicalize())
        {
            canonical_path.starts_with(canonical_root)
        } else {
            // Fallback to non-canonical comparison if canonicalization fails
            path.starts_with(&self.root)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_from_custom_root() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        assert!(sah_dir.root().exists());
        assert!(sah_dir.root().is_dir());
        assert_eq!(sah_dir.root(), temp.path().join(".swissarmyhammer"));
        assert_eq!(
            *sah_dir.root_type(),
            DirectoryRootType::Custom(temp.path().to_path_buf())
        );
    }

    #[test]
    fn test_from_git_root_not_in_repo() {
        let temp = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        std::env::set_current_dir(temp.path()).unwrap();
        let result = SwissarmyhammerDirectory::from_git_root();
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_err());
        match result {
            Err(SwissArmyHammerError::NotInGitRepository) => {}
            _ => panic!("Expected NotInGitRepository error"),
        }
    }

    #[test]
    fn test_from_git_root_in_repo() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let sah_dir = SwissarmyhammerDirectory::from_git_root().unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        assert!(sah_dir.root().exists());
        assert_eq!(*sah_dir.root_type(), DirectoryRootType::GitRoot);
    }

    #[test]
    fn test_subdir() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let todo_path = sah_dir.subdir("todo");
        assert_eq!(todo_path, sah_dir.root().join("todo"));
        // Directory should NOT exist yet
        assert!(!todo_path.exists());
    }

    #[test]
    fn test_ensure_subdir() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let todo_dir = sah_dir.ensure_subdir("todo").unwrap();
        assert_eq!(todo_dir, sah_dir.root().join("todo"));
        // Directory SHOULD exist now
        assert!(todo_dir.exists());
        assert!(todo_dir.is_dir());
    }

    #[test]
    fn test_ensure_subdir_nested() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let nested_dir = sah_dir.ensure_subdir("cache/rules").unwrap();
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
        assert_eq!(nested_dir, sah_dir.root().join("cache/rules"));
    }

    #[test]
    fn test_contains_path() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        // Create a file inside .swissarmyhammer
        let todo_dir = sah_dir.ensure_subdir("todo").unwrap();
        let todo_file = todo_dir.join("todo.yaml");
        fs::write(&todo_file, "test").unwrap();

        // Should detect file is inside .swissarmyhammer
        assert!(sah_dir.contains_path(&todo_file));

        // File outside .swissarmyhammer should return false
        let outside_file = temp.path().join("outside.txt");
        fs::write(&outside_file, "test").unwrap();
        assert!(!sah_dir.contains_path(&outside_file));
    }

    #[test]
    fn test_contains_path_relative() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        // contains_path uses canonicalize which requires paths to exist
        // So we test with the actual root path
        assert!(sah_dir.contains_path(sah_dir.root()));
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
    fn test_gitignore_created_on_init() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let gitignore_path = sah_dir.root().join(".gitignore");
        assert!(gitignore_path.exists(), ".gitignore should be created");

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(
            content.contains("tmp/"),
            ".gitignore should contain tmp/"
        );
        assert!(
            content.contains("todo/"),
            ".gitignore should contain todo/"
        );
        assert!(
            content.contains("*.log"),
            ".gitignore should contain *.log"
        );
        assert!(
            content.contains(".abort"),
            ".gitignore should contain .abort"
        );
    }

    #[test]
    fn test_gitignore_not_overwritten() {
        let temp = TempDir::new().unwrap();

        // Create .swissarmyhammer directory manually with custom .gitignore
        let sah_path = temp.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_path).unwrap();

        let custom_content = "# Custom gitignore\n*.custom\n";
        let gitignore_path = sah_path.join(".gitignore");
        fs::write(&gitignore_path, custom_content).unwrap();

        // Now create SwissarmyhammerDirectory - should not overwrite
        let _sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(
            content, custom_content,
            ".gitignore should not be overwritten"
        );
    }

    #[test]
    fn test_gitignore_created_from_git_root() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let sah_dir = SwissarmyhammerDirectory::from_git_root().unwrap();
        let gitignore_path = sah_dir.root().join(".gitignore");

        // Change back to original directory before assertions
        // to ensure we can read the file
        std::env::set_current_dir(&original_dir).unwrap();

        assert!(
            gitignore_path.exists(),
            ".gitignore should be created in Git root at {}",
            gitignore_path.display()
        );

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("SwissArmyHammer temporary files"));
    }
}
