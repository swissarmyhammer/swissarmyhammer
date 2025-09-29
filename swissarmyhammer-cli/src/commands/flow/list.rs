//! List available workflows command implementation

use super::display::{VerboseWorkflowInfo, WorkflowInfo};
use crate::cli::{OutputFormat, PromptSource, PromptSourceArg};
use crate::context::CliContext;
use swissarmyhammer::{Result, WorkflowResolver, WorkflowStorageBackend};
use swissarmyhammer_workflow::MemoryWorkflowStorage;

/// Execute the list workflows command
pub async fn execute_list_command(
    _format: OutputFormat,
    verbose: bool,
    source_filter: Option<PromptSourceArg>,
    context: &CliContext,
) -> Result<()> {
    // Load all workflows from all sources using resolver (same pattern as prompts)
    let mut storage = MemoryWorkflowStorage::new();
    let mut resolver = WorkflowResolver::new();
    resolver.load_all_workflows(&mut storage)?;

    // Get all workflows
    let all_workflows = storage.list_workflows()?;

    // Collect workflow information
    let mut workflow_infos = Vec::new();

    for workflow in all_workflows {
        // Get the source from the resolver
        let workflow_source = match resolver.workflow_sources.get(&workflow.name) {
            Some(swissarmyhammer::FileSource::Builtin) => PromptSource::Builtin,
            Some(swissarmyhammer::FileSource::User) => PromptSource::User,
            Some(swissarmyhammer::FileSource::Local) => PromptSource::Local,
            Some(swissarmyhammer::FileSource::Dynamic) => PromptSource::Dynamic,
            None => PromptSource::Dynamic,
        };

        // Apply source filter
        if let Some(ref filter) = source_filter {
            let filter_source: PromptSource = filter.clone().into();
            if filter_source != workflow_source && filter_source != PromptSource::Dynamic {
                continue;
            }
        }

        workflow_infos.push((workflow, workflow_source));
    }

    // Sort by name for consistent output
    workflow_infos.sort_by(|a, b| a.0.name.as_str().cmp(b.0.name.as_str()));

    // Convert to display objects based on verbose flag using emoji-based sources
    if verbose {
        let verbose_workflows: Vec<VerboseWorkflowInfo> = workflow_infos
            .iter()
            .map(|(workflow, _)| {
                let file_source = resolver.workflow_sources.get(&workflow.name);
                VerboseWorkflowInfo::from_workflow_with_source(workflow, file_source)
            })
            .collect();
        context.display(verbose_workflows)?;
    } else {
        let workflow_info: Vec<WorkflowInfo> = workflow_infos
            .iter()
            .map(|(workflow, _)| {
                let file_source = resolver.workflow_sources.get(&workflow.name);
                WorkflowInfo::from_workflow_with_source(workflow, file_source)
            })
            .collect();
        context.display(workflow_info)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{OutputFormat, PromptSourceArg};
    use swissarmyhammer_config::TemplateContext;

    async fn create_test_context() -> Result<CliContext> {
        let template_context = TemplateContext::new();
        let matches = clap::ArgMatches::default();
        CliContext::new(
            template_context,
            OutputFormat::Table,
            None,
            false,
            false,
            false,
            matches,
        )
        .await
    }

    #[tokio::test]
    async fn test_execute_list_command_basic() -> Result<()> {
        let context = create_test_context().await?;

        // This should succeed without error, even if no workflows are found
        let result = execute_list_command(OutputFormat::Table, false, None, &context).await;

        assert!(result.is_ok(), "Basic list command should succeed");
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_list_command_verbose() -> Result<()> {
        let context = create_test_context().await?;

        let result = execute_list_command(OutputFormat::Table, true, None, &context).await;

        assert!(result.is_ok(), "Verbose list command should succeed");
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_list_command_with_filter() -> Result<()> {
        let context = create_test_context().await?;

        let result = execute_list_command(
            OutputFormat::Table,
            false,
            Some(PromptSourceArg::Builtin),
            &context,
        )
        .await;

        assert!(result.is_ok(), "Filtered list command should succeed");
        Ok(())
    }
}
