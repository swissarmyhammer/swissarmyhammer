//! Flow command implementation
//!
//! Executes and manages workflows with support for starting new runs and resuming existing ones

pub mod display;
pub mod list;
pub mod params;
pub mod run;

use crate::cli::{FlowSubcommand, OutputFormat, PromptSourceArg};
use crate::context::CliContext;
use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use anyhow::{anyhow, Result};
use std::sync::Arc;

/// Help text for the flow command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Helper function to extract the next argument value for a flag
fn next_arg_value<'a>(args: &'a [String], i: &mut usize, flag: &str) -> Result<&'a str> {
    *i += 1;
    if *i < args.len() {
        Ok(&args[*i])
    } else {
        Err(anyhow!("Missing value for flag: {}", flag))
    }
}

/// Helper function to parse OutputFormat from a string
fn parse_output_format(value: &str) -> Result<OutputFormat> {
    match value {
        "json" => Ok(OutputFormat::Json),
        "yaml" => Ok(OutputFormat::Yaml),
        "table" => Ok(OutputFormat::Table),
        _ => Err(anyhow!("Invalid format: {}", value)),
    }
}

/// Helper function to parse PromptSourceArg from a string
fn parse_prompt_source(value: &str) -> Result<PromptSourceArg> {
    match value {
        "builtin" => Ok(PromptSourceArg::Builtin),
        "user" => Ok(PromptSourceArg::User),
        "local" => Ok(PromptSourceArg::Local),
        "dynamic" => Ok(PromptSourceArg::Dynamic),
        _ => Err(anyhow!("Invalid source: {}", value)),
    }
}

/// Parse the "list" subcommand arguments
fn parse_list_command(args: &[String]) -> Result<FlowSubcommand> {
    let mut verbose = false;
    let mut format = OutputFormat::Table;
    let mut source = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--verbose" | "-v" => verbose = true,
            "--format" => {
                let value = next_arg_value(args, &mut i, "--format")?;
                format = parse_output_format(value)?;
            }
            "--source" => {
                let value = next_arg_value(args, &mut i, "--source")?;
                source = Some(parse_prompt_source(value)?);
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

/// Parse workflow execution command arguments
fn parse_execute_command(args: &[String]) -> Result<FlowSubcommand> {
    let workflow = args[0].clone();
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
            match arg.as_str() {
                "--param" | "-p" => {
                    let value = next_arg_value(args, &mut i, arg)?;
                    params.push(value.to_string());
                }
                "--var" => {
                    let value = next_arg_value(args, &mut i, "--var")?;
                    vars.push(value.to_string());
                }
                "--interactive" | "-i" => interactive = true,
                "--dry-run" => dry_run = true,
                "--quiet" | "-q" => quiet = true,
                _ => return Err(anyhow!("Unknown flag for workflow execution: {}", arg)),
            }
        } else {
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

    match first_arg.as_str() {
        "list" => parse_list_command(&args),
        _ => parse_execute_command(&args),
    }
}

/// Handle the flow command - PURE ROUTING ONLY
pub async fn handle_command(
    subcommand: FlowSubcommand,
    context: &CliContext,
    cli_tool_context: Arc<crate::mcp_integration::CliToolContext>,
) -> i32 {
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
                cli_tool_context,
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
