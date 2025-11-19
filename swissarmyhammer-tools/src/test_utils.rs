//! Test utilities for MCP tools

use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::ToolContext;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swissarmyhammer_config::agent::AgentConfig;
use swissarmyhammer_git::GitOperations;
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use tempfile::TempDir;
use tokio::sync::{Mutex as TokioMutex, RwLock};

#[cfg(test)]
use crate::mcp::progress_notifications::{ProgressNotification, ProgressSender};
#[cfg(test)]
use tokio::sync::mpsc;

/// Git-specific test helpers
pub mod git_test_helpers;

/// Creates a test context with mock storage backends for testing MCP tools
///
/// This function creates a ToolContext similar to the one in swissarmyhammer,
/// but available for testing MCP tools in swissarmyhammer-tools.
/// Each call creates a unique test directory to prevent conflicts between parallel tests.
#[cfg(test)]
pub async fn create_test_context() -> ToolContext {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique_id = format!(
        "{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );

    // Use system temp directory to avoid path issues
    let test_issues_dir = std::env::temp_dir()
        .join("sah_test_issues")
        .join(&unique_id);
    let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
        FileSystemIssueStorage::new(test_issues_dir).unwrap(),
    )));
    let git_ops: Arc<TokioMutex<Option<GitOperations>>> = Arc::new(TokioMutex::new(None));
    // Create temporary directory for memo storage in tests
    let memo_temp_dir = std::env::temp_dir().join("sah_test_memos").join(&unique_id);

    let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> = Arc::new(RwLock::new(Box::new(
        MarkdownMemoStorage::new(memo_temp_dir),
    )));

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
    let agent_config = Arc::new(AgentConfig::default());

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
        agent_config,
    )
}

/// Test environment specifically designed for issue-related testing
///
/// Provides convenient setup and access to issue directories following
/// the new `.swissarmyhammer/issues` structure.
#[cfg(test)]
pub struct TestIssueEnvironment {
    /// Temporary directory that will be automatically cleaned up
    pub temp_dir: TempDir,
    /// Path to the issues directory within the test environment
    pub issues_dir: PathBuf,
    /// Path to the completed issues directory within the test environment
    pub complete_dir: PathBuf,
}

#[cfg(test)]
impl Default for TestIssueEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl TestIssueEnvironment {
    /// Create a new test environment with proper .swissarmyhammer/issues structure
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let swissarmyhammer_dir = temp_dir.path().join(".swissarmyhammer");
        let issues_dir = swissarmyhammer_dir.join("issues");
        let complete_dir = issues_dir.join("complete");

        // Create directory structure
        std::fs::create_dir_all(&complete_dir).expect("Failed to create directory structure");

        Self {
            temp_dir,
            issues_dir,
            complete_dir,
        }
    }

    /// Create a FileSystemIssueStorage using this test environment
    pub fn storage(&self) -> FileSystemIssueStorage {
        FileSystemIssueStorage::new(self.issues_dir.clone()).unwrap()
    }

    /// Get the root path of the test environment
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Get the .swissarmyhammer directory path
    pub fn swissarmyhammer_dir(&self) -> PathBuf {
        self.temp_dir.path().join(".swissarmyhammer")
    }
}

/// Helper for testing tools that send progress notifications
///
/// This helper consolidates the common pattern of:
/// 1. Creating a progress notification channel
/// 2. Setting up a test context with progress sender
/// 3. Executing a tool
/// 4. Collecting all progress notifications
///
/// # Type Parameters
/// * `F` - A future that executes the tool and returns a Result
///
/// # Arguments
/// * `tool_executor` - An async closure that receives a ToolContext and executes the tool
///
/// # Returns
/// A tuple containing:
/// * The result returned by the tool executor
/// * A vector of all progress notifications that were sent during execution
///
/// # Example
/// ```ignore
/// let (result, notifications) = execute_with_progress_capture(|context| async move {
///     let tool = MyTool::new();
///     let mut arguments = serde_json::Map::new();
///     arguments.insert("key".to_string(), json!("value"));
///     tool.execute(arguments, &context).await
/// }).await;
///
/// assert!(result.is_ok());
/// assert_eq!(notifications.len(), 2);
/// assert_eq!(notifications[0].progress, Some(0));
/// assert_eq!(notifications[1].progress, Some(100));
/// ```
#[cfg(test)]
pub async fn execute_with_progress_capture<F, Fut, T>(
    tool_executor: F,
) -> (T, Vec<ProgressNotification>)
where
    F: FnOnce(ToolContext) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    // Create progress notification channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = ProgressSender::new(tx);

    // Set up test context with progress sender
    let mut context = create_test_context().await;
    context.progress_sender = Some(progress_sender);

    // Execute the tool
    let result = tool_executor(context).await;

    // Collect all notifications
    let mut notifications = Vec::new();
    while let Ok(notification) = rx.try_recv() {
        notifications.push(notification);
    }

    (result, notifications)
}
