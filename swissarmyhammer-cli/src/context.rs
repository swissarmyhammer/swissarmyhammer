//! CLI Context
//!
//! Shared context object that holds all storage instances and configuration
//! to avoid recreating them in each command.

use std::{rc::Rc, sync::Arc};
use swissarmyhammer_common::Result;
use swissarmyhammer_git::GitOperations;

use crate::cli::OutputFormat;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_workflow::{FileSystemWorkflowRunStorage, WorkflowStorage};

/// Shared CLI context containing all storage objects, configuration, and parsed arguments
#[derive(derive_builder::Builder)]
#[builder(pattern = "owned")]
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
    #[builder(setter(into))]
    pub template_context: swissarmyhammer_config::TemplateContext,

    /// Global output format setting
    #[builder(default = "OutputFormat::Table")]
    pub format: OutputFormat,

    /// Original global output format option (None if not explicitly specified)
    #[builder(default)]
    pub format_option: Option<OutputFormat>,

    /// Enable verbose output
    #[builder(default)]
    pub verbose: bool,

    /// Enable debug output
    #[builder(default)]
    pub debug: bool,

    /// Suppress output except errors
    #[builder(default)]
    pub quiet: bool,

    /// Parsed CLI arguments
    #[builder(setter(into))]
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
        CliContextBuilder::default()
            .template_context(template_context)
            .format(format)
            .format_option(format_option)
            .verbose(verbose)
            .debug(debug)
            .quiet(quiet)
            .matches(matches)
            .build_async()
            .await
    }

    /// Get the prompt library - returns a new library with all prompts loaded
    /// This reloads prompts to ensure we have the latest version
    pub fn get_prompt_library(&self) -> Result<swissarmyhammer_prompts::PromptLibrary> {
        let mut library = swissarmyhammer_prompts::PromptLibrary::new();
        let mut resolver = swissarmyhammer::PromptResolver::new();

        resolver.load_all_prompts(&mut library).map_err(|e| {
            swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Failed to load prompts: {e}"),
            }
        })?;

        Ok(library)
    }

    /// Render a prompt with parameters, merging with template context
    pub fn render_prompt(
        &self,
        prompt_name: &str,
        parameters: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let library = self.get_prompt_library()?;

        // Create a template context with CLI arguments having highest precedence
        let mut final_context = self.template_context.clone();
        for (key, value) in parameters {
            final_context.set(key.clone(), value.clone());
        }

        // Render the prompt with the merged context
        library.render(prompt_name, &final_context).map_err(|e| {
            swissarmyhammer_common::SwissArmyHammerError::Other {
                message: format!("Failed to render prompt '{}': {}", prompt_name, e),
            }
        })
    }

    /// Display items using the configured output format
    pub fn display<T>(&self, items: Vec<T>) -> Result<()>
    where
        T: serde::Serialize + tabled::Tabled,
    {
        // Use explicit format option if provided, otherwise use default format
        let format = self.format_option.unwrap_or(self.format);
        match format {
            OutputFormat::Table => {
                if items.is_empty() {
                    println!("No items to display");
                } else {
                    println!(
                        "{}",
                        tabled::Table::new(&items).with(tabled::settings::Style::modern())
                    );
                }
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&items).map_err(|e| {
                    swissarmyhammer_common::SwissArmyHammerError::Other {
                        message: format!("Failed to serialize to JSON: {e}"),
                    }
                })?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(&items).map_err(|e| {
                    swissarmyhammer_common::SwissArmyHammerError::Other {
                        message: format!("Failed to serialize to YAML: {e}"),
                    }
                })?;
                println!("{}", yaml);
            }
        }
        Ok(())
    }

    /// Display different types based on verbose flag using display rows enum
    pub fn display_prompts(
        &self,
        rows: crate::commands::prompt::display::DisplayRows,
    ) -> Result<()> {
        use crate::commands::prompt::display::DisplayRows;

        match rows {
            DisplayRows::Standard(items) => self.display(items),
            DisplayRows::Verbose(items) => self.display(items),
        }
    }

    /// Display rules using display rows enum
    pub fn display_rules(&self, rows: crate::commands::rule::display::DisplayRows) -> Result<()> {
        use crate::commands::rule::display::DisplayRows;

        match rows {
            DisplayRows::Standard(items) => self.display(items),
            DisplayRows::Verbose(items) => self.display(items),
        }
    }
}

impl CliContextBuilder {
    /// Build the CliContext with async initialization of storage components
    pub async fn build_async(self) -> Result<CliContext> {
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
                Some(Rc::new(ops))
            }
            Err(e) => {
                tracing::warn!("Git operations not available: {}", e);
                None
            }
        };

        Ok(CliContext {
            workflow_storage,
            workflow_run_storage,
            prompt_library: Arc::new(prompt_library),
            memo_storage,
            issue_storage,
            git_operations,
            template_context: self.template_context.ok_or_else(|| {
                swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: "template_context is required".to_string(),
                }
            })?,
            format: self.format.unwrap_or(OutputFormat::Table),
            format_option: self.format_option.unwrap_or_default(),
            verbose: self.verbose.unwrap_or_default(),
            debug: self.debug.unwrap_or_default(),
            quiet: self.quiet.unwrap_or_default(),
            matches: self.matches.ok_or_else(|| {
                swissarmyhammer_common::SwissArmyHammerError::Other {
                    message: "matches is required".to_string(),
                }
            })?,
        })
    }
}
