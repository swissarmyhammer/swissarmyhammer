//! Flow command implementation
//!
//! Executes and manages workflows with support for starting new runs and resuming existing ones

pub mod display;
pub mod list;
pub mod logs;
pub mod params;
pub mod resume;
pub mod run;
pub mod shared;
pub mod status;
pub mod test;

use crate::cli::{FlowSubcommand, OutputFormat, PromptSourceArg};
use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use anyhow::{anyhow, Result};

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Parse flow command arguments into a FlowSubcommand
///
/// This function examines the first argument to determine if it's a special command
/// (list, resume, status, logs, test) or a workflow name.
///
/// For workflow execution: `sah flow <workflow> [args...] [--param k=v]`
/// For special commands: `sah flow list [--verbose]`, etc.
pub fn parse_flow_args(args: Vec<String>) -> Result<FlowSubcommand> {
    if args.is_empty() {
        return Err(anyhow!(
            "No workflow or command specified. Use 'sah flow list' to see available workflows."
        ));
    }

    let first_arg = &args[0];

    // Check if first arg is a special command
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

        "resume" => {
            // Parse resume command: flow resume <run_id> [--interactive] [--quiet]
            if args.len() < 2 {
                return Err(anyhow!("Resume command requires a run ID"));
            }

            let run_id = args[1].clone();
            let mut interactive = false;
            let mut quiet = false;

            for arg in &args[2..] {
                match arg.as_str() {
                    "--interactive" | "-i" => interactive = true,
                    "--quiet" | "-q" => quiet = true,
                    _ => return Err(anyhow!("Unknown flag for resume command: {}", arg)),
                }
            }

            Ok(FlowSubcommand::Resume {
                run_id,
                interactive,
                quiet,
            })
        }

        "status" => {
            // Parse status command: flow status <run_id> [--format FORMAT] [--watch]
            if args.len() < 2 {
                return Err(anyhow!("Status command requires a run ID"));
            }

            let run_id = args[1].clone();
            let mut format = OutputFormat::Table;
            let mut watch = false;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--watch" | "-w" => watch = true,
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
                    _ => return Err(anyhow!("Unknown flag for status command: {}", args[i])),
                }
                i += 1;
            }

            Ok(FlowSubcommand::Status {
                run_id,
                format,
                watch,
            })
        }

        "logs" => {
            // Parse logs command: flow logs <run_id> [--follow] [--tail N] [--level LEVEL]
            if args.len() < 2 {
                return Err(anyhow!("Logs command requires a run ID"));
            }

            let run_id = args[1].clone();
            let mut follow = false;
            let mut tail = None;
            let mut level = None;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--follow" | "-f" => follow = true,
                    "--tail" | "-n" => {
                        i += 1;
                        if i < args.len() {
                            tail = Some(
                                args[i]
                                    .parse()
                                    .map_err(|_| anyhow!("Invalid tail value: {}", args[i]))?,
                            );
                        }
                    }
                    "--level" => {
                        i += 1;
                        if i < args.len() {
                            level = Some(args[i].clone());
                        }
                    }
                    _ => return Err(anyhow!("Unknown flag for logs command: {}", args[i])),
                }
                i += 1;
            }

            Ok(FlowSubcommand::Logs {
                run_id,
                follow,
                tail,
                level,
            })
        }

        "test" => {
            // Parse test command: flow test <workflow> [--var KEY=VALUE]... [--interactive] [--quiet]
            // Check for help flag first
            if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
                // Print help for test command to stdout and return an error to signal help was shown
                println!("Test workflow execution without actually running it\n");
                println!("Usage: sah flow test <WORKFLOW> [OPTIONS]\n");
                println!("Arguments:");
                println!("  <WORKFLOW>  Workflow name to test\n");
                println!("Options:");
                println!("  --var <KEY=VALUE>     Set workflow variable (deprecated, for backward compatibility)");
                println!("  -i, --interactive     Interactive mode - prompt at each state");
                println!("  -q, --quiet          Quiet mode - only show errors");
                println!("  -h, --help           Print help");
                // Return a special error that the caller can handle as "help displayed, exit 0"
                return Err(anyhow!("__HELP_DISPLAYED__"));
            }

            if args.len() < 2 {
                return Err(anyhow!("Test command requires a workflow name"));
            }

            let workflow = args[1].clone();
            let mut vars = Vec::new();
            let mut interactive = false;
            let mut quiet = false;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--var" => {
                        i += 1;
                        if i < args.len() {
                            vars.push(args[i].clone());
                        }
                    }
                    "--interactive" | "-i" => interactive = true,
                    "--quiet" | "-q" => quiet = true,
                    "--help" | "-h" => {
                        println!("Test workflow execution without actually running it\n");
                        println!("Usage: sah flow test <WORKFLOW> [OPTIONS]\n");
                        println!("Arguments:");
                        println!("  <WORKFLOW>  Workflow name to test\n");
                        println!("Options:");
                        println!("  --var <KEY=VALUE>     Set workflow variable (deprecated, for backward compatibility)");
                        println!("  -i, --interactive     Interactive mode - prompt at each state");
                        println!("  -q, --quiet          Quiet mode - only show errors");
                        println!("  -h, --help           Print help");
                        return Err(anyhow!("__HELP_DISPLAYED__"));
                    }
                    _ => return Err(anyhow!("Unknown flag for test command: {}", args[i])),
                }
                i += 1;
            }

            Ok(FlowSubcommand::Test {
                workflow,
                vars,
                interactive,
                quiet,
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
        FlowSubcommand::Resume {
            run_id,
            interactive,
            quiet,
        } => resume::execute_resume_command(run_id, interactive, quiet).await,
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
            quiet,
        } => test::execute_test_command(workflow, vars, interactive, quiet, context).await,
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
