//! CLI Context
//!
//! Shared context object that holds all storage instances and configuration
//! to avoid recreating them in each command.

use std::sync::Arc;
use swissarmyhammer_common::Result;
use swissarmyhammer_git::GitOperations;

use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_workflow::{FileSystemWorkflowRunStorage, WorkflowStorage};

/// Shared CLI context containing all storage objects, configuration, and parsed arguments
pub struct CliContext {
    /// Workflow storage for loading and managing workflows
    pub workflow_storage: Arc<WorkflowStorage>,

    /// Workflow run storage for execution state
    #[allow(dead_code)]
    pub workflow_run_storage: Arc<FileSystemWorkflowRunStorage>,

    /// Prompt library for managing prompts
    #[allow(dead_code)]
    pub prompt_library: Arc<PromptLibrary>,

    /// Memo storage for memoranda
    #[allow(dead_code)]
    pub memo_storage: Arc<swissarmyhammer_memoranda::MarkdownMemoStorage>,

    /// Issue storage for issue tracking
    #[allow(dead_code)]
    pub issue_storage: Arc<swissarmyhammer_issues::FileSystemIssueStorage>,

    /// Git operations (optional - None if not in a git repository)
    #[allow(dead_code)]
    pub git_operations: Option<GitOperations>,

    /// Template context with configuration
    pub template_context: swissarmyhammer_config::TemplateContext,

    /// Parsed CLI arguments
    pub matches: clap::ArgMatches,
}

impl CliContext {
    /// Create a new CLI context with default storage implementations
    pub async fn new(
        template_context: swissarmyhammer_config::TemplateContext,
        matches: clap::ArgMatches,
    ) -> Result<Self> {
        let workflow_storage = Arc::new(
            tokio::task::spawn_blocking(WorkflowStorage::file_system)
                .await
                .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: format!("Failed to create workflow storage: {e}"),
                })??,
        );

        let workflow_run_storage = Arc::new(
            tokio::task::spawn_blocking(|| {
                let base_path = swissarmyhammer_common::utils::paths::get_swissarmyhammer_dir()
                    .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
                FileSystemWorkflowRunStorage::new(base_path)
            })
            .await
            .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Failed to create workflow run storage: {e}"),
            })??,
        );

        let mut prompt_library = PromptLibrary::new();

        // Add default prompt sources
        if let Ok(home_dir) = swissarmyhammer_common::utils::paths::get_swissarmyhammer_dir() {
            let prompts_dir = home_dir.join("prompts");
            if prompts_dir.exists() {
                if let Err(e) = prompt_library.add_directory(&prompts_dir) {
                    eprintln!(
                        "Warning: Failed to load prompts from {:?}: {}",
                        prompts_dir, e
                    );
                }
            }
        }

        let memo_storage = Arc::new(
            swissarmyhammer_memoranda::MarkdownMemoStorage::new_default()
                .await
                .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: format!("Failed to create memo storage: {e}"),
                })?,
        );

        let issue_storage =
            Arc::new(swissarmyhammer_issues::FileSystemIssueStorage::new_default()?);

        // Initialize git operations - make it optional when not in a git repository
        let git_operations = match GitOperations::new() {
            Ok(ops) => {
                tracing::debug!("Git operations initialized successfully");
                Some(ops)
            }
            Err(e) => {
                tracing::warn!("Git operations not available: {}", e);
                None
            }
        };
        Ok(Self {
            workflow_storage,
            workflow_run_storage,
            prompt_library: Arc::new(prompt_library),
            memo_storage,
            issue_storage,
            git_operations,
            template_context,
            matches,
        })
    }
}
