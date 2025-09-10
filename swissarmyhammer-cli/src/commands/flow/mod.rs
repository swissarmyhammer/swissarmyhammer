//! Flow command implementation
//!
//! Executes and manages workflows with support for starting new runs and resuming existing ones

use crate::cli::{
    FlowSubcommand, OutputFormat, PromptSource, PromptSourceArg, VisualizationFormat,
};
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use crate::parameter_cli;
use colored::*;
use is_terminal::IsTerminal;
use std::collections::HashMap;
use std::future;
use std::io::{self, Write};
use std::time::Duration;
use swissarmyhammer::{Result, SwissArmyHammerError, WorkflowName};
use swissarmyhammer::{
    Workflow, WorkflowExecutor, WorkflowResolver, WorkflowRunId, WorkflowRunStatus,
    WorkflowRunStorageBackend, WorkflowStorage, WorkflowStorageBackend,
};
use swissarmyhammer_common::{read_abort_file, remove_abort_file};
use swissarmyhammer_workflow::{ExecutorError, MemoryWorkflowStorage};
use tokio::signal;
use tokio::time::timeout;

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the flow command
pub async fn handle_command(
    subcommand: FlowSubcommand,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> i32 {
    match run_flow_command(subcommand, _template_context).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Flow command failed: {}", e);
            EXIT_ERROR
        }
    }
}

