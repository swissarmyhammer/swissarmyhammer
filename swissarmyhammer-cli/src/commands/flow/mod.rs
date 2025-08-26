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
use std::collections::{HashMap, HashSet};
use std::future;
use std::io::{self, Write};
use std::time::Duration;
use swissarmyhammer::workflow::{
    ExecutionVisualizer, ExecutorError, MemoryWorkflowStorage, StateId, TransitionKey, Workflow,
    WorkflowExecutor, WorkflowName, WorkflowResolver, WorkflowRunId, WorkflowRunStatus,
    WorkflowRunStorageBackend, WorkflowStorage, WorkflowStorageBackend,
};
use swissarmyhammer::{Result, SwissArmyHammerError};
use tokio::signal;
use tokio::time::timeout;

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Default timeout for workflow test mode execution in seconds
const DEFAULT_TEST_MODE_TIMEOUT_SECS: u64 = 5;

/// Setup debug logging for a specific workflow run in the correct directory per spec
fn setup_workflow_debug_logging(run_id: &WorkflowRunId) -> Result<()> {
    use std::fs;

    // Create debug log file in proper workflow run directory per specification
    let workflow_runs_path = std::path::PathBuf::from(".swissarmyhammer/workflow-runs");
    let run_dir = workflow_runs_path.join("runs").join(format!("{run_id:?}"));
    
    // Ensure directory exists
    fs::create_dir_all(&run_dir).map_err(|e| {
        SwissArmyHammerError::Other(format!(
            "Failed to create workflow run directory: {e}"
        ))
    })?;

    let debug_log_path = run_dir.join("run_logs.ndjson");
    
    // Create debug log file that will be populated by the NDJSON layer
    fs::File::create(&debug_log_path).map_err(|e| {
        SwissArmyHammerError::Other(format!(
            "Failed to create debug log file: {e}"
        ))
    })?;
    
    tracing::info!(
        run_id = %run_id,
        debug_log_path = %debug_log_path.display(),
        "Debug logging enabled for workflow run"
    );

    Ok(())
}

/// Handle the flow command
pub async fn handle_command(
    subcommand: FlowSubcommand,
    _template_context: &swissarmyhammer_config::TemplateContext,
    debug: bool,
) -> i32 {
    match run_flow_command(subcommand, _template_context, debug).await {
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
    debug: bool,
) -> Result<()> {
    match subcommand {
        FlowSubcommand::Run {
            workflow,
            vars,
            interactive,
            dry_run,
            test,
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
                    test_mode: test,
                    timeout_str,
                    quiet,
                    debug,
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
                    test_mode: true,
                    timeout_str,
                    quiet,
                    debug,
                },
                _template_context,
            )
            .await
        }
    }
}

/// Configuration for running a workflow command
struct WorkflowCommandConfig {
    workflow_name: String,
    vars: Vec<String>,
    interactive: bool,
    dry_run: bool,
    test_mode: bool,
    timeout_str: Option<String>,
    quiet: bool,
    debug: bool,
}

