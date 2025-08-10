//! Test utilities for MCP tools

use crate::mcp::tool_registry::ToolContext;
use crate::mcp::tool_handlers::ToolHandlers;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex as TokioMutex, RwLock};
use swissarmyhammer::git::GitOperations;
use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};

/// Creates a test context with mock storage backends for testing MCP tools
/// 
/// This function creates a ToolContext similar to the one in swissarmyhammer,
/// but available for testing MCP tools in swissarmyhammer-tools
#[cfg(test)]
pub async fn create_test_context() -> ToolContext {
    let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
        FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap(),
    )));
    let git_ops: Arc<TokioMutex<Option<GitOperations>>> = Arc::new(TokioMutex::new(None));
    let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> =
        Arc::new(RwLock::new(Box::new(MockMemoStorage::new())));

    let rate_limiter = Arc::new(swissarmyhammer::common::rate_limiter::MockRateLimiter);

    let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));

    ToolContext::new(tool_handlers, issue_storage, git_ops, memo_storage, rate_limiter)
}