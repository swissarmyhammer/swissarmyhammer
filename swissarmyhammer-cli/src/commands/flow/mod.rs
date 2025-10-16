//! Flow command implementation
//!
//! Executes and manages workflows with support for starting new runs and resuming existing ones

pub mod display;
pub mod list;
pub mod params;
pub mod run;
pub mod shared;

use crate::cli::{FlowSubcommand, OutputFormat, PromptSourceArg};
use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use anyhow::{anyhow, Result};

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Parse flow command arguments into a FlowSubcommand
///
/// This function examines the first argument to determine if it's the special "list" command
/// or a workflow name for execution.
///
/// For workflow execution: `sah flow <workflow> [args...] [--param k=v]`
/// For listing workflows: `sah flow list [--verbose]`
pub fn parse_flow_args(args: Vec<String>) -> Result<FlowSubcommand> {
    if args.is_empty() {
        return Err(anyhow!(
            "No workflow or command specified. Use 'sah flow list' to see available workflows."
        ));
    }

    let first_arg = &args[0];

    // Check if first arg is the special "list" command
    match first_arg.as_str() {
        "list" => {
            // Parse list command: flow list [--verbose] [--format FORMAT] [--source SOURCE]
            let mut verbose = false;
            let mut format = OutputFormat::Table;
            let mut source = None;

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--verbose" | "-v" => verbose = true,
                    "--format" => {
                        i += 1;
                        if i < args.len() {
                            format = match args[i].as_str() {
                                "json" => OutputFormat::Json,
                                "yaml" => OutputFormat::Yaml,
                                "table" => OutputFormat::Table,
                                _ => return Err(anyhow!("Invalid format: {}", args[i])),
                            };
                        }
                    }
                    "--source" => {
                        i += 1;
                        if i < args.len() {
                            source = Some(match args[i].as_str() {
                                "builtin" => PromptSourceArg::Builtin,
                                "user" => PromptSourceArg::User,
                                "local" => PromptSourceArg::Local,
                                "dynamic" => PromptSourceArg::Dynamic,
                                _ => return Err(anyhow!("Invalid source: {}", args[i])),
                            });
                        }
                    }
                    _ => return Err(anyhow!("Unknown flag for list command: {}", args[i])),
                }
                i += 1;
            }

            Ok(FlowSubcommand::List {
                format,
                verbose,
                source,
            })
        }

        _ => {
            // Not a special command, treat as workflow execution
            // Parse: flow <workflow> [positional_args...] [--param KEY=VALUE]... [--var KEY=VALUE]... [flags]
            let workflow = first_arg.clone();
            let mut positional_args = Vec::new();
            let mut params = Vec::new();
            let mut vars = Vec::new();
            let mut interactive = false;
            let mut dry_run = false;
            let mut quiet = false;

            let mut i = 1;
            while i < args.len() {
                let arg = &args[i];

                if arg.starts_with("--") || arg.starts_with("-") {
                    // It's a flag
                    match arg.as_str() {
                        "--param" | "-p" => {
                            i += 1;
                            if i < args.len() {
                                params.push(args[i].clone());
                            }
                        }
                        "--var" => {
                            i += 1;
                            if i < args.len() {
                                vars.push(args[i].clone());
                            }
                        }
                        "--interactive" | "-i" => interactive = true,
                        "--dry-run" => dry_run = true,
                        "--quiet" | "-q" => quiet = true,
                        _ => return Err(anyhow!("Unknown flag for workflow execution: {}", arg)),
                    }
                } else {
                    // It's a positional argument
                    positional_args.push(arg.clone());
                }

                i += 1;
            }

            Ok(FlowSubcommand::Execute {
                workflow,
                positional_args,
                params,
                vars,
                interactive,
                dry_run,
                quiet,
            })
        }
    }
}

/// Handle the flow command - PURE ROUTING ONLY
pub async fn handle_command(subcommand: FlowSubcommand, context: &CliContext) -> i32 {
    let result = match subcommand {
        FlowSubcommand::Execute {
            workflow,
            positional_args,
            params,
            vars,
            interactive,
            dry_run,
            quiet,
        } => {
            // Show deprecation warning if --var is used
            if !vars.is_empty() {
                eprintln!("Warning: --var is deprecated, use --param instead");
            }

            run::execute_run_command(
                run::RunCommandConfig {
                    workflow,
                    positional_args,
                    params,
                    vars,
                    interactive,
                    dry_run,
                    quiet,
                },
                context,
            )
            .await
        }
        FlowSubcommand::List {
            format,
            verbose,
            source,
        } => list::execute_list_command(format, verbose, source, context).await,
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
