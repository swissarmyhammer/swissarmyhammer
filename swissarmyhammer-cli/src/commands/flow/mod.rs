//! Flow command implementation
//!
//! Executes and manages workflows with support for starting new runs and resuming existing ones

pub mod display;
pub mod list;
pub mod logs;

pub mod resume;
pub mod run;
pub mod shared;
pub mod status;
pub mod test;


use crate::cli::FlowSubcommand;
use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the flow command - PURE ROUTING ONLY
pub async fn handle_command(subcommand: FlowSubcommand, context: &CliContext) -> i32 {
    let result = match subcommand {
        FlowSubcommand::Run {
            workflow,
            vars,
            interactive,
            dry_run,
            timeout,
            quiet,
        } => {
            run::execute_run_command(
                workflow,
                vars,
                interactive,
                dry_run,
                timeout,
                quiet,
                context,
            )
            .await
        }
        FlowSubcommand::Resume {
            run_id,
            interactive,
            timeout,
            quiet,
        } => resume::execute_resume_command(run_id, interactive, timeout, quiet).await,
        FlowSubcommand::List {
            format,
            verbose,
            source,
        } => list::execute_list_command(format, verbose, source, context).await,
        FlowSubcommand::Status {
            run_id,
            format,
            watch,
        } => status::execute_status_command(run_id, format, watch, context).await,
        FlowSubcommand::Logs {
            run_id,
            follow,
            tail,
            level,
        } => logs::execute_logs_command(run_id, follow, tail, level).await,


        FlowSubcommand::Test {
            workflow,
            vars,
            interactive,
            timeout,
            quiet,
        } => test::execute_test_command(workflow, vars, interactive, timeout, quiet, context).await,
    };

    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Flow command failed: {}", e);
            EXIT_ERROR
        }
    }
}

// NO business logic here - only routing and error handling