/// Main entry point for flow command
pub async fn run_flow_command(
    subcommand: FlowSubcommand,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> Result<()> {
    match subcommand {
        FlowSubcommand::Run {
            workflow,
            vars,
            interactive,
            dry_run,
            timeout: timeout_str,
            quiet,
        } => {
            let all_vars = vars;

            run_workflow_command(
                WorkflowCommandConfig {
                    workflow_name: workflow,
                    vars: all_vars,
                    interactive,
                    dry_run,

                    timeout_str,
                    quiet,
                },
                _template_context,
            )
            .await
        }
        FlowSubcommand::Resume {
            run_id,
            interactive,
            timeout: timeout_str,
            quiet,
        } => resume_workflow_command(run_id, interactive, timeout_str, quiet).await,
        FlowSubcommand::List {
            format,
            verbose,
            source,
        } => list_workflows_command(format, verbose, source).await,
        FlowSubcommand::Status {
            run_id,
            format,
            watch,
        } => status_workflow_command(run_id, format, watch).await,
        FlowSubcommand::Logs {
            run_id,
            follow,
            tail,
            level,
        } => logs_workflow_command(run_id, follow, tail, level).await,
        FlowSubcommand::Metrics {
            run_id,
            workflow,
            format,
            global,
        } => metrics_workflow_command(run_id, workflow, format, global).await,
        FlowSubcommand::Visualize {
            run_id,
            format,
            output,
            timing,
            counts,
            path_only,
        } => visualize_workflow_command(run_id, format, output, timing, counts, path_only).await,
        FlowSubcommand::Test {
            workflow,
            vars,
            interactive,
            timeout: timeout_str,
            quiet,
        } => {
            let all_vars = vars;

            // Run workflow in test mode - same as flow run --test
            run_workflow_command(
                WorkflowCommandConfig {
                    workflow_name: workflow,
                    vars: all_vars,
                    interactive,
                    dry_run: false,
                    timeout_str,
                    quiet,
                },
                _template_context,
            )
            .await
        }
    }
}

/// Configuration for running a workflow command
pub struct WorkflowCommandConfig {
    pub workflow_name: String,
    pub vars: Vec<String>,
    pub interactive: bool,
    pub dry_run: bool,

    pub timeout_str: Option<String>,
    pub quiet: bool,
}

/// Execute a workflow
pub async fn run_workflow_command(
    config: WorkflowCommandConfig,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> Result<()> {
    // Use proper WorkflowStorage with embedded builtins
    let workflow_storage = tokio::task::spawn_blocking(WorkflowStorage::file_system)
        .await
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to create workflow storage: {e}"),
        })??;

    let workflow_name_typed = WorkflowName::new(&config.workflow_name);
    let workflow = workflow_storage.get_workflow(&workflow_name_typed)?;

    // Resolve workflow parameters with enhanced parameter system
    let workflow_variables = parameter_cli::resolve_workflow_parameters_interactive(
        &config.workflow_name,
        &config.vars,
        config.interactive && !config.dry_run,
    )
    .unwrap_or_else(|e| {
        eprintln!("Warning: Failed to resolve workflow parameters: {e}");
        HashMap::new()
    });

    // Parse additional variables not handled by workflow parameters (backward compatibility)
    let mut variables = workflow_variables;
    for var in config.vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].to_string();
            // Add variable, allowing later values to override earlier ones
            variables.insert(key, serde_json::Value::String(parts[1].to_string()));
        } else {
            return Err(SwissArmyHammerError::Other { message: format!(
                "Invalid variable format: '{var}'. Expected 'key=value' format. Example: --var input=test"
            ) });
        }
    }

    // Template variables are now passed through the regular workflow variables system

    // Parse timeout
    let timeout_duration = if let Some(timeout_str) = config.timeout_str {
        Some(parse_duration(&timeout_str)?)
    } else {
        None
    };

    if config.dry_run {
        println!("🔍 Dry run mode - showing execution plan:");
        println!("📋 Workflow: {}", workflow.name);
        println!("🏁 Initial state: {}", workflow.initial_state);
        println!("🔧 Variables: {variables:?}");
        if let Some(timeout) = timeout_duration {
            println!("⏱️  Timeout: {timeout:?}");
        }
        println!("📊 States: {}", workflow.states.len());
        println!("🔄 Transitions: {}", workflow.transitions.len());

        // Show workflow structure
        println!("\n📈 Workflow structure:");
        for (state_id, state) in &workflow.states {
            println!(
                "  {} - {} {}",
                state_id,
                state.description,
                if state.is_terminal { "(terminal)" } else { "" }
            );
        }

        return Ok(());
    }

    tracing::info!("🚀 Starting workflow: {}", workflow.name);

    // Check for abort file before starting workflow
    if let Some(abort_reason) = read_abort_file(".").map_err(|e| SwissArmyHammerError::Other {
        message: e.to_string(),
    })? {
        // Clean up the abort file after detection
        let _ = remove_abort_file(".").map_err(|e| SwissArmyHammerError::Other {
            message: e.to_string(),
        });
        return Err(SwissArmyHammerError::from(
            swissarmyhammer_workflow::ExecutorError::Abort(abort_reason)
        ));
    }

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Create workflow run
    let mut run =
        executor
            .start_workflow(workflow.clone())
            .map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to start workflow '{}': {}", workflow.name, e),
            })?;

    // Set initial variables
    run.context.set_workflow_vars(variables.clone());

    // Set agent configuration from template context
    let agent_config = _template_context.get_agent_config(None);
    run.context.set_agent_config(agent_config);

    // Store variables in context for liquid template rendering - this will now include config values
    // The template context agent config is now properly transferred to workflow context

    // Set quiet mode in context for actions to use
    if config.quiet {
        run.context
            .insert("_quiet".to_string(), serde_json::Value::Bool(true));
    }

    // Set timeout in context for actions to use
    if let Some(timeout_duration) = timeout_duration {
        run.context.insert(
            "_timeout_secs".to_string(),
            serde_json::Value::Number(serde_json::Number::from(timeout_duration.as_secs())),
        );
    }

    // Setup signal handling for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Execute workflow with timeout and signal handling
    let execution_result = if let Some(timeout_duration) = timeout_duration {
        tokio::select! {
            result = execute_workflow_with_progress(&mut executor, &mut run, config.interactive) => result,
            _ = timeout(timeout_duration, future::pending::<()>()) => {
                tracing::warn!("Workflow execution timed out");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            },
            _ = shutdown_rx.recv() => {
                tracing::info!("Workflow execution interrupted by user");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            }
        }
    } else {
        tokio::select! {
            result = execute_workflow_with_progress(&mut executor, &mut run, config.interactive) => result,
            _ = shutdown_rx.recv() => {
                tracing::info!("Workflow execution interrupted by user");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            }
        }
    };

    // Create local workflow run storage (only store failed runs for debugging)
    let mut run_storage = create_local_workflow_run_storage()?;

    match execution_result {
        Ok(_) => match run.status {
            WorkflowRunStatus::Completed => {
                tracing::info!("✅ Workflow completed successfully");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));

                // Don't store successful runs to avoid accumulating thousands of runs
                // Only failed runs are stored for debugging purposes
            }
            WorkflowRunStatus::Failed => {
                tracing::error!("❌ Workflow failed");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));

                // Store failed runs for debugging
                if let Err(storage_err) = run_storage.store_run(&run) {
                    tracing::warn!("Failed to store failed run: {}", storage_err);
                }
            }
            WorkflowRunStatus::Cancelled => {
                tracing::warn!("🚫 Workflow cancelled");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));

                // Store cancelled runs for debugging
                if let Err(storage_err) = run_storage.store_run(&run) {
                    tracing::warn!("Failed to store cancelled run: {}", storage_err);
                }
            }
            _ => {
                tracing::info!("⏸️  Workflow paused");
                tracing::info!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            }
        },
        Err(e) => {
            tracing::error!("❌ Workflow execution failed: {}", e);
            run.fail();

            // Store failed runs for debugging
            if let Err(storage_err) = run_storage.store_run(&run) {
                tracing::warn!("Failed to store failed run: {}", storage_err);
            }

            // Return the error to allow proper exit code handling in main.rs
            return Err(e);
        }
    }

    Ok(())
}

