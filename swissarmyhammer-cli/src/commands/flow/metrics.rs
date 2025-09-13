//! View metrics for workflow runs command implementation

use crate::cli::OutputFormat;
use crate::context::CliContext;
use super::shared::parse_workflow_run_id;
use swissarmyhammer::{Result, WorkflowExecutor, WorkflowStorage};

/// Execute the metrics workflow command
pub async fn execute_metrics_command(
    run_id: Option<String>,
    workflow: Option<String>,
    _format: OutputFormat,
    global: bool,
    context: &CliContext,
) -> Result<()> {
    let _storage = WorkflowStorage::file_system()?;
    let executor = WorkflowExecutor::new();
    let metrics = executor.get_metrics();

    if global {
        // Show global metrics summary
        let global_metrics = metrics.get_global_metrics();
        match context.format {
            crate::cli::OutputFormat::Table => {
                println!("📊 Global Workflow Metrics");
                println!("========================");
                println!("Total runs: {}", global_metrics.total_runs);
                println!("Success rate: {:.2}%", global_metrics.success_rate * 100.0);
                println!(
                    "Average execution time: {:.2}s",
                    global_metrics.average_execution_time.as_secs_f64()
                );
                println!(
                    "Total execution time: {:.2}s",
                    global_metrics.total_execution_time.as_secs_f64()
                );
                println!("Active workflows: {}", global_metrics.active_workflows);
                println!("Unique workflows: {}", global_metrics.unique_workflows);
            }
            crate::cli::OutputFormat::Json => {
                let json_output = serde_json::to_string_pretty(&global_metrics)?;
                println!("{json_output}");
            }
            crate::cli::OutputFormat::Yaml => {
                let yaml_output = serde_yaml::to_string(&global_metrics)?;
                println!("{yaml_output}");
            }
        }
    } else if let Some(run_id_str) = run_id {
        // Show metrics for specific run
        let run_id_typed = parse_workflow_run_id(&run_id_str)?;

        if let Some(run_metrics) = metrics.get_run_metrics(&run_id_typed) {
            match context.format {
                crate::cli::OutputFormat::Table => {
                    println!("📊 Run Metrics: {run_id_str}");
                    println!("Workflow: {}", run_metrics.workflow_name);
                    println!("Status: {:?}", run_metrics.status);
                    println!("Status: {:?}", run_metrics.status);
                    println!("Started: {}", run_metrics.started_at.format("%Y-%m-%d %H:%M:%S UTC"));
                    if let Some(completed) = run_metrics.completed_at {
                        println!("Completed: {}", completed.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                    println!(
                        "Started: {}",
                        run_metrics.started_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                }
                crate::cli::OutputFormat::Json => {
                    let json_output = serde_json::to_string_pretty(&run_metrics)?;
                    println!("{json_output}");
                }
                crate::cli::OutputFormat::Yaml => {
                    let yaml_output = serde_yaml::to_string(&run_metrics)?;
                    println!("{yaml_output}");
                }
            }
        } else {
            println!("❌ No metrics found for run ID: {run_id_str}");
        }
    } else if let Some(workflow_name) = workflow {
        // Show metrics filtered by workflow name
        // Show all runs for this workflow
        println!("📊 Workflow Metrics: {workflow_name}");
        println!("Note: Individual run metrics available with --run-id");
        // For now, just show the workflow name since detailed workflow metrics API is not available
    } else {
        println!("❌ Please specify either --global, --run-id <ID>, or --workflow <NAME>");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;
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
    async fn test_execute_metrics_command_global() -> Result<()> {
        let context = create_test_context().await?;
        
        let result = execute_metrics_command(
            None,
            None,
            OutputFormat::Table,
            true,
            &context,
        ).await;

        assert!(result.is_ok(), "Global metrics command should succeed");
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_metrics_command_no_params() -> Result<()> {
        let context = create_test_context().await?;
        
        let result = execute_metrics_command(
            None,
            None,
            OutputFormat::Table,
            false,
            &context,
        ).await;

        // Should succeed but show error message about missing parameters
        assert!(result.is_ok(), "Metrics command with no params should succeed");
        Ok(())
    }
}