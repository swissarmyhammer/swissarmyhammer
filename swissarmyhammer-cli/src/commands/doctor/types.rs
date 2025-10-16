//! Type definitions for the doctor module

use std::path::{Path, PathBuf};

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

/// Status of a diagnostic check
#[derive(Debug, PartialEq, Clone)]
pub enum CheckStatus {
    /// Check passed without issues
    Ok,
    /// Check passed but with potential issues
    Warning,
    /// Check failed with errors
    Error,
}

/// Exit codes for the doctor command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// All checks passed
    Success = 0,
    /// Warnings detected
    Warning = 1,
    /// Errors detected
    Error = 2,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> i32 {
        code as i32
    }
}

/// Result of a single diagnostic check
#[derive(Debug, Clone)]
pub struct Check {
    /// Name of the check performed
    pub name: String,
    /// Status of the check (Ok, Warning, Error)
    pub status: CheckStatus,
    /// Descriptive message about the check result
    pub message: String,
    /// Optional fix suggestion for warnings or errors
    pub fix: Option<String>,
}

impl Check {
    // Builder pattern methods are currently unused but kept for potential future use
    #[allow(dead_code)]
    /// Create a new Check builder
    ///
    /// # Example
    ///
    /// ```
    /// use swissarmyhammer_cli::commands::doctor::types::{Check, CheckStatus};
    ///
    /// let check = Check::builder("Test Check", CheckStatus::Ok)
    ///     .with_message("Everything is working")
    ///     .with_fix("No fix needed")
    ///     .build();
    /// ```
    pub fn builder(name: impl Into<String>, status: CheckStatus) -> CheckBuilder {
        CheckBuilder::new(name, status)
    }
}

#[allow(dead_code)]
/// Builder for creating Check instances
pub struct CheckBuilder {
    name: String,
    status: CheckStatus,
    message: String,
    fix: Option<String>,
}

#[allow(dead_code)]
impl CheckBuilder {
    /// Create a new CheckBuilder
    pub fn new(name: impl Into<String>, status: CheckStatus) -> Self {
        Self {
            name: name.into(),
            status,
            message: String::new(),
            fix: None,
        }
    }
    /// Set the message for this check
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the fix suggestion for this check
    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    /// Build the Check instance
    pub fn build(self) -> Check {
        Check {
            name: self.name,
            status: self.status,
            message: self.message,
            fix: self.fix,
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
    fn test_disk_space_from_mb() {
        let space = DiskSpace::from_mb(100);
        assert_eq!(space.as_mb(), 100);
    }

    #[test]
    fn test_disk_space_is_low() {
        let space = DiskSpace::from_mb(50);
        assert!(space.is_low(100));
        assert!(!space.is_low(50));
        assert!(!space.is_low(40));
    }

    #[test]
    fn test_disk_space_display() {
        let space = DiskSpace::from_mb(1024);
        assert_eq!(format!("{space}"), "1024 MB");
    }

    #[test]
    fn test_disk_space_ordering() {
        let space1 = DiskSpace::from_mb(100);
        let space2 = DiskSpace::from_mb(200);
        let space3 = DiskSpace::from_mb(100);

        assert!(space1 < space2);
        assert!(space2 > space1);
        assert_eq!(space1, space3);
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

    #[test]
    fn test_check_builder_minimal() {
        let check = Check::builder("Test Check", CheckStatus::Ok).build();
        assert_eq!(check.name, "Test Check");
        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(check.message, "");
        assert_eq!(check.fix, None);
    }

    #[test]
    fn test_check_builder_with_message() {
        let check = Check::builder("Test Check", CheckStatus::Warning)
            .with_message("This is a warning")
            .build();
        assert_eq!(check.name, "Test Check");
        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(check.message, "This is a warning");
        assert_eq!(check.fix, None);
    }

    #[test]
    fn test_check_builder_with_fix() {
        let check = Check::builder("Test Check", CheckStatus::Error)
            .with_message("Something is wrong")
            .with_fix("Try this to fix it")
            .build();
        assert_eq!(check.name, "Test Check");
        assert_eq!(check.status, CheckStatus::Error);
        assert_eq!(check.message, "Something is wrong");
        assert_eq!(check.fix, Some("Try this to fix it".to_string()));
    }

    #[test]
    fn test_check_builder_string_conversion() {
        let check = Check::builder(String::from("Test"), CheckStatus::Ok)
            .with_message(String::from("Message"))
            .with_fix(String::from("Fix"))
            .build();
        assert_eq!(check.name, "Test");
        assert_eq!(check.message, "Message");
        assert_eq!(check.fix, Some("Fix".to_string()));
    }
}