/// Execute a workflow
async fn run_workflow_command(
    config: WorkflowCommandConfig,
    _template_context: &swissarmyhammer_config::TemplateContext,
) -> Result<()> {
    // Use proper WorkflowStorage with embedded builtins
    let workflow_storage = tokio::task::spawn_blocking(WorkflowStorage::file_system)
        .await
        .map_err(|e| {
            SwissArmyHammerError::Other(format!("Failed to create workflow storage: {e}"))
        })??;

    let workflow_name_typed = WorkflowName::new(&config.workflow_name);
    let workflow = workflow_storage.get_workflow(&workflow_name_typed)?;

    // Resolve workflow parameters with enhanced parameter system
    let workflow_variables = parameter_cli::resolve_workflow_parameters_interactive(
        &config.workflow_name,
        &config.vars,
        config.interactive && !config.dry_run && !config.test_mode,
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
            return Err(SwissArmyHammerError::Other(format!(
                "Invalid variable format: '{var}'. Expected 'key=value' format. Example: --var input=test"
            )));
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

    if config.test_mode {
        println!("🧪 Test mode - executing workflow with mocked actions:");
        println!("📋 Workflow: {}", workflow.name);
        println!("🏁 Initial state: {}", workflow.initial_state);
        println!("🔧 Variables: {variables:?}");
        if let Some(timeout) = timeout_duration {
            println!("⏱️  Timeout: {timeout:?}");
        }

        // Execute in test mode with coverage tracking
        let coverage = execute_workflow_test_mode(workflow, variables, timeout_duration).await?;

        // Generate coverage report
        println!("\n📊 Coverage Report:");

        // Calculate state coverage percentage safely
        let state_percentage = if coverage.total_states > 0 {
            (coverage.visited_states.len() as f64 / coverage.total_states as f64) * 100.0
        } else {
            100.0 // Consider empty workflow as 100% covered
        };

        println!(
            "  States visited: {}/{} ({:.1}%)",
            coverage.visited_states.len(),
            coverage.total_states,
            state_percentage
        );

        // Calculate transition coverage percentage safely
        let transition_percentage = if coverage.total_transitions > 0 {
            (coverage.visited_transitions.len() as f64 / coverage.total_transitions as f64) * 100.0
        } else {
            100.0 // Consider workflow with no transitions as 100% covered
        };

        println!(
            "  Transitions used: {}/{} ({:.1}%)",
            coverage.visited_transitions.len(),
            coverage.total_transitions,
            transition_percentage
        );

        // Show unvisited states
        if !coverage.unvisited_states.is_empty() {
            println!("\n❌ Unvisited states:");
            for state in &coverage.unvisited_states {
                println!("  - {state}");
            }
        }

        // Show unvisited transitions
        if !coverage.unvisited_transitions.is_empty() {
            println!("\n❌ Unvisited transitions:");
            for transition in &coverage.unvisited_transitions {
                println!("  - {transition}");
            }
        }

        if coverage.visited_states.len() == coverage.total_states {
            println!("\n✅ Full state coverage achieved!");
        }
        if coverage.visited_transitions.len() == coverage.total_transitions {
            println!("✅ Full transition coverage achieved!");
        }

        return Ok(());
    }

    tracing::info!("🚀 Starting workflow: {}", workflow.name);

    // Check for abort file before starting workflow
    if let Some(abort_reason) = swissarmyhammer::common::read_abort_file(".")? {
        // Clean up the abort file after detection
        let _ = swissarmyhammer::common::remove_abort_file(".");
        return Err(SwissArmyHammerError::ExecutorError(
            swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
        ));
    }

    // Create executor
    let mut executor = WorkflowExecutor::new();

    // Create workflow run
    let mut run = executor.start_workflow(workflow.clone()).map_err(|e| {
        SwissArmyHammerError::Other(format!(
            "Failed to start workflow '{}': {}",
            workflow.name, e
        ))
    })?;

    // Set initial variables
    run.context.set_workflow_vars(variables.clone());

    // Setup debug logging in proper workflow run directory if enabled
    if config.debug {
        setup_workflow_debug_logging(&run.id)?;
    }

    // Merge configuration context into workflow context
    // Configuration has lower precedence than workflow variables (CLI args have highest precedence)
    // Note: WorkflowTemplateContext already manages template context internally
    // so this explicit merge may not be needed
    // _template_context.merge_into_workflow_context(&mut run.context);

    // Store variables in context for liquid template rendering - this will now include config values
    // The merge_into_workflow_context method properly handles precedence

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
    SwissArmyHammerError::from(executor_error)
}

/// Execute workflow with progress display
async fn execute_workflow_with_progress(
    executor: &mut WorkflowExecutor,
    run: &mut swissarmyhammer::workflow::WorkflowRun,
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
    run: &swissarmyhammer::workflow::WorkflowRun,
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
    run: &swissarmyhammer::workflow::WorkflowRun,
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
        return Err(SwissArmyHammerError::Other(
            "Empty duration string. Expected format: 30s, 5m, or 1h".to_string(),
        ));
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

    let value: u64 = value_str.parse().map_err(|_| {
        SwissArmyHammerError::Other(format!(
            "Invalid duration value: '{value_str}'. Expected a positive number"
        ))
    })?;

    let duration = match unit {
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value * 60),
        "h" => Duration::from_secs(value * 3600),
        _ => {
            return Err(SwissArmyHammerError::Other(format!(
            "Invalid duration unit: '{unit}'. Supported units: s (seconds), m (minutes), h (hours)"
        )))
        }
    };

    Ok(duration)
}