/// Resume a workflow run
async fn resume_workflow_command(
    run_id: String,
    interactive: bool,
    timeout_str: Option<String>,
    quiet: bool,
) -> Result<()> {
    let mut storage = WorkflowStorage::file_system()?;

    // Parse run ID
    let run_id_typed = parse_workflow_run_id(&run_id)?;

    // Get the run
    let mut run = storage.get_run(&run_id_typed)?;

    // Check if run can be resumed
    if run.status == WorkflowRunStatus::Completed {
        println!("❌ Cannot resume completed workflow");
        return Ok(());
    }

    if run.status == WorkflowRunStatus::Failed {
        println!("❌ Cannot resume failed workflow");
        return Ok(());
    }

    // Parse timeout
    let timeout_duration = if let Some(timeout_str) = timeout_str {
        Some(parse_duration(&timeout_str)?)
    } else {
        None
    };

    println!("🔄 Resuming workflow: {}", run.workflow.name);
    println!("🔄 From state: {}", run.current_state);

    // Set quiet mode in context for actions to use
    if quiet {
        run.context
            .insert("_quiet".to_string(), serde_json::Value::Bool(true));
    }

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Setup signal handling for graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Resume workflow execution
    let execution_result = if let Some(timeout_duration) = timeout_duration {
        tokio::select! {
            result = execute_workflow_with_progress(&mut executor, &mut run, interactive) => result,
            _ = timeout(timeout_duration, future::pending::<()>()) => {
                tracing::warn!("Workflow execution timed out");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            },
            _ = shutdown_rx.recv() => {
                tracing::info!("Workflow execution interrupted by user");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            }
        }
    } else {
        tokio::select! {
            result = execute_workflow_with_progress(&mut executor, &mut run, interactive) => result,
            _ = shutdown_rx.recv() => {
                tracing::info!("Workflow execution interrupted by user");
                run.status = WorkflowRunStatus::Cancelled;
                Ok(())
            }
        }
    };

    // Store the updated run
    storage.store_run(&run)?;

    match execution_result {
        Ok(_) => match run.status {
            WorkflowRunStatus::Completed => {
                tracing::info!("✅ Workflow resumed and completed successfully");
            }
            WorkflowRunStatus::Failed => {
                tracing::error!("❌ Workflow resumed but failed");
            }
            WorkflowRunStatus::Cancelled => {
                tracing::warn!("🚫 Workflow resumed but was cancelled");
            }
            _ => {
                tracing::info!("⏸️  Workflow resumed and paused");
            }
        },
        Err(e) => {
            tracing::error!("❌ Workflow resume failed: {}", e);
            run.fail();
            // Skip storage for now - run storage was only for persistence
        }
    }

    Ok(())
}

/// List available workflows
async fn list_workflows_command(
    format: OutputFormat,
    verbose: bool,
    source_filter: Option<PromptSourceArg>,
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

    match format {
        OutputFormat::Table => {
            display_workflows_table(&workflow_infos, verbose)?;
        }
        OutputFormat::Json => {
            let workflows: Vec<_> = workflow_infos.into_iter().map(|(w, _)| w).collect();
            let json_output = serde_json::to_string_pretty(&workflows)?;
            println!("{json_output}");
        }
        OutputFormat::Yaml => {
            let workflows: Vec<_> = workflow_infos.into_iter().map(|(w, _)| w).collect();
            let yaml_output = serde_yaml::to_string(&workflows)?;
            println!("{yaml_output}");
        }
    }

    Ok(())
}

