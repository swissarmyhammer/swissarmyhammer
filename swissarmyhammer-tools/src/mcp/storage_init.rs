//! Storage initialization module for MCP server
//!
//! This module handles storage instantiation separately from the MCP server,
//! following the architecture principle that the MCP server should not directly
//! instantiate storage implementations.

use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_common::{Result, SwissArmyHammerError};
use swissarmyhammer_issues::{FileSystemIssueStorage, IssueStorage};
use swissarmyhammer_memoranda::{MarkdownMemoStorage, MemoStorage};
use tokio::sync::RwLock;

/// Initialize issue storage with the given working directory context
///
/// # Arguments
///
/// * `work_dir` - The working directory for issue storage
///
/// # Returns
///
/// * `Result<Arc<RwLock<Box<dyn IssueStorage>>>>` - Initialized issue storage or error
pub fn initialize_issue_storage(
    work_dir: &std::path::Path,
) -> Result<Arc<RwLock<Box<dyn IssueStorage>>>> {
    // Execute storage initialization in the context of the work_dir
    let original_dir = std::env::current_dir().ok();
    let needs_dir_change = original_dir.as_ref().map_or(true, |dir| work_dir != *dir);

    // Set working directory context if different from current
    if needs_dir_change {
        std::env::set_current_dir(work_dir).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to set working directory: {e}"),
        })?;
    }

    // Initialize issue storage
    let storage = FileSystemIssueStorage::new_default()
        .map(|storage| Box::new(storage) as Box<dyn IssueStorage>)
        .map_err(|e| {
            tracing::error!("Failed to create issue storage: {}", e);
            SwissArmyHammerError::Other {
                message: format!("Failed to create issue storage: {e}"),
            }
        })?;

    // Restore original working directory if we changed it
    if needs_dir_change {
        if let Some(ref original_dir) = original_dir {
            if let Err(e) = std::env::set_current_dir(original_dir) {
                tracing::warn!("Failed to restore original working directory: {}", e);
            }
        }
    }

    Ok(Arc::new(RwLock::new(storage)))
}

/// Initialize memo storage with environment variable support and fallbacks
///
/// # Returns
///
/// * `Result<Arc<RwLock<Box<dyn MemoStorage>>>>` - Initialized memo storage or error
pub async fn initialize_memo_storage() -> Result<Arc<RwLock<Box<dyn MemoStorage>>>> {
    // First check if SWISSARMYHAMMER_MEMOS_DIR environment variable is set
    if let Ok(custom_path) = std::env::var("SWISSARMYHAMMER_MEMOS_DIR") {
        let custom_dir = PathBuf::from(custom_path);
        // Try to create directory, but don't fail if it already exists or can't be created
        if let Err(e) = std::fs::create_dir_all(&custom_dir) {
            tracing::warn!(
                "Failed to create custom memos directory {}: {}",
                custom_dir.display(),
                e
            );
        }
        Ok(Arc::new(RwLock::new(
            Box::new(MarkdownMemoStorage::new(custom_dir)) as Box<dyn MemoStorage>,
        )))
    } else {
        match MarkdownMemoStorage::new_default().await {
            Ok(storage) => Ok(Arc::new(RwLock::new(
                Box::new(storage) as Box<dyn MemoStorage>
            ))),
            Err(e) => {
                tracing::warn!(
                    "Cannot create memo storage in Git repository ({}), using temporary directory for testing",
                    e
                );
                // Fallback to temporary directory for tests
                let temp_dir = std::env::temp_dir().join("swissarmyhammer-mcp-test");
                std::fs::create_dir_all(&temp_dir).map_err(|err| SwissArmyHammerError::Other {
                    message: format!("Failed to create temporary memo directory: {err}"),
                })?;
                Ok(Arc::new(RwLock::new(
                    Box::new(MarkdownMemoStorage::new(temp_dir)) as Box<dyn MemoStorage>,
                )))
            }
        }
    }
}
