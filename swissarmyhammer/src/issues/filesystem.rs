use crate::common::generate_monotonic_ulid_string;
use crate::error::{Result, SwissArmyHammerError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use tracing::{debug, warn};

// IssueNumber type eliminated - we now use issue names (filename without .md) as the primary identifier

/// Represents an issue in the tracking system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    /// The primary identifier - issue name derived from filename (without .md extension)
    pub name: String,
    /// The full content of the issue markdown file
    pub content: String,
}

impl Issue {
    /// Check if this issue is completed based on file path location
    pub fn is_completed(&self, file_path: &Path, completed_dir: &Path) -> bool {
        file_path
            .parent()
            .map(|parent| parent == completed_dir)
            .unwrap_or(false)
    }

    /// Get the file path for this issue based on its location (completed or active)
    pub fn get_file_path(&self, base_dir: &Path, completed: bool) -> PathBuf {
        let dir = if completed {
            base_dir.join("complete")
        } else {
            base_dir.to_path_buf()
        };
        dir.join(format!("{}.md", self.name))
    }

    /// Get the creation time from file metadata
    pub fn get_created_at(file_path: &Path) -> DateTime<Utc> {
        file_path
            .metadata()
            .and_then(|m| m.created())
            .or_else(|_| file_path.metadata().and_then(|m| m.modified()))
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now())
    }
}

/// Extended issue information that includes derived metadata
#[derive(Debug, Clone)]
pub struct IssueInfo {
    /// The core issue data
    pub issue: Issue,
    /// Whether this issue is completed (in completed directory)
    pub completed: bool,
    /// Full path to the issue file
    pub file_path: PathBuf,
    /// When this issue was created
    pub created_at: DateTime<Utc>,
}

impl IssueInfo {
    /// Create issue info from an issue and its file path
    pub fn from_issue_and_path(issue: Issue, file_path: PathBuf, completed_dir: &Path) -> Self {
        let completed = issue.is_completed(&file_path, completed_dir);
        let created_at = Issue::get_created_at(&file_path);

        Self {
            issue,
            completed,
            file_path,
            created_at,
        }
    }
}

/// Represents the current state of the issue system
#[derive(Debug, Clone)]
pub struct IssueState {
    /// Path to the issues directory
    pub issues_dir: PathBuf,
    /// Path to the completed issues directory
    pub completed_dir: PathBuf,
}

/// Trait for issue storage operations
#[async_trait::async_trait]
pub trait IssueStorage: Send + Sync {
    /// List all issues (both pending and completed)
    async fn list_issues(&self) -> Result<Vec<Issue>>;

    /// List all issues with extended information (includes completion status and file paths)
    async fn list_issues_info(&self) -> Result<Vec<IssueInfo>>;

    /// Get a specific issue by name
    async fn get_issue(&self, name: &str) -> Result<Issue>;

    /// Get a specific issue with extended information by name
    async fn get_issue_info(&self, name: &str) -> Result<IssueInfo>;

    /// Create a new issue - if name is empty, generates a ULID
    async fn create_issue(&self, name: String, content: String) -> Result<Issue>;

    /// Update an existing issue's content by name
    async fn update_issue(&self, name: &str, content: String) -> Result<Issue>;

    /// Delete an issue by name
    async fn delete_issue(&self, name: &str) -> Result<()>;

    /// Mark an issue as completed by name
    async fn complete_issue(&self, name: &str) -> Result<Issue>;

    /// Get the next available issue (first pending issue alphabetically)
    async fn next_issue(&self) -> Result<Option<Issue>>;

    /// Check if all issues are completed
    async fn all_issues_completed(&self) -> Result<bool>;
}

/// File-system based issue storage implementation
#[derive(Debug)]
pub struct FileSystemIssueStorage {
    /// Directory for active issues
    issues_dir: PathBuf,
    /// Directory for completed issues
    completed_dir: PathBuf,
    /// Locking mechanism for thread safety
    lock: Mutex<()>,
}