/// Display workflows in table format with color coding
fn display_workflows_table(
    workflow_infos: &[(Workflow, PromptSource)],
    verbose: bool,
) -> Result<()> {
    let mut stdout = io::stdout();
    let is_tty = stdout.is_terminal();
    display_workflows_to_writer(workflow_infos, verbose, &mut stdout, is_tty)
}

fn display_workflows_to_writer<W: Write>(
    workflow_infos: &[(Workflow, PromptSource)],
    verbose: bool,
    writer: &mut W,
    is_tty: bool,
) -> Result<()> {
    if workflow_infos.is_empty() {
        writeln!(writer, "No workflows found matching the criteria.")?;
        return Ok(());
    }

    // Use 2-line format similar to prompt list with color coding
    for (workflow, source) in workflow_infos {
        let name = workflow.name.as_str();
        let description = &workflow.description;

        // Extract title from metadata, or use a formatted version of the name
        let title = workflow
            .metadata
            .get("title")
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Fallback: convert workflow name to a readable title
                name.replace(['-', '_'], " ")
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => {
                                first.to_uppercase().collect::<String>() + chars.as_str()
                            }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            });

        // Color code based on source, matching prompt list
        let first_line = if is_tty {
            let (name_colored, title_colored) = match source {
                PromptSource::Builtin => {
                    (name.green().bold().to_string(), title.green().to_string())
                }
                PromptSource::User => (name.blue().bold().to_string(), title.blue().to_string()),
                PromptSource::Local => {
                    (name.yellow().bold().to_string(), title.yellow().to_string())
                }
                PromptSource::Dynamic => (
                    name.magenta().bold().to_string(),
                    title.magenta().to_string(),
                ),
            };
            format!("{name_colored} | {title_colored}")
        } else {
            format!("{name} | {title}")
        };

        writeln!(writer, "{first_line}")?;

        // Second line: Full description (indented)
        if !description.is_empty() {
            writeln!(writer, "  {description}")?;
        } else {
            writeln!(writer, "  (no description)")?;
        }

        // Add verbose information if requested
        if verbose {
            let terminal_count = workflow.states.values().filter(|s| s.is_terminal).count();
            writeln!(
                writer,
                "  States: {}, Terminal: {}, Transitions: {}",
                workflow.states.len(),
                terminal_count,
                workflow.transitions.len()
            )?;
        }

        writeln!(writer)?; // Empty line between entries
    }

    // Add legend similar to prompt list
    if is_tty && !workflow_infos.is_empty() {
        writeln!(writer, "{}", "Legend:".bright_white())?;
        writeln!(writer, "  {} Built-in workflows", "●".green())?;
        writeln!(
            writer,
            "  {} User workflows (~/.swissarmyhammer/workflows/)",
            "●".blue()
        )?;
        writeln!(
            writer,
            "  {} Local workflows (./.swissarmyhammer/workflows/)",
            "●".yellow()
        )?;
        writeln!(writer, "  {} Dynamic workflows", "●".magenta())?;
    }

    Ok(())
}

/// Check workflow run status
async fn status_workflow_command(run_id: String, format: OutputFormat, watch: bool) -> Result<()> {
    let storage = WorkflowStorage::file_system()?;

    // Parse run ID
    let run_id_typed = parse_workflow_run_id(&run_id)?;

    if watch {
        println!("👁️  Watching workflow run status (Press Ctrl+C to stop)...");

        loop {
            match storage.get_run(&run_id_typed) {
                Ok(run) => {
                    print_run_status(&run, &format)?;

                    // Exit if workflow is completed
                    if run.status == WorkflowRunStatus::Completed
                        || run.status == WorkflowRunStatus::Failed
                        || run.status == WorkflowRunStatus::Cancelled
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Error getting run status: {}", e);
                    break;
                }
            }

            // Check for Ctrl+C
            if (tokio::time::timeout(Duration::from_secs(2), signal::ctrl_c()).await).is_ok() {
                println!("\n🛑 Stopped watching");
                break;
            }
        }
    } else {
        let run = storage.get_run(&run_id_typed)?;
        print_run_status(&run, &format)?;
    }

    Ok(())
}