/// Helper to parse WorkflowRunId from string
fn parse_workflow_run_id(s: &str) -> Result<WorkflowRunId> {
    WorkflowRunId::parse(s)
        .map_err(|e| SwissArmyHammerError::Other(format!("Invalid workflow run ID '{s}': {e}")))
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

/// Coverage tracking for workflow test execution
///
/// This struct tracks which parts of a workflow were exercised during test execution,
/// providing metrics for test coverage analysis.
///
/// # Fields
///
/// * `visited_states` - Set of states that were entered during execution
/// * `visited_transitions` - Set of transitions that were taken during execution
/// * `total_states` - Total number of states in the workflow
/// * `total_transitions` - Total number of transitions in the workflow
/// * `unvisited_states` - List of states that were not visited (for reporting)
/// * `unvisited_transitions` - List of transitions that were not taken (for reporting)
struct WorkflowCoverage {
    visited_states: HashSet<StateId>,
    visited_transitions: HashSet<TransitionKey>,
    total_states: usize,
    total_transitions: usize,
    unvisited_states: Vec<StateId>,
    unvisited_transitions: Vec<TransitionKey>,
}

/// Execute workflow in test mode with mocked actions
///
/// This function simulates workflow execution without performing actual actions,
/// allowing for testing workflow logic and generating coverage reports.
///
/// # Algorithm
///
/// 1. Start from the initial state
/// 2. For each state, find available transitions
/// 3. Prefer unvisited transitions to maximize coverage
/// 4. Mock action execution by setting success results
/// 5. Track visited states and transitions for coverage reporting
///
/// # Parameters
///
/// * `workflow` - The workflow to test
/// * `initial_variables` - Initial context variables for the workflow
/// * `timeout_duration` - Optional timeout for execution (defaults to 60 seconds)
///
/// # Returns
///
/// Returns a `WorkflowCoverage` struct containing:
/// * Lists of visited and unvisited states
/// * Lists of visited and unvisited transitions
/// * Total counts for percentage calculations
async fn execute_workflow_test_mode(
    workflow: Workflow,
    initial_variables: HashMap<String, serde_json::Value>,
    timeout_duration: Option<Duration>,
) -> Result<WorkflowCoverage> {
    use swissarmyhammer::workflow::{ConditionType, WorkflowRun};

    let mut coverage = WorkflowCoverage {
        visited_states: HashSet::new(),
        visited_transitions: HashSet::new(),
        total_states: workflow.states.len(),
        total_transitions: workflow.transitions.len(),
        unvisited_states: Vec::new(),
        unvisited_transitions: Vec::new(),
    };

    // Create a mock workflow run
    let mut run = WorkflowRun::new(workflow.clone());
    run.context.set_workflow_vars(initial_variables);

    // All variables are now handled through the regular workflow variables system

    // Track visited states and transitions
    let mut current_state = workflow.initial_state.clone();
    coverage.visited_states.insert(current_state.clone());

    println!("\n▶️  Starting test execution...");

    // Simple execution loop - try to visit all states
    let start_time = std::time::Instant::now();
    let timeout = timeout_duration.unwrap_or(Duration::from_secs(DEFAULT_TEST_MODE_TIMEOUT_SECS));
    let mut steps_without_progress = 0;
    const MAX_STEPS_WITHOUT_PROGRESS: usize = 3;

    while !workflow
        .states
        .get(&current_state)
        .map(|s| s.is_terminal)
        .unwrap_or(false)
    {
        if start_time.elapsed() > timeout {
            tracing::warn!("Test execution timed out after {:?}", timeout);
            break;
        }

        if steps_without_progress >= MAX_STEPS_WITHOUT_PROGRESS {
            tracing::debug!(
                "No progress made for {} steps, terminating early",
                MAX_STEPS_WITHOUT_PROGRESS
            );
            break;
        }

        // Find transitions from current state
        let available_transitions: Vec<_> = workflow
            .transitions
            .iter()
            .filter(|t| t.from_state == current_state)
            .collect();

        if available_transitions.is_empty() {
            tracing::warn!("No transitions available from state: {}", current_state);
            break;
        }

        // Try each transition, preferring unvisited ones
        let mut transition_taken = false;
        for transition in &available_transitions {
            let transition_key =
                TransitionKey::from_refs(&transition.from_state, &transition.to_state);

            // Check if we should take this transition based on condition
            let should_take = match &transition.condition.condition_type {
                ConditionType::Always => true,
                ConditionType::Never => false,
                ConditionType::OnSuccess => true, // Mock success
                ConditionType::OnFailure => false,
                ConditionType::Custom => true, // Always true in test mode
            };

            if should_take
                && (!coverage.visited_transitions.contains(&transition_key)
                    || available_transitions.len() == 1)
            {
                // Mock action execution
                if let Some(action) = &transition.action {
                    tracing::debug!("Mock executing action: {}", action);
                    // Set mock result in context
                    run.context.insert(
                        "result".to_string(),
                        serde_json::json!({
                            "success": true,
                            "output": "Mock output"
                        }),
                    );
                }

                // Take the transition
                tracing::debug!("Taking transition: {}", transition_key);
                coverage.visited_transitions.insert(transition_key);
                coverage.visited_states.insert(transition.to_state.clone());
                current_state = transition.to_state.clone();
                transition_taken = true;
                break;
            }
        }

        if !transition_taken {
            // All transitions have been visited or conditions not met
            tracing::debug!("All transitions from {} have been explored", current_state);
            steps_without_progress += 1;
        } else {
            steps_without_progress = 0; // Reset counter when progress is made
        }
    }

    // Calculate unvisited states and transitions
    for state_id in workflow.states.keys() {
        if !coverage.visited_states.contains(state_id) {
            coverage.unvisited_states.push(state_id.clone());
        }
    }

    for transition in &workflow.transitions {
        let transition_key = TransitionKey::from_refs(&transition.from_state, &transition.to_state);
        if !coverage.visited_transitions.contains(&transition_key) {
            coverage.unvisited_transitions.push(transition_key);
        }
    }

    println!("\n✅ Test execution completed");

    Ok(coverage)
}

/// Create a local workflow run storage that stores runs in .swissarmyhammer/workflow-runs directory
fn create_local_workflow_run_storage() -> Result<Box<dyn WorkflowRunStorageBackend>> {
    use std::fs;

    // Create local .swissarmyhammer/workflow-runs directory
    let local_dir = std::path::PathBuf::from(".swissarmyhammer/workflow-runs");
    fs::create_dir_all(&local_dir).map_err(|e| {
        SwissArmyHammerError::Other(format!(
            "Failed to create .swissarmyhammer/workflow-runs directory: {e}"
        ))
    })?;

    let run_storage = swissarmyhammer::workflow::FileSystemWorkflowRunStorage::new(&local_dir)
        .map_err(|e| {
            SwissArmyHammerError::Other(format!("Failed to create local workflow run storage: {e}"))
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
    async fn test_execute_workflow_test_mode_simple_workflow() {
        use swissarmyhammer::workflow::{
            ConditionType, State, StateType, Transition, TransitionCondition, WorkflowName,
        };

        // Create a simple workflow: Start -> End
        let mut workflow = Workflow::new(
            WorkflowName::new("test"),
            "Test workflow".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(State {
            id: StateId::new("start"),
            description: "Start state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("end"),
            description: "End state".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_transition(Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("end"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: Some("log \"Moving to end\"".to_string()),
            metadata: HashMap::new(),
        });

        let variables = HashMap::new();
        let coverage = execute_workflow_test_mode(workflow, variables, None)
            .await
            .unwrap();

        // Check coverage
        assert_eq!(coverage.visited_states.len(), 2);
        assert_eq!(coverage.visited_transitions.len(), 1);
        assert_eq!(coverage.total_states, 2);
        assert_eq!(coverage.total_transitions, 1);
        assert!(coverage.unvisited_states.is_empty());
        assert!(coverage.unvisited_transitions.is_empty());
    }

    #[tokio::test]
    async fn test_execute_workflow_test_mode_with_conditions() {
        use swissarmyhammer::workflow::{
            ConditionType, State, StateType, Transition, TransitionCondition, WorkflowName,
        };

        // Create workflow with conditional transitions
        let mut workflow = Workflow::new(
            WorkflowName::new("conditional"),
            "Conditional workflow".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(State {
            id: StateId::new("start"),
            description: "Start state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("success"),
            description: "Success state".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("failure"),
            description: "Failure state".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        // OnSuccess transition (should be taken in test mode)
        workflow.add_transition(Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("success"),
            condition: TransitionCondition {
                condition_type: ConditionType::OnSuccess,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        // OnFailure transition (should NOT be taken in test mode)
        workflow.add_transition(Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("failure"),
            condition: TransitionCondition {
                condition_type: ConditionType::OnFailure,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        let variables = HashMap::new();
        let coverage = execute_workflow_test_mode(workflow, variables, None)
            .await
            .unwrap();

        // Should visit start and success, but not failure
        assert_eq!(coverage.visited_states.len(), 2);
        assert!(coverage.visited_states.contains(&StateId::new("start")));
        assert!(coverage.visited_states.contains(&StateId::new("success")));
        assert!(!coverage.visited_states.contains(&StateId::new("failure")));

        // Should have one unvisited state and transition
        assert_eq!(coverage.unvisited_states.len(), 1);
        assert_eq!(coverage.unvisited_transitions.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_workflow_test_mode_timeout() {
        use swissarmyhammer::workflow::{
            ConditionType, State, StateType, Transition, TransitionCondition, WorkflowName,
        };

        // Create an infinite loop workflow
        let mut workflow = Workflow::new(
            WorkflowName::new("loop"),
            "Loop workflow".to_string(),
            StateId::new("state1"),
        );

        workflow.add_state(State {
            id: StateId::new("state1"),
            description: "State 1".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("state2"),
            description: "State 2".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        // Create a loop
        workflow.add_transition(Transition {
            from_state: StateId::new("state1"),
            to_state: StateId::new("state2"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        workflow.add_transition(Transition {
            from_state: StateId::new("state2"),
            to_state: StateId::new("state1"),
            condition: TransitionCondition {
                condition_type: ConditionType::Always,
                expression: None,
            },
            action: None,
            metadata: HashMap::new(),
        });

        let variables = HashMap::new();
        // Use a very short timeout
        let timeout = Some(Duration::from_millis(100));
        let coverage = execute_workflow_test_mode(workflow, variables, timeout)
            .await
            .unwrap();

        // Should have visited both states
        assert_eq!(coverage.visited_states.len(), 2);
        assert_eq!(coverage.visited_transitions.len(), 2);
    }

    #[tokio::test]
    async fn test_execute_workflow_test_mode_no_transitions() {
        use swissarmyhammer::workflow::{State, StateType, WorkflowName};

        // Create workflow with isolated state
        let mut workflow = Workflow::new(
            WorkflowName::new("isolated"),
            "Isolated workflow".to_string(),
            StateId::new("alone"),
        );

        workflow.add_state(State {
            id: StateId::new("alone"),
            description: "Alone state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        let variables = HashMap::new();
        let coverage = execute_workflow_test_mode(workflow, variables, None)
            .await
            .unwrap();

        // Should visit only the initial state
        assert_eq!(coverage.visited_states.len(), 1);
        assert_eq!(coverage.visited_transitions.len(), 0);
        assert_eq!(coverage.total_transitions, 0);
    }

    #[tokio::test]
    async fn test_execute_workflow_test_mode_with_variables() {
        use swissarmyhammer::workflow::{
            ConditionType, State, StateType, Transition, TransitionCondition, WorkflowName,
        };

        // Create workflow that uses variables
        let mut workflow = Workflow::new(
            WorkflowName::new("vars"),
            "Variables workflow".to_string(),
            StateId::new("start"),
        );

        workflow.add_state(State {
            id: StateId::new("start"),
            description: "Start state".to_string(),
            state_type: StateType::Normal,
            is_terminal: false,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_state(State {
            id: StateId::new("end"),
            description: "End state".to_string(),
            state_type: StateType::Normal,
            is_terminal: true,
            allows_parallel: false,
            metadata: HashMap::new(),
        });

        workflow.add_transition(Transition {
            from_state: StateId::new("start"),
            to_state: StateId::new("end"),
            condition: TransitionCondition {
                condition_type: ConditionType::Custom,
                expression: Some("input == \"test\"".to_string()),
            },
            action: Some("set_variable output \"processed\"".to_string()),
            metadata: HashMap::new(),
        });

        let mut variables = HashMap::new();
        variables.insert("input".to_string(), serde_json::json!("test"));

        let coverage = execute_workflow_test_mode(workflow, variables, None)
            .await
            .unwrap();

        // Should complete the workflow
        assert_eq!(coverage.visited_states.len(), 2);
        assert_eq!(coverage.visited_transitions.len(), 1);
        assert!(coverage.unvisited_states.is_empty());
        assert!(coverage.unvisited_transitions.is_empty());
    }

    #[tokio::test]
    async fn test_execute_workflow_test_mode_empty_workflow() {
        use swissarmyhammer::workflow::WorkflowName;

        // Create empty workflow (will fail validation but test mode should handle it)
        let workflow = Workflow::new(
            WorkflowName::new("empty"),
            "Empty workflow".to_string(),
            StateId::new("nonexistent"),
        );

        let variables = HashMap::new();
        let coverage = execute_workflow_test_mode(workflow, variables, None)
            .await
            .unwrap();

        // Should handle gracefully - initial state is tracked even if not in workflow
        assert_eq!(coverage.visited_states.len(), 1);
        assert_eq!(coverage.visited_transitions.len(), 0);
        assert_eq!(coverage.total_states, 0);
        assert_eq!(coverage.total_transitions, 0);
    }

    #[tokio::test]
    async fn test_plan_workflow_legacy_compatibility() {
        // Test that plan workflow works without parameters (legacy mode)
        let workflow_storage = WorkflowStorage::file_system().unwrap();
        let workflow_name_typed = WorkflowName::new("plan");
        let workflow = workflow_storage.get_workflow(&workflow_name_typed).unwrap();

        // Create a workflow run without plan_filename parameter
        let run = swissarmyhammer::workflow::WorkflowRun::new(workflow.clone());

        // This should work without plan_filename - testing backward compatibility
        assert_eq!(run.workflow.name.as_str(), "plan");
        assert_eq!(
            run.status,
            swissarmyhammer::workflow::WorkflowRunStatus::Running
        );

        // The workflow should have the expected states
        assert_eq!(workflow.states.len(), 3);
        assert!(workflow
            .states
            .contains_key(&swissarmyhammer::workflow::StateId::new("start")));
        assert!(workflow
            .states
            .contains_key(&swissarmyhammer::workflow::StateId::new("plan")));
        assert!(workflow
            .states
            .contains_key(&swissarmyhammer::workflow::StateId::new("done")));
    }

    #[tokio::test]
    async fn test_plan_workflow_with_parameters() {
        // Test new parameterized functionality
        let workflow_storage = WorkflowStorage::file_system().unwrap();
        let workflow_name_typed = WorkflowName::new("plan");
        let workflow = workflow_storage.get_workflow(&workflow_name_typed).unwrap();

        // Create a workflow run with plan_filename parameter
        let mut run = swissarmyhammer::workflow::WorkflowRun::new(workflow.clone());
        run.context.insert(
            "plan_filename".to_string(),
            serde_json::Value::String("./specification/test.md".to_string()),
        );

        // This should work with plan_filename - testing new functionality
        assert_eq!(run.workflow.name.as_str(), "plan");
        assert_eq!(
            run.status,
            swissarmyhammer::workflow::WorkflowRunStatus::Running
        );
        assert!(run.context.contains_key("plan_filename"));
        assert_eq!(
            run.context.get("plan_filename").unwrap(),
            &serde_json::json!("./specification/test.md")
        );
    }
}