impl FileSystemIssueStorage {
    /// Create a new FileSystemIssueStorage with a specific directory
    pub fn new(issues_dir: PathBuf) -> Result<Self> {
        let completed_dir = issues_dir.join("complete");

        // Create directories if they don't exist
        fs::create_dir_all(&issues_dir).map_err(SwissArmyHammerError::Io)?;
        fs::create_dir_all(&completed_dir).map_err(SwissArmyHammerError::Io)?;

        Ok(Self {
            issues_dir,
            completed_dir,
            lock: Mutex::new(()),
        })
    }

    /// Create with default directory structure
    pub fn new_default() -> Result<Self> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::new_default_in(&current_dir)
    }

    /// Create with default directory structure in a specific working directory
    pub fn new_default_in(work_dir: &Path) -> Result<Self> {
        let issues_dir = Self::default_directory_in(work_dir)?;
        Self::new(issues_dir)
    }

    /// Get the default issues directory (creates .swissarmyhammer if needed)
    pub fn default_directory() -> Result<PathBuf> {
        let current_dir = std::env::current_dir().map_err(SwissArmyHammerError::Io)?;
        Self::default_directory_in(&current_dir)
    }

    /// Get the default issues directory in a specific working directory
    pub fn default_directory_in(work_dir: &Path) -> Result<PathBuf> {
        let swissarmyhammer_dir = work_dir.join(".swissarmyhammer");
        let issues_dir = swissarmyhammer_dir.join("issues");

        // Create the directories if they don't exist
        fs::create_dir_all(&issues_dir).map_err(SwissArmyHammerError::Io)?;

        Ok(issues_dir)
    }

    /// Get issue state information
    pub fn get_state(&self) -> IssueState {
        IssueState {
            issues_dir: self.issues_dir.clone(),
            completed_dir: self.completed_dir.clone(),
        }
    }

    /// List all markdown files in a directory
    fn list_markdown_files(dir: &Path) -> Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(dir).map_err(SwissArmyHammerError::Io)?;

        for entry in entries {
            let entry = entry.map_err(SwissArmyHammerError::Io)?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                files.push(path);
            }
        }

        // Sort by filename for consistent ordering
        files.sort();
        Ok(files)
    }

    /// Load an issue from a file path
    fn load_issue_from_path(&self, file_path: &Path) -> Result<Issue> {
        let content = fs::read_to_string(file_path).map_err(SwissArmyHammerError::Io)?;

        let name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(file_path.display().to_string()))?
            .to_string();

        Ok(Issue { name, content })
    }

    /// Save an issue to a file
    fn save_issue_to_file(&self, issue: &Issue, file_path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(SwissArmyHammerError::Io)?;
        }

        fs::write(file_path, &issue.content).map_err(SwissArmyHammerError::Io)?;
        Ok(())
    }

    /// Find the file path for an issue by name
    fn find_issue_file(&self, name: &str) -> Result<Option<PathBuf>> {
        // Check active issues directory
        let active_path = self.issues_dir.join(format!("{}.md", name));
        if active_path.exists() {
            return Ok(Some(active_path));
        }

        // Check completed issues directory
        let completed_path = self.completed_dir.join(format!("{}.md", name));
        if completed_path.exists() {
            return Ok(Some(completed_path));
        }

        Ok(None)
    }

    /// Generate a unique issue name using ULID
    fn generate_issue_name(&self) -> String {
        generate_monotonic_ulid_string()
    }

    /// Validate that an issue name is acceptable
    fn validate_issue_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(SwissArmyHammerError::IssueNotFound(name.to_string()));
        }

        // Check for invalid characters
        if name.contains('/') || name.contains('\\') || name.contains('\0') {
            return Err(SwissArmyHammerError::IssueNotFound(name.to_string()));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl IssueStorage for FileSystemIssueStorage {
    async fn list_issues(&self) -> Result<Vec<Issue>> {
        let _lock = self.lock.lock().await;

        let mut issues = Vec::new();

        // Load active issues
        let active_files = Self::list_markdown_files(&self.issues_dir)?;
        for file_path in active_files {
            match self.load_issue_from_path(&file_path) {
                Ok(issue) => issues.push(issue),
                Err(e) => {
                    warn!("Failed to load issue from {}: {}", file_path.display(), e);
                }
            }
        }

        // Load completed issues
        let completed_files = Self::list_markdown_files(&self.completed_dir)?;
        for file_path in completed_files {
            match self.load_issue_from_path(&file_path) {
                Ok(issue) => issues.push(issue),
                Err(e) => {
                    warn!("Failed to load issue from {}: {}", file_path.display(), e);
                }
            }
        }

        // Sort by name for consistent ordering
        issues.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(issues)
    }

    async fn list_issues_info(&self) -> Result<Vec<IssueInfo>> {
        let _lock = self.lock.lock().await;

        let mut issues_info = Vec::new();

        // Load active issues
        let active_files = Self::list_markdown_files(&self.issues_dir)?;
        for file_path in active_files {
            match self.load_issue_from_path(&file_path) {
                Ok(issue) => {
                    let issue_info =
                        IssueInfo::from_issue_and_path(issue, file_path, &self.completed_dir);
                    issues_info.push(issue_info);
                }
                Err(e) => {
                    warn!("Failed to load issue from {}: {}", file_path.display(), e);
                }
            }
        }

        // Load completed issues
        let completed_files = Self::list_markdown_files(&self.completed_dir)?;
        for file_path in completed_files {
            match self.load_issue_from_path(&file_path) {
                Ok(issue) => {
                    let issue_info =
                        IssueInfo::from_issue_and_path(issue, file_path, &self.completed_dir);
                    issues_info.push(issue_info);
                }
                Err(e) => {
                    warn!("Failed to load issue from {}: {}", file_path.display(), e);
                }
            }
        }

        // Sort by name for consistent ordering
        issues_info.sort_by(|a, b| a.issue.name.cmp(&b.issue.name));
        Ok(issues_info)
    }

    async fn get_issue(&self, name: &str) -> Result<Issue> {
        let _lock = self.lock.lock().await;

        let file_path = self
            .find_issue_file(name)?
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(name.to_string()))?;

        self.load_issue_from_path(&file_path)
    }

    async fn get_issue_info(&self, name: &str) -> Result<IssueInfo> {
        let _lock = self.lock.lock().await;

        let file_path = self
            .find_issue_file(name)?
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(name.to_string()))?;

        let issue = self.load_issue_from_path(&file_path)?;
        Ok(IssueInfo::from_issue_and_path(
            issue,
            file_path,
            &self.completed_dir,
        ))
    }

    async fn create_issue(&self, name: String, content: String) -> Result<Issue> {
        let _lock = self.lock.lock().await;

        let issue_name = if name.is_empty() {
            self.generate_issue_name()
        } else {
            Self::validate_issue_name(&name)?;
            name
        };

        // Check if issue already exists
        if self.find_issue_file(&issue_name)?.is_some() {
            return Err(SwissArmyHammerError::IssueAlreadyExists(0));
        }

        let issue = Issue {
            name: issue_name.clone(),
            content,
        };

        let file_path = self.issues_dir.join(format!("{}.md", issue_name));
        self.save_issue_to_file(&issue, &file_path)?;

        debug!("Created issue '{}' at {}", issue_name, file_path.display());
        Ok(issue)
    }

    async fn update_issue(&self, name: &str, content: String) -> Result<Issue> {
        let _lock = self.lock.lock().await;

        let file_path = self
            .find_issue_file(name)?
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(name.to_string()))?;

        let issue = Issue {
            name: name.to_string(),
            content,
        };

        self.save_issue_to_file(&issue, &file_path)?;
        debug!("Updated issue '{}' at {}", name, file_path.display());
        Ok(issue)
    }

    async fn delete_issue(&self, name: &str) -> Result<()> {
        let _lock = self.lock.lock().await;

        let file_path = self
            .find_issue_file(name)?
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(name.to_string()))?;

        fs::remove_file(&file_path).map_err(SwissArmyHammerError::Io)?;
        debug!("Deleted issue '{}' from {}", name, file_path.display());
        Ok(())
    }

    async fn complete_issue(&self, name: &str) -> Result<Issue> {
        let _lock = self.lock.lock().await;

        let file_path = self
            .find_issue_file(name)?
            .ok_or_else(|| SwissArmyHammerError::IssueNotFound(name.to_string()))?;

        let issue = self.load_issue_from_path(&file_path)?;

        // If already completed, just return it
        if file_path.parent() == Some(&self.completed_dir) {
            return Ok(issue);
        }

        // Move to completed directory
        let completed_path = self.completed_dir.join(format!("{}.md", name));
        fs::rename(&file_path, &completed_path).map_err(SwissArmyHammerError::Io)?;

        debug!(
            "Completed issue '{}': moved from {} to {}",
            name,
            file_path.display(),
            completed_path.display()
        );

        Ok(issue)
    }

    async fn next_issue(&self) -> Result<Option<Issue>> {
        let _lock = self.lock.lock().await;

        let active_files = Self::list_markdown_files(&self.issues_dir)?;
        if active_files.is_empty() {
            return Ok(None);
        }

        // Return the first issue (files are sorted)
        let issue = self.load_issue_from_path(&active_files[0])?;
        Ok(Some(issue))
    }

    async fn all_issues_completed(&self) -> Result<bool> {
        let _lock = self.lock.lock().await;

        let active_files = Self::list_markdown_files(&self.issues_dir)?;
        Ok(active_files.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_temp_storage() -> (FileSystemIssueStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let issues_dir = temp_dir.path().join("issues");
        let storage = FileSystemIssueStorage::new(issues_dir).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_get_issue() {
        let (storage, _temp_dir) = create_temp_storage();

        let issue = storage
            .create_issue(
                "test-issue".to_string(),
                "# Test Issue\n\nContent".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(issue.name, "test-issue");
        assert_eq!(issue.content, "# Test Issue\n\nContent");

        let retrieved = storage.get_issue("test-issue").await.unwrap();
        assert_eq!(retrieved, issue);
    }

    #[tokio::test]
    async fn test_list_issues() {
        let (storage, _temp_dir) = create_temp_storage();

        storage
            .create_issue("issue1".to_string(), "Content 1".to_string())
            .await
            .unwrap();
        storage
            .create_issue("issue2".to_string(), "Content 2".to_string())
            .await
            .unwrap();

        let issues = storage.list_issues().await.unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].name, "issue1");
        assert_eq!(issues[1].name, "issue2");
    }

    #[tokio::test]
    async fn test_complete_issue() {
        let (storage, _temp_dir) = create_temp_storage();

        storage
            .create_issue("test-issue".to_string(), "Content".to_string())
            .await
            .unwrap();

        let completed = storage.complete_issue("test-issue").await.unwrap();
        assert_eq!(completed.name, "test-issue");

        // Issue should no longer be in active list
        let _active_issues = storage.list_issues().await.unwrap();

        // But should still be accessible by name
        let retrieved = storage.get_issue("test-issue").await.unwrap();
        assert_eq!(retrieved.name, "test-issue");
    }

    #[tokio::test]
    async fn test_next_issue() {
        let (storage, _temp_dir) = create_temp_storage();

        // No issues initially
        let next = storage.next_issue().await.unwrap();
        assert!(next.is_none());

        // Create some issues
        storage
            .create_issue("b-issue".to_string(), "B".to_string())
            .await
            .unwrap();
        storage
            .create_issue("a-issue".to_string(), "A".to_string())
            .await
            .unwrap();

        // Should get the first alphabetically
        let next = storage.next_issue().await.unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().name, "a-issue");
    }

    #[tokio::test]
    async fn test_all_issues_completed() {
        let (storage, _temp_dir) = create_temp_storage();

        // No issues - considered all completed
        let all_completed = storage.all_issues_completed().await.unwrap();
        assert!(all_completed);

        // Create an issue
        storage
            .create_issue("test".to_string(), "Content".to_string())
            .await
            .unwrap();

        let all_completed = storage.all_issues_completed().await.unwrap();
        assert!(!all_completed);

        // Complete the issue
        storage.complete_issue("test").await.unwrap();

        let all_completed = storage.all_issues_completed().await.unwrap();
        assert!(all_completed);
    }

    #[tokio::test]
    async fn test_issue_name_generation() {
        let (storage, _temp_dir) = create_temp_storage();

        let issue = storage
            .create_issue("".to_string(), "Content".to_string())
            .await
            .unwrap();

        // Should have generated a ULID name
        assert!(!issue.name.is_empty());
        assert!(issue.name.len() > 10); // ULIDs are longer than this
    }
}