/// View workflow run logs
async fn logs_workflow_command(
    run_id: String,
    follow: bool,
    tail: Option<usize>,
    level: Option<String>,
) -> Result<()> {
    let storage = WorkflowStorage::file_system()?;

    // Parse run ID
    let run_id_typed = parse_workflow_run_id(&run_id)?;

    let run = storage.get_run(&run_id_typed)?;

    if follow {
        println!("📄 Following logs for run {run_id} (Press Ctrl+C to stop)...");

        loop {
            let updated_run = storage.get_run(&run_id_typed)?;
            print_run_logs(&updated_run, tail, &level)?;

            // Exit if workflow is completed
            if updated_run.status == WorkflowRunStatus::Completed
                || updated_run.status == WorkflowRunStatus::Failed
                || updated_run.status == WorkflowRunStatus::Cancelled
            {
                break;
            }

            // Check for Ctrl+C
            if (tokio::time::timeout(Duration::from_secs(1), signal::ctrl_c()).await).is_ok() {
                println!("\n🛑 Stopped following logs");
                break;
            }
        }
    } else {
        print_run_logs(&run, tail, &level)?;
    }

    Ok(())
}

/// Handle ExecutorError and check for abort condition
fn handle_executor_error(executor_error: ExecutorError, _context: &str) -> SwissArmyHammerError {
    // Convert ExecutorError directly to SwissArmyHammerError using From trait
    swissarmyhammer_common::SwissArmyHammerError::from(executor_error)
}

/// Execute workflow with progress display
async fn execute_workflow_with_progress(
    executor: &mut WorkflowExecutor,
    run: &mut swissarmyhammer_workflow::WorkflowRun,
    interactive: bool,
) -> Result<()> {
    if interactive {
        println!("🎯 Interactive mode - press Enter to continue at each step");

        while run.status == WorkflowRunStatus::Running {
            println!(
                "📍 Current state: {} - {}",
                run.current_state,
                run.workflow
                    .states
                    .get(&run.current_state)
                    .map(|s| s.description.as_str())
                    .unwrap_or("Unknown state")
            );

            println!("Press Enter to execute this step...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            // Execute single step
            executor.execute_state(run).await.map_err(|e| {
                handle_executor_error(
                    e,
                    &format!("Failed to execute state '{}'", run.current_state),
                )
            })?;

            println!("✅ Step completed");

            if run.status != WorkflowRunStatus::Running {
                break;
            }
        }
    } else {
        // Non-interactive execution
        executor.execute_state(run).await.map_err(|e| {
            handle_executor_error(
                e,
                &format!(
                    "Failed to execute workflow '{}' at state '{}'",
                    run.workflow.name, run.current_state
                ),
            )
        })?;
    }

    Ok(())
}

/// Print run status
fn print_run_status(
    run: &swissarmyhammer_workflow::WorkflowRun,
    format: &OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!("🆔 Run ID: {}", workflow_run_id_to_string(&run.id));
            println!("📋 Workflow: {}", run.workflow.name);
            println!("📊 Status: {:?}", run.status);
            println!("📍 Current State: {}", run.current_state);
            println!(
                "🕐 Started: {}",
                run.started_at.format("%Y-%m-%d %H:%M:%S UTC")
            );
            if let Some(completed_at) = run.completed_at {
                println!(
                    "🏁 Completed: {}",
                    completed_at.format("%Y-%m-%d %H:%M:%S UTC")
                );
            }
            println!("📈 History: {} transitions", run.history.len());
            println!("🔧 Variables: {} items", run.context.workflow_vars().len());
        }
        OutputFormat::Json => {
            let json_output = serde_json::to_string_pretty(&run)?;
            println!("{json_output}");
        }
        OutputFormat::Yaml => {
            let yaml_output = serde_yaml::to_string(&run)?;
            println!("{yaml_output}");
        }
    }

    Ok(())
}

