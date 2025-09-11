//! CLI Context
//!
//! Shared context object that holds all storage instances and configuration
//! to avoid recreating them in each command.

use std::{sync::Arc, rc::Rc};
use swissarmyhammer_common::Result;
use swissarmyhammer_git::GitOperations;

use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_workflow::{FileSystemWorkflowRunStorage, WorkflowStorage};
use crate::cli::OutputFormat;


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
    pub git_operations: Option<Rc<GitOperations>>,

    /// Template context with configuration
    pub template_context: swissarmyhammer_config::TemplateContext,

    /// Global output format setting
    pub format: OutputFormat,

    /// Original global output format option (None if not explicitly specified)
    pub format_option: Option<OutputFormat>,

    /// Enable verbose output
    pub verbose: bool,

    /// Enable debug output
    pub debug: bool,

    /// Suppress output except errors
    pub quiet: bool,

    /// Parsed CLI arguments
    pub matches: clap::ArgMatches,
}

impl CliContext {
    /// Create a new CLI context with default storage implementations
    pub async fn new(
        template_context: swissarmyhammer_config::TemplateContext,
        format: OutputFormat,
        format_option: Option<OutputFormat>,
        verbose: bool,
        debug: bool,
        quiet: bool,
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
                })??
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
            swissarmyhammer_memoranda::MarkdownMemoStorage::new_default().await
                .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: format!("Failed to create memo storage: {e}"),
                })?
        );

        let issue_storage = Arc::new(
            swissarmyhammer_issues::FileSystemIssueStorage::new_default()?
        );

        // Initialize git operations - make it optional when not in a git repository
        let git_operations = match GitOperations::new() {
            Ok(ops) => {
                tracing::debug!("Git operations initialized successfully");
                Some(Rc::new(ops))
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
            format,
            format_option,
            verbose,
            debug,
            quiet,
            matches,
        })
    }

    /// Display items using the configured output format
    pub fn display<T>(&self, items: Vec<T>) -> Result<()> 
    where 
        T: serde::Serialize,
    {
        match self.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&items)
                    .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                        message: format!("Failed to serialize to JSON: {e}"),
                    })?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(&items)
                    .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                        message: format!("Failed to serialize to YAML: {e}"),
                    })?;
                println!("{}", yaml);
            }
            OutputFormat::Table => {
                // Simple table fallback - just print as JSON for now
                let json = serde_json::to_string_pretty(&items)
                    .map_err(|e| swissarmyhammer_common::SwissArmyHammerError::Other {
                        message: format!("Failed to serialize to JSON: {e}"),
                    })?;
                println!("{}", json);
            }
        }
        Ok(())
    }
}
