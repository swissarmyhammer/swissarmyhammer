//! Type definitions for the doctor module
//!
//! Re-exports core types from swissarmyhammer-doctor and provides
//! sah-specific types like WorkflowDirectory.

use std::path::{Path, PathBuf};

// Re-export core doctor types from the shared crate
pub use swissarmyhammer_doctor::{Check, CheckStatus, ExitCode};

/// Wrapper type for workflow directory paths to provide type safety
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDirectory(PathBuf);

impl WorkflowDirectory {
    /// Create a new WorkflowDirectory from a PathBuf
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the workflow directory
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use swissarmyhammer_cli::commands::doctor::types::WorkflowDirectory;
    ///
    /// let dir = WorkflowDirectory::new(PathBuf::from("/home/user/.swissarmyhammer/workflows"));
    /// ```
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// Get the underlying path
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use swissarmyhammer_cli::commands::doctor::types::WorkflowDirectory;
    ///
    /// let dir = WorkflowDirectory::new(PathBuf::from("/test"));
    /// assert_eq!(dir.path(), Path::new("/test"));
    /// ```
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for WorkflowDirectory {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for WorkflowDirectory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

/// Information about a workflow directory including its path and category
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDirectoryInfo {
    pub path: WorkflowDirectory,
    pub category: WorkflowCategory,
}

impl WorkflowDirectoryInfo {
    /// Create a new WorkflowDirectoryInfo
    ///
    /// # Arguments
    ///
    /// * `path` - The workflow directory path
    /// * `category` - The category of the workflow directory (User or Local)
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use swissarmyhammer_cli::commands::doctor::types::{WorkflowDirectory, WorkflowDirectoryInfo, WorkflowCategory};
    ///
    /// let dir = WorkflowDirectory::new(PathBuf::from("/home/user/.swissarmyhammer/workflows"));
    /// let info = WorkflowDirectoryInfo::new(dir, WorkflowCategory::User);
    /// ```
    pub fn new(path: WorkflowDirectory, category: WorkflowCategory) -> Self {
        Self { path, category }
    }
}

/// Category of workflow directory (User or Local)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCategory {
    User,
    Local,
}

impl std::fmt::Display for WorkflowCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowCategory::User => write!(f, "User"),
            WorkflowCategory::Local => write!(f, "Local"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_workflow_directory_new() {
        let path = PathBuf::from("/test/workflows");
        let dir = WorkflowDirectory::new(path.clone());
        assert_eq!(dir.path(), &path);
    }

    #[test]
    fn test_workflow_directory_as_ref() {
        let path = PathBuf::from("/test/workflows");
        let dir = WorkflowDirectory::new(path.clone());
        let path_ref: &Path = dir.as_ref();
        assert_eq!(path_ref, &path);
    }

    #[test]
    fn test_workflow_directory_display() {
        let path = PathBuf::from("/test/workflows");
        let dir = WorkflowDirectory::new(path);
        let display = format!("{dir}");
        assert!(display.contains("/test/workflows"));
    }

    #[test]
    fn test_workflow_directory_equality() {
        let dir1 = WorkflowDirectory::new(PathBuf::from("/test"));
        let dir2 = WorkflowDirectory::new(PathBuf::from("/test"));
        let dir3 = WorkflowDirectory::new(PathBuf::from("/other"));
        assert_eq!(dir1, dir2);
        assert_ne!(dir1, dir3);
    }

    #[test]
    fn test_workflow_directory_info_new() {
        let dir = WorkflowDirectory::new(PathBuf::from("/test"));
        let info = WorkflowDirectoryInfo::new(dir.clone(), WorkflowCategory::User);
        assert_eq!(info.path, dir);
        assert_eq!(info.category, WorkflowCategory::User);
    }

    #[test]
    fn test_workflow_category_display() {
        assert_eq!(format!("{}", WorkflowCategory::User), "User");
        assert_eq!(format!("{}", WorkflowCategory::Local), "Local");
    }

    #[test]
    fn test_workflow_category_equality() {
        assert_eq!(WorkflowCategory::User, WorkflowCategory::User);
        assert_ne!(WorkflowCategory::User, WorkflowCategory::Local);
    }

    #[test]
    fn test_check_status_equality() {
        assert_eq!(CheckStatus::Ok, CheckStatus::Ok);
        assert_ne!(CheckStatus::Ok, CheckStatus::Warning);
        assert_ne!(CheckStatus::Warning, CheckStatus::Error);
    }

    #[test]
    fn test_exit_code_conversion() {
        assert_eq!(i32::from(ExitCode::Success), 0);
        assert_eq!(i32::from(ExitCode::Warning), 1);
        assert_eq!(i32::from(ExitCode::Error), 2);
    }

    #[test]
    fn test_exit_code_equality() {
        assert_eq!(ExitCode::Success, ExitCode::Success);
        assert_ne!(ExitCode::Success, ExitCode::Warning);
    }
}