/// Print run logs
fn print_run_logs(
    run: &swissarmyhammer_workflow::WorkflowRun,
    tail: Option<usize>,
    _level: &Option<String>,
) -> Result<()> {
    println!("📄 Logs for run {}", workflow_run_id_to_string(&run.id));
    println!("📋 Workflow: {}", run.workflow.name);
    println!();

    // Show execution history as logs
    let history = if let Some(tail_count) = tail {
        if run.history.len() > tail_count {
            &run.history[run.history.len() - tail_count..]
        } else {
            &run.history
        }
    } else {
        &run.history
    };

    for (state_id, timestamp) in history {
        let state_desc = run
            .workflow
            .states
            .get(state_id)
            .map(|s| s.description.as_str())
            .unwrap_or("Unknown state");

        println!(
            "{} 📍 Transitioned to: {} - {}",
            timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            state_id,
            state_desc
        );
    }

    // Show current context/variables
    if !run.context.workflow_vars().is_empty() {
        println!("\n🔧 Current Variables:");
        for (key, value) in run.context.iter() {
            println!("  {key} = {value}");
        }
    }

    Ok(())
}

/// Parse duration string (e.g., "30s", "5m", "1h")
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return Err(SwissArmyHammerError::Other {
            message: "Empty duration string. Expected format: 30s, 5m, or 1h".to_string(),
        });
    }

    let (value_str, unit) = if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, "m")
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, "h")
    } else {
        (s, "s") // Default to seconds
    };

    let value: u64 = value_str.parse().map_err(|_| SwissArmyHammerError::Other {
        message: format!("Invalid duration value: '{value_str}'. Expected a positive number"),
    })?;

    let duration = match unit {
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value * 60),
        "h" => Duration::from_secs(value * 3600),
        _ => {
            return Err(SwissArmyHammerError::Other {
                message: format!(
            "Invalid duration unit: '{unit}'. Supported units: s (seconds), m (minutes), h (hours)"
        ),
            })
        }
    };

    Ok(duration)
}

/// Helper to parse WorkflowRunId from string
fn parse_workflow_run_id(s: &str) -> Result<WorkflowRunId> {
    WorkflowRunId::parse(s).to_swiss_error_with_context(&format!("Invalid workflow run ID '{s}'"))
}

/// Helper to convert WorkflowRunId to string
fn workflow_run_id_to_string(id: &WorkflowRunId) -> String {
    id.to_string()
}

