//! Shared utilities for flow command subcommands

use crate::context::CliContext;

use swissarmyhammer::{Result, SwissArmyHammerError, WorkflowRunId, WorkflowRunStorageBackend};
use swissarmyhammer::{WorkflowExecutor, WorkflowRunStatus};
use swissarmyhammer_workflow::{ExecutorError, WorkflowRun};

/// Helper to parse WorkflowRunId from string
pub fn parse_workflow_run_id(s: &str) -> Result<WorkflowRunId> {
    WorkflowRunId::parse(s).map_err(|e| SwissArmyHammerError::Other {
        message: format!("Invalid workflow run ID '{s}': {e}"),
    })
}

/// Helper to convert WorkflowRunId to string
pub fn workflow_run_id_to_string(id: &WorkflowRunId) -> String {
    id.to_string()
}

/// Convert ExecutorError to SwissArmyHammerError
pub fn handle_executor_error(
    executor_error: ExecutorError,
    _context: &str,
) -> SwissArmyHammerError {
    // Convert ExecutorError directly to SwissArmyHammerError using From trait
    swissarmyhammer_common::SwissArmyHammerError::from(executor_error)
}

/// Execute workflow with progress display and interactive support
pub async fn execute_workflow_with_progress(
    executor: &mut WorkflowExecutor,
    run: &mut WorkflowRun,
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

/// Print workflow run status using CliContext formatting
pub fn print_run_status(run: &WorkflowRun, context: &CliContext) -> Result<()> {
    // For now, use simple serialization since WorkflowRun doesn't implement Tabled
    match context.format {
        crate::cli::OutputFormat::Table => {
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
        crate::cli::OutputFormat::Json => {
            let json_output = serde_json::to_string_pretty(&run)?;
            println!("{json_output}");
        }
        crate::cli::OutputFormat::Yaml => {
            let yaml_output = serde_yaml::to_string(&run)?;
            println!("{yaml_output}");
        }
    }
    Ok(())
}

/// Log level for filtering workflow logs
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Parse log level from string
    fn from_str(s: &str) -> Option<LogLevel> {
        match s.to_lowercase().as_str() {
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warn" | "warning" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            _ => None,
        }
    }

    /// Get the emoji for this log level
    fn emoji(&self) -> &str {
        match self {
            LogLevel::Debug => "🔍",
            LogLevel::Info => "📍",
            LogLevel::Warn => "⚠️",
            LogLevel::Error => "❌",
        }
    }
}

/// Determine log level for a workflow transition based on state properties
fn get_state_log_level(
    run: &WorkflowRun,
    state_id: &swissarmyhammer_workflow::StateId,
) -> LogLevel {
    if let Some(state) = run.workflow.states.get(state_id) {
        // Terminal states that are successful are Info level
        if state.is_terminal && run.status == WorkflowRunStatus::Completed {
            return LogLevel::Info;
        }
        // Terminal states that are failed are Error level
        if state.is_terminal && run.status == WorkflowRunStatus::Failed {
            return LogLevel::Error;
        }
        // States with "error", "fail" in description are Error level
        if state.description.to_lowercase().contains("error")
            || state.description.to_lowercase().contains("fail")
        {
            return LogLevel::Error;
        }
        // States with "warn" in description are Warn level
        if state.description.to_lowercase().contains("warn") {
            return LogLevel::Warn;
        }
    }
    // Default to Info level
    LogLevel::Info
}

/// Print workflow run logs
pub fn print_run_logs(
    run: &WorkflowRun,
    tail: Option<usize>,
    level: &Option<String>,
) -> Result<()> {
    println!("📄 Logs for run {}", workflow_run_id_to_string(&run.id));
    println!("📋 Workflow: {}", run.workflow.name);

    // Parse level filter
    let level_filter = if let Some(level_str) = level {
        match LogLevel::from_str(level_str) {
            Some(level) => {
                println!(
                    "🔍 Filtering by level: {} and above",
                    level_str.to_uppercase()
                );
                Some(level)
            }
            None => {
                println!("⚠️  Invalid log level '{}', showing all logs", level_str);
                None
            }
        }
    } else {
        None
    };

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

        let log_level = get_state_log_level(run, state_id);

        // Apply level filter
        if let Some(ref filter_level) = level_filter {
            if log_level < *filter_level {
                continue;
            }
        }

        println!(
            "{} {} Transitioned to: {} - {}",
            timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            log_level.emoji(),
            state_id,
            state_desc
        );
    }

    // Show current context/variables (always shown regardless of level)
    if !run.context.workflow_vars().is_empty() {
        println!("\n🔧 Current Variables:");
        for (key, value) in run.context.iter() {
            println!("  {key} = {value}");
        }
    }

    Ok(())
}

/// Create local workflow run storage backend
pub fn create_local_workflow_run_storage() -> Result<Box<dyn WorkflowRunStorageBackend>> {
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

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(LogLevel::from_str("debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("invalid"), None);
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }
}
