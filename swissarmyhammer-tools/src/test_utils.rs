//! Test utilities for MCP tools

use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::ToolContext;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use swissarmyhammer::common::rate_limiter::{RateLimiter, RateLimiterConfig};
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use tempfile::TempDir;
use tokio::sync::{Mutex as TokioMutex, RwLock};

/// Creates a test rate limiter with generous limits suitable for testing
///
/// This replaces MockRateLimiter usage in tests with a real RateLimiter
/// configured with very high limits to prevent rate limiting during tests
/// while still testing the actual rate limiting implementation.
pub fn create_test_rate_limiter() -> Arc<RateLimiter> {
    Arc::new(RateLimiter::with_config(RateLimiterConfig {
        global_limit: 10000,                     // Very high global limit
        per_client_limit: 1000,                  // High per-client limit
        expensive_operation_limit: 500,          // High expensive operation limit
        window_duration: Duration::from_secs(1), // Short refill window for tests
    }))
}

/// Creates a test context with mock storage backends for testing MCP tools
///
/// This function creates a ToolContext similar to the one in swissarmyhammer,
/// but available for testing MCP tools in swissarmyhammer-tools.
/// Each call creates a unique test directory to prevent conflicts between parallel tests.
#[cfg(test)]
pub async fn create_test_context() -> ToolContext {
    // Use system temp directory to avoid path issues
    let test_issues_dir = std::env::temp_dir()
        .join("sah_test_issues")
        .join(format!("{}", std::process::id()));
    let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
        FileSystemIssueStorage::new(test_issues_dir).unwrap(),
    )));
    let git_ops: Arc<TokioMutex<Option<GitOperations>>> = Arc::new(TokioMutex::new(None));
    // Create temporary directory for memo storage in tests
    let memo_temp_dir = std::env::temp_dir()
        .join("sah_test_memos")
        .join(format!("{}", std::process::id()));

    let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> = Arc::new(RwLock::new(Box::new(
        MarkdownMemoStorage::new(memo_temp_dir),
    )));

    let rate_limiter = create_test_rate_limiter();

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(
        tool_handlers,
        issue_storage,
        git_ops,
        memo_storage,
        rate_limiter,
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