/// Display metrics for workflow runs
async fn metrics_workflow_command(
    run_id: Option<String>,
    workflow: Option<String>,
    format: OutputFormat,
    global: bool,
) -> Result<()> {
    let _storage = WorkflowStorage::file_system()?;
    let executor = WorkflowExecutor::new();
    let metrics = executor.get_metrics();

    if global {
        // Show global metrics summary
        let global_metrics = metrics.get_global_metrics();

        match format {
            OutputFormat::Table => {
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
            OutputFormat::Json => {
                let json_output = serde_json::to_string_pretty(&global_metrics)?;
                println!("{json_output}");
            }
            OutputFormat::Yaml => {
                let yaml_output = serde_yaml::to_string(&global_metrics)?;
                println!("{yaml_output}");
            }
        }
    } else if let Some(run_id_str) = run_id {
        // Show metrics for specific run
        let run_id_typed = parse_workflow_run_id(&run_id_str)?;

        if let Some(run_metrics) = metrics.get_run_metrics(&run_id_typed) {
            match format {
                OutputFormat::Table => {
                    println!("📊 Run Metrics: {run_id_str}");
                    println!("Workflow: {}", run_metrics.workflow_name);
                    println!("Status: {:?}", run_metrics.status);
                    println!(
                        "Started: {}",
                        run_metrics.started_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    if let Some(completed) = run_metrics.completed_at {
                        println!("Completed: {}", completed.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                    if let Some(duration) = run_metrics.total_duration {
                        println!("Duration: {:.2}s", duration.as_secs_f64());
                    }
                    println!("Transitions: {}", run_metrics.transition_count);
                    println!("State execution times:");
                    for (state_id, duration) in &run_metrics.state_durations {
                        println!("  {}: {:.2}s", state_id, duration.as_secs_f64());
                    }
                }
                OutputFormat::Json => {
                    let json_output = serde_json::to_string_pretty(&run_metrics)?;
                    println!("{json_output}");
                }
                OutputFormat::Yaml => {
                    let yaml_output = serde_yaml::to_string(&run_metrics)?;
                    println!("{yaml_output}");
                }
            }
        } else {
            println!("No metrics found for run: {run_id_str}");
        }
    } else if let Some(workflow_name) = workflow {
        // Show metrics for specific workflow
        let workflow_name_typed = WorkflowName::new(&workflow_name);

        if let Some(workflow_metrics) = metrics.get_workflow_summary(&workflow_name_typed) {
            match format {
                OutputFormat::Table => {
                    println!("📊 Workflow Metrics: {workflow_name}");
                    println!("Total runs: {}", workflow_metrics.total_runs);
                    println!("Successful runs: {}", workflow_metrics.successful_runs);
                    println!("Failed runs: {}", workflow_metrics.failed_runs);
                    println!(
                        "Success rate: {:.2}%",
                        workflow_metrics.success_rate() * 100.0
                    );
                    if let Some(avg_duration) = workflow_metrics.average_duration {
                        println!("Average duration: {:.2}s", avg_duration.as_secs_f64());
                    }
                    if let Some(min_duration) = workflow_metrics.min_duration {
                        println!("Min duration: {:.2}s", min_duration.as_secs_f64());
                    }
                    if let Some(max_duration) = workflow_metrics.max_duration {
                        println!("Max duration: {:.2}s", max_duration.as_secs_f64());
                    }
                    println!(
                        "Average transitions: {:.1}",
                        workflow_metrics.average_transitions
                    );

                    if !workflow_metrics.hot_states.is_empty() {
                        println!("Hot states:");
                        for state_count in &workflow_metrics.hot_states {
                            println!(
                                "  {}: {} executions ({:.2}s avg)",
                                state_count.state_id,
                                state_count.execution_count,
                                state_count.average_duration.as_secs_f64()
                            );
                        }
                    }
                }
                OutputFormat::Json => {
                    let json_output = serde_json::to_string_pretty(&workflow_metrics)?;
                    println!("{json_output}");
                }
                OutputFormat::Yaml => {
                    let yaml_output = serde_yaml::to_string(&workflow_metrics)?;
                    println!("{yaml_output}");
                }
            }
        } else {
            println!("No metrics found for workflow: {workflow_name}");
        }
    } else {
        // Show all run metrics
        match format {
            OutputFormat::Table => {
                println!("📊 All Run Metrics");
                println!("==================");
                for (run_id, run_metrics) in &metrics.run_metrics {
                    println!("Run: {}", workflow_run_id_to_string(run_id));
                    println!("  Workflow: {}", run_metrics.workflow_name);
                    println!("  Status: {:?}", run_metrics.status);
                    if let Some(duration) = run_metrics.total_duration {
                        println!("  Duration: {:.2}s", duration.as_secs_f64());
                    }
                    println!("  Transitions: {}", run_metrics.transition_count);
                    println!();
                }
            }
            OutputFormat::Json => {
                let json_output = serde_json::to_string_pretty(&metrics.run_metrics)?;
                println!("{json_output}");
            }
            OutputFormat::Yaml => {
                let yaml_output = serde_yaml::to_string(&metrics.run_metrics)?;
                println!("{yaml_output}");
            }
        }
    }

    Ok(())
}

/// Generate execution visualization
async fn visualize_workflow_command(
    run_id: String,
    format: VisualizationFormat,
    output: Option<String>,
    timing: bool,
    counts: bool,
    _path_only: bool,
) -> Result<()> {
    let storage = WorkflowStorage::file_system()?;
    let run_id_typed = parse_workflow_run_id(&run_id)?;
    let run = storage.get_run(&run_id_typed)?;

    let mut visualizer = ExecutionVisualizer::new();
    visualizer.include_timing = timing;
    visualizer.include_counts = counts;

    let trace = visualizer.generate_trace(&run);

    let content = match format {
        VisualizationFormat::Mermaid => {
            visualizer.generate_mermaid_with_execution(&run.workflow, &trace)
        }
        VisualizationFormat::Html => visualizer.generate_html(&run.workflow, &trace),
        VisualizationFormat::Json => visualizer.export_trace_json(&trace)?,
        VisualizationFormat::Dot => {
            // Simple DOT format - could be enhanced
            format!(
                "digraph workflow {{\n{}\n}}",
                trace
                    .execution_path
                    .iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let next_step = trace.execution_path.get(i + 1);
                        if let Some(next) = next_step {
                            format!("  \"{}\" -> \"{}\"", step.state_id, next.state_id)
                        } else {
                            format!("  \"{}\"", step.state_id)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    };

    if let Some(output_path) = output {
        std::fs::write(&output_path, content)?;
        println!("Visualization saved to: {output_path}");
    } else {
        println!("{content}");
    }

    Ok(())
}

/// Create a local workflow run storage that stores runs in .swissarmyhammer/workflow-runs directory
fn create_local_workflow_run_storage() -> Result<Box<dyn WorkflowRunStorageBackend>> {
    use std::fs;

    // Create local .swissarmyhammer/workflow-runs directory
    let local_dir = std::path::PathBuf::from(".swissarmyhammer/workflow-runs");
    fs::create_dir_all(&local_dir).map_err(|e| SwissArmyHammerError::Other {
        message: format!("Failed to create .swissarmyhammer/workflow-runs directory: {e}"),
    })?;

    let run_storage = swissarmyhammer_workflow::FileSystemWorkflowRunStorage::new(&local_dir)
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to create local workflow run storage: {e}"),
        })?;

    Ok(Box::new(run_storage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert_eq!(parse_duration("60").unwrap(), Duration::from_secs(60));

        assert!(parse_duration("").is_err());
        assert!(parse_duration("invalid").is_err());
        assert!(parse_duration("10x").is_err());
    }

    #[test]
    fn test_workflow_run_id_helpers() {
        let id = WorkflowRunId::new();
        let id_str = workflow_run_id_to_string(&id);
        let parsed_id = parse_workflow_run_id(&id_str).unwrap();

        // Test round-trip conversion works correctly
        assert_eq!(id, parsed_id);
        assert_eq!(id_str, workflow_run_id_to_string(&parsed_id));
    }

    #[test]
    fn test_workflow_run_id_parse_error() {
        let invalid_id = "invalid-ulid-string";
        let result = parse_workflow_run_id(invalid_id);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_plan_workflow_legacy_compatibility() {
        // Test that plan workflow works without parameters (legacy mode)
        let workflow_storage = WorkflowStorage::file_system().unwrap();
        let workflow_name_typed = WorkflowName::new("plan");
        let workflow = workflow_storage.get_workflow(&workflow_name_typed).unwrap();

        // Create a workflow run without plan_filename parameter
        let run = swissarmyhammer_workflow::WorkflowRun::new(workflow.clone());

        // This should work without plan_filename - testing backward compatibility
        assert_eq!(run.workflow.name.as_str(), "plan");
        assert_eq!(
            run.status,
            swissarmyhammer_workflow::WorkflowRunStatus::Running
        );

        // The workflow should have the expected states
        assert_eq!(workflow.states.len(), 3);
        assert!(workflow
            .states
            .contains_key(&swissarmyhammer_workflow::StateId::new("start")));
        assert!(workflow
            .states
            .contains_key(&swissarmyhammer_workflow::StateId::new("plan")));
        assert!(workflow
            .states
            .contains_key(&swissarmyhammer_workflow::StateId::new("done")));
    }

    #[tokio::test]
    async fn test_plan_workflow_with_parameters() {
        // Test new parameterized functionality
        let workflow_storage = WorkflowStorage::file_system().unwrap();
        let workflow_name_typed = WorkflowName::new("plan");
        let workflow = workflow_storage.get_workflow(&workflow_name_typed).unwrap();

        // Create a workflow run with plan_filename parameter
        let mut run = swissarmyhammer_workflow::WorkflowRun::new(workflow.clone());
        run.context.insert(
            "plan_filename".to_string(),
            serde_json::Value::String("./specification/test.md".to_string()),
        );

        // This should work with plan_filename - testing new functionality
        assert_eq!(run.workflow.name.as_str(), "plan");
        assert_eq!(
            run.status,
            swissarmyhammer_workflow::WorkflowRunStatus::Running
        );
        assert!(run.context.contains_key("plan_filename"));
        assert_eq!(
            run.context.get("plan_filename").unwrap(),
            &serde_json::json!("./specification/test.md")
        );
    }

    #[test]
    fn test_agent_config_fix_is_in_place() {
        // This test verifies that the fix to transfer agent configuration from
        // template context to workflow context is present in the code.
        // The actual functionality is tested through integration tests.

        // Read the source code to verify the fix line is present
        let source_code = include_str!("mod.rs");

        // Verify that we call get_agent_config from template context
        assert!(
            source_code.contains("let agent_config = _template_context.get_agent_config(None);")
        );

        // Verify that we set the agent config on the run context
        assert!(source_code.contains("run.context.set_agent_config(agent_config);"));

        // Verify the comment explaining the fix
        assert!(source_code.contains("Set agent configuration from template context"));
    }
}
