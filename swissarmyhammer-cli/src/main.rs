use std::process;
mod cli;
mod commands;
mod dynamic_cli;
mod error;
mod exit_codes;
mod logging;
mod mcp_integration;
mod parameter_cli;
mod schema_conversion;
mod schema_validation;
mod signal_handler;
mod validate;
use dynamic_cli::CliBuilder;
use exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use logging::FileWriterGuard;
use mcp_integration::CliToolContext;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;

/// Load configuration for CLI usage with graceful error handling
///
/// This function loads configuration from all standard sources (global, project, environment)
/// and handles errors gracefully to ensure the CLI remains functional even with invalid config.
fn load_cli_configuration() -> TemplateContext {
    match swissarmyhammer_config::load_configuration_for_cli() {
        Ok(context) => {
            tracing::debug!("Loaded configuration with {} variables", context.len());
            context
        }
        Err(e) => {
            // Log the error but don't fail the CLI - configuration is optional for many operations
            tracing::warn!("Failed to load configuration: {}", e);
            eprintln!("Warning: Configuration loading failed: {}", e);
            eprintln!("Continuing with default configuration...");
            TemplateContext::new()
        }
    }
}

#[tokio::main]
async fn main() {
    // Load configuration early for CLI operations
    let template_context = load_cli_configuration();

    // Initialize tool context and registry for dynamic CLI
    let cli_tool_context = match CliToolContext::new().await {
        Ok(context) => Arc::new(context),
        Err(e) => {
            eprintln!("Failed to initialize tool context: {}", e);
            process::exit(EXIT_ERROR);
        }
    };

    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry);

    // Get validation statistics for startup reporting (only for non-serve commands)
    let validation_stats = cli_builder.get_validation_stats();

    // Check for validation issues and report them
    if !validation_stats.is_all_valid() {
        // Always show validation summary for issues (not just in verbose mode)
        eprintln!("⚠️  CLI Validation Issues: {}", validation_stats.summary());

        // Show detailed warnings if there are validation problems
        let warnings = cli_builder.get_validation_warnings();
        if !warnings.is_empty() {
            eprintln!("Validation warnings ({} issues):", warnings.len());
            for (i, warning) in warnings.iter().enumerate().take(5) {
                eprintln!("  {}. {}", i + 1, warning);
            }
            if warnings.len() > 5 {
                eprintln!("  ... and {} more warnings", warnings.len() - 5);
                eprintln!("  Use --verbose for complete validation report");
            }
        }
        eprintln!(); // Add blank line for readability
    }

    // Build CLI with warnings for validation issues (graceful degradation)
    // This will skip problematic tools but continue building the CLI
    let dynamic_cli = cli_builder.build_cli_with_warnings();

    // Parse arguments with dynamic CLI
    let matches = match dynamic_cli.try_get_matches() {
        Ok(matches) => matches,
        Err(e) => {
            // Check if this is a help or version request (which are normal exits)
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    // Print the help/version output
                    print!("{}", e);
                    process::exit(EXIT_SUCCESS);
                }
                _ => {
                    eprintln!("{}", e);
                    process::exit(EXIT_ERROR);
                }
            }
        }
    };

    // Handle dynamic command dispatch
    let exit_code = handle_dynamic_matches(matches, cli_tool_context, template_context).await;
    process::exit(exit_code);
}

async fn handle_tool_validation(cli_tool_context: Arc<CliToolContext>, verbose: bool) -> i32 {
    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry.clone());

    println!("🔍 Validating MCP tool schemas for CLI compatibility...\n");

    let validation_stats = cli_builder.get_validation_stats();
    let validation_errors = cli_builder.validate_all_tools();

    // Always show validation summary
    println!("📊 Validation Summary:");
    println!("   {}", validation_stats.summary());
    println!();

    if validation_stats.is_all_valid() {
        println!("✅ All tools passed validation!");
        if verbose {
            let categories = tool_registry.get_cli_categories();
            println!("\n📋 Validated CLI categories ({}):", categories.len());
            for category in categories {
                let tools = tool_registry.get_tools_for_category(&category);
                println!("   {} - {} tools", category, tools.len());
                if verbose {
                    for tool in tools {
                        println!("     ├── {} ({})", tool.cli_name(), tool.name());
                    }
                }
            }
        }
        return EXIT_SUCCESS;
    }

    // Show validation errors
    println!("❌ Validation Issues Found:");

    if verbose {
        for (i, error) in validation_errors.iter().enumerate() {
            println!("{}. {}", i + 1, error);
            if let Some(suggestion) = error.suggestion() {
                println!("   💡 {}", suggestion);
            }
            println!();
        }
    } else {
        let warnings = cli_builder.get_validation_warnings();
        for (i, warning) in warnings.iter().enumerate().take(10) {
            println!("{}. {}", i + 1, warning);
        }
        if warnings.len() > 10 {
            println!("   ... and {} more issues", warnings.len() - 10);
            println!("   Use --verbose for complete details");
        }
    }

    println!("🔧 To fix these issues:");
    println!("   • Review tool schema definitions");
    println!("   • Ensure all CLI tools have proper categories");
    println!("   • Use supported parameter types (string, integer, number, boolean, array)");
    println!("   • Add required schema fields like 'properties'");

    EXIT_WARNING
}

async fn handle_dynamic_matches(
    matches: clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
    template_context: TemplateContext,
) -> i32 {
    // Handle global verbose/debug/quiet flags
    let verbose = matches.get_flag("verbose");
    let debug = matches.get_flag("debug");
    let quiet = matches.get_flag("quiet");
    let validate_tools = matches.get_flag("validate-tools");

    // Check if this is a serve command for MCP mode logging
    let is_serve_command = matches
        .subcommand()
        .is_some_and(|(name, _)| name == "serve");

    // Initialize logging similar to static CLI
    configure_logging(verbose, debug, quiet, is_serve_command).await;

    // Handle --validate-tools flag
    if validate_tools {
        return handle_tool_validation(cli_tool_context, verbose).await;
    }

    // Show detailed validation report in verbose mode (but not during serve mode)
    if verbose && !is_serve_command {
        let tool_registry = cli_tool_context.get_tool_registry_arc();
        let cli_builder = CliBuilder::new(tool_registry);
        let validation_stats = cli_builder.get_validation_stats();

        if verbose {
            eprintln!("🔍 CLI Tool Validation Report:");
            eprintln!("   {}", validation_stats.summary());

            if !validation_stats.is_all_valid() {
                eprintln!("   Tools with issues:");
                let warnings = cli_builder.get_validation_warnings();
                for (i, warning) in warnings.iter().enumerate() {
                    eprintln!("     {}. {}", i + 1, warning);
                }
            }
            eprintln!(); // Add blank line
        }
    }

    // Handle subcommands
    match matches.subcommand() {
        Some(("serve", sub_matches)) => handle_serve_command(sub_matches, &template_context).await,
        Some(("doctor", sub_matches)) => {
            handle_doctor_command(sub_matches, &template_context).await
        }
        Some(("prompt", sub_matches)) => {
            handle_prompt_command(sub_matches, &template_context).await
        }
        Some(("flow", sub_matches)) => handle_flow_command(sub_matches, &template_context).await,
        Some(("validate", sub_matches)) => {
            handle_validate_command(sub_matches, &template_context).await
        }
        Some(("plan", sub_matches)) => handle_plan_command(sub_matches, &template_context).await,
        Some(("implement", _sub_matches)) => handle_implement_command(&template_context).await,
        Some((category, sub_matches)) => match sub_matches.subcommand() {
            Some((tool_name, tool_matches)) => {
                handle_dynamic_tool_command(category, tool_name, tool_matches, cli_tool_context)
                    .await
            }
            None => {
                eprintln!("No command specified for category '{}'", category);
                EXIT_ERROR
            }
        },
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            EXIT_ERROR
        }
    }
}

async fn handle_serve_command(
    matches: &clap::ArgMatches,
    template_context: &TemplateContext,
) -> i32 {
    commands::serve::handle_command(matches, template_context).await
}

async fn handle_dynamic_tool_command(
    category: &str,
    tool_name: &str,
    matches: &clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    // Look up the tool by category and CLI name
    let tool = match cli_tool_context
        .get_tool_registry()
        .get_tool_by_cli_name(category, tool_name)
    {
        Some(tool) => tool,
        None => {
            let available_tools: Vec<String> = cli_tool_context
                .get_tool_registry()
                .get_tools_for_category(category)
                .iter()
                .map(|t| format!("{} -> {}", t.cli_name(), t.name()))
                .collect();
            eprintln!(
                "Tool '{}' not found in category '{}'. Available tools in this category: [{}]",
                tool_name,
                category,
                available_tools.join(", ")
            );
            return EXIT_ERROR;
        }
    };

    let full_tool_name = tool.name();

    // Convert clap matches to JSON arguments
    let arguments =
        match convert_matches_to_arguments(matches, full_tool_name, &cli_tool_context).await {
            Ok(args) => args,
            Err(e) => {
                eprintln!("Error processing arguments: {}", e);
                return EXIT_ERROR;
            }
        };

    // Execute the MCP tool
    match cli_tool_context
        .execute_tool(full_tool_name, arguments)
        .await
    {
        Ok(result) => {
            // Format and display the result
            if result.is_error.unwrap_or(false) {
                eprintln!(
                    "{}",
                    mcp_integration::response_formatting::format_error_response(&result)
                );
                EXIT_ERROR
            } else {
                println!(
                    "{}",
                    mcp_integration::response_formatting::format_success_response(&result)
                );
                EXIT_SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Tool execution error: {}", e);
            EXIT_ERROR
        }
    }
}

async fn convert_matches_to_arguments(
    matches: &clap::ArgMatches,
    tool_name: &str,
    cli_tool_context: &CliToolContext,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut arguments = serde_json::Map::new();

    // Get the tool to access its schema
    let tool = cli_tool_context
        .get_tool_registry()
        .get_tool(tool_name)
        .ok_or_else(|| format!("Tool not found: {}", tool_name))?;

    let schema = tool.schema();

    // Extract properties from schema
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties {
            if let Some(value) = extract_clap_value(matches, prop_name, prop_schema) {
                arguments.insert(prop_name.clone(), value);
            }
        }
    }

    Ok(arguments)
}

fn extract_clap_value(
    matches: &clap::ArgMatches,
    prop_name: &str,
    prop_schema: &serde_json::Value,
) -> Option<serde_json::Value> {
    match prop_schema.get("type").and_then(|t| t.as_str()) {
        Some("boolean") => {
            if matches.get_flag(prop_name) {
                Some(serde_json::Value::Bool(true))
            } else {
                None
            }
        }
        Some("integer") => matches
            .get_one::<i64>(prop_name)
            .map(|v| serde_json::Value::Number(serde_json::Number::from(*v))),
        Some("number") => matches
            .get_one::<f64>(prop_name)
            .and_then(|v| serde_json::Number::from_f64(*v))
            .map(serde_json::Value::Number),
        Some("array") => {
            let values: Vec<String> = matches
                .get_many::<String>(prop_name)
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            if values.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(
                    values.into_iter().map(serde_json::Value::String).collect(),
                ))
            }
        }
        _ => {
            // Default to string
            matches
                .get_one::<String>(prop_name)
                .map(|s| serde_json::Value::String(s.clone()))
        }
    }
}

async fn handle_doctor_command(
    matches: &clap::ArgMatches,
    template_context: &TemplateContext,
) -> i32 {
    let migration = matches.get_flag("migration");
    commands::doctor::handle_command(migration, template_context).await
}

async fn handle_prompt_command(
    matches: &clap::ArgMatches,
    template_context: &TemplateContext,
) -> i32 {
    use crate::cli::{OutputFormat, PromptSourceArg, PromptSubcommand};

    let subcommand = match matches.subcommand() {
        Some(("list", sub_matches)) => {
            let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
                Some("json") => OutputFormat::Json,
                Some("yaml") => OutputFormat::Yaml,
                _ => OutputFormat::Table,
            };
            let verbose = sub_matches.get_flag("verbose");
            let source = sub_matches
                .get_one::<String>("source")
                .map(|s| match s.as_str() {
                    "builtin" => PromptSourceArg::Builtin,
                    "user" => PromptSourceArg::User,
                    "local" => PromptSourceArg::Local,
                    "dynamic" => PromptSourceArg::Dynamic,
                    _ => PromptSourceArg::Dynamic,
                });
            let category = sub_matches.get_one::<String>("category").cloned();
            let search = sub_matches.get_one::<String>("search").cloned();

            PromptSubcommand::List {
                format,
                verbose,
                source,
                category,
                search,
            }
        }
        Some(("test", sub_matches)) => {
            let prompt_name = sub_matches.get_one::<String>("prompt_name").cloned();
            let file = sub_matches.get_one::<String>("file").cloned();
            let vars = sub_matches
                .get_many::<String>("vars")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            let raw = sub_matches.get_flag("raw");
            let copy = sub_matches.get_flag("copy");
            let save = sub_matches.get_one::<String>("save").cloned();
            let debug = sub_matches.get_flag("debug");

            PromptSubcommand::Test {
                prompt_name,
                file,
                vars,
                raw,
                copy,
                save,
                debug,
            }
        }
        Some(("search", sub_matches)) => {
            let query = sub_matches.get_one::<String>("query").cloned().unwrap();
            let r#in = sub_matches
                .get_many::<String>("in")
                .map(|vals| vals.cloned().collect());
            let regex = sub_matches.get_flag("regex");
            let fuzzy = sub_matches.get_flag("fuzzy");
            let case_sensitive = sub_matches.get_flag("case-sensitive");
            let source = sub_matches
                .get_one::<String>("source")
                .map(|s| match s.as_str() {
                    "builtin" => PromptSourceArg::Builtin,
                    "user" => PromptSourceArg::User,
                    "local" => PromptSourceArg::Local,
                    "dynamic" => PromptSourceArg::Dynamic,
                    _ => PromptSourceArg::Dynamic,
                });
            let has_arg = sub_matches.get_one::<String>("has-arg").cloned();
            let no_args = sub_matches.get_flag("no-args");
            let full = sub_matches.get_flag("full");
            let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
                Some("json") => OutputFormat::Json,
                Some("yaml") => OutputFormat::Yaml,
                _ => OutputFormat::Table,
            };
            let highlight = sub_matches.get_flag("highlight");
            let limit = sub_matches.get_one::<usize>("limit").copied();

            PromptSubcommand::Search {
                query,
                r#in,
                regex,
                fuzzy,
                case_sensitive,
                source,
                has_arg,
                no_args,
                full,
                format,
                highlight,
                limit,
            }
        }
        _ => {
            eprintln!("No prompt subcommand specified");
            return EXIT_ERROR;
        }
    };

    commands::prompt::handle_command(subcommand, template_context).await
}

async fn handle_flow_command(
    matches: &clap::ArgMatches,
    template_context: &TemplateContext,
) -> i32 {
    use crate::cli::{FlowSubcommand, OutputFormat, PromptSourceArg, VisualizationFormat};

    let subcommand = match matches.subcommand() {
        Some(("run", sub_matches)) => {
            let workflow = sub_matches.get_one::<String>("workflow").cloned().unwrap();
            let vars = sub_matches
                .get_many::<String>("vars")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            let interactive = sub_matches.get_flag("interactive");
            let dry_run = sub_matches.get_flag("dry-run");
            let test = sub_matches.get_flag("test");
            let timeout = sub_matches.get_one::<String>("timeout").cloned();
            let quiet = sub_matches.get_flag("quiet");

            FlowSubcommand::Run {
                workflow,
                vars,
                interactive,
                dry_run,
                test,
                timeout,
                quiet,
            }
        }
        Some(("resume", sub_matches)) => {
            let run_id = sub_matches.get_one::<String>("run_id").cloned().unwrap();
            let interactive = sub_matches.get_flag("interactive");
            let timeout = sub_matches.get_one::<String>("timeout").cloned();
            let quiet = sub_matches.get_flag("quiet");

            FlowSubcommand::Resume {
                run_id,
                interactive,
                timeout,
                quiet,
            }
        }
        Some(("list", sub_matches)) => {
            let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
                Some("json") => OutputFormat::Json,
                Some("yaml") => OutputFormat::Yaml,
                _ => OutputFormat::Table,
            };
            let verbose = sub_matches.get_flag("verbose");
            let source = sub_matches
                .get_one::<String>("source")
                .map(|s| match s.as_str() {
                    "builtin" => PromptSourceArg::Builtin,
                    "user" => PromptSourceArg::User,
                    "local" => PromptSourceArg::Local,
                    "dynamic" => PromptSourceArg::Dynamic,
                    _ => PromptSourceArg::Dynamic,
                });

            FlowSubcommand::List {
                format,
                verbose,
                source,
            }
        }
        Some(("status", sub_matches)) => {
            let run_id = sub_matches.get_one::<String>("run_id").cloned().unwrap();
            let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
                Some("json") => OutputFormat::Json,
                Some("yaml") => OutputFormat::Yaml,
                _ => OutputFormat::Table,
            };
            let watch = sub_matches.get_flag("watch");

            FlowSubcommand::Status {
                run_id,
                format,
                watch,
            }
        }
        Some(("logs", sub_matches)) => {
            let run_id = sub_matches.get_one::<String>("run_id").cloned().unwrap();
            let follow = sub_matches.get_flag("follow");
            let tail = sub_matches.get_one::<usize>("tail").copied();
            let level = sub_matches.get_one::<String>("level").cloned();

            FlowSubcommand::Logs {
                run_id,
                follow,
                tail,
                level,
            }
        }
        Some(("metrics", sub_matches)) => {
            let run_id = sub_matches.get_one::<String>("run_id").cloned();
            let workflow = sub_matches.get_one::<String>("workflow").cloned();
            let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
                Some("json") => OutputFormat::Json,
                Some("yaml") => OutputFormat::Yaml,
                _ => OutputFormat::Table,
            };
            let global = sub_matches.get_flag("global");

            FlowSubcommand::Metrics {
                run_id,
                workflow,
                format,
                global,
            }
        }
        Some(("visualize", sub_matches)) => {
            let run_id = sub_matches.get_one::<String>("run_id").cloned().unwrap();
            let format = match sub_matches.get_one::<String>("format").map(|s| s.as_str()) {
                Some("html") => VisualizationFormat::Html,
                Some("json") => VisualizationFormat::Json,
                Some("dot") => VisualizationFormat::Dot,
                _ => VisualizationFormat::Mermaid,
            };
            let output = sub_matches.get_one::<String>("output").cloned();
            let timing = sub_matches.get_flag("timing");
            let counts = sub_matches.get_flag("counts");
            let path_only = sub_matches.get_flag("path-only");

            FlowSubcommand::Visualize {
                run_id,
                format,
                output,
                timing,
                counts,
                path_only,
            }
        }
        Some(("test", sub_matches)) => {
            let workflow = sub_matches.get_one::<String>("workflow").cloned().unwrap();
            let vars = sub_matches
                .get_many::<String>("vars")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            let interactive = sub_matches.get_flag("interactive");
            let timeout = sub_matches.get_one::<String>("timeout").cloned();
            let quiet = sub_matches.get_flag("quiet");

            FlowSubcommand::Test {
                workflow,
                vars,
                interactive,
                timeout,
                quiet,
            }
        }
        _ => {
            eprintln!("No flow subcommand specified");
            return EXIT_ERROR;
        }
    };

    commands::flow::handle_command(subcommand, template_context).await
}

async fn handle_validate_command(
    matches: &clap::ArgMatches,
    template_context: &TemplateContext,
) -> i32 {
    use crate::cli::ValidateFormat;

    let quiet = matches.get_flag("quiet");
    let format = match matches.get_one::<String>("format").map(|s| s.as_str()) {
        Some("json") => ValidateFormat::Json,
        _ => ValidateFormat::Text,
    };
    let workflow_dirs = matches
        .get_many::<String>("workflow-dirs")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    let validate_tools = matches.get_flag("validate-tools");

    commands::validate::handle_command(
        quiet,
        format,
        workflow_dirs,
        validate_tools,
        template_context,
    )
    .await
}

async fn handle_plan_command(
    matches: &clap::ArgMatches,
    template_context: &TemplateContext,
) -> i32 {
    let plan_filename = matches.get_one::<String>("plan_filename").cloned().unwrap();
    commands::plan::handle_command(plan_filename, template_context).await
}

async fn handle_implement_command(template_context: &TemplateContext) -> i32 {
    commands::implement::handle_command(template_context).await
}

async fn configure_logging(verbose: bool, debug: bool, quiet: bool, is_mcp_mode: bool) {
    use tracing::Level;
    use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

    let log_level = if is_mcp_mode {
        Level::DEBUG // More verbose for MCP mode to help with debugging
    } else if quiet {
        Level::ERROR
    } else if debug {
        Level::DEBUG
    } else if verbose {
        Level::TRACE
    } else {
        Level::INFO
    };

    let create_filter = || EnvFilter::new(format!("ort=warn,rmcp=warn,{log_level}"));

    if is_mcp_mode {
        // In MCP mode, write logs to .swissarmyhammer/mcp.log for debugging
        use std::fs;
        use std::path::PathBuf;

        let log_dir = PathBuf::from(".swissarmyhammer");
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Warning: Could not create log directory: {}", e);
        }

        let log_file_name =
            std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
        let log_file_path = log_dir.join(log_file_name);
        match fs::File::create(&log_file_path) {
            Ok(file) => {
                let shared_file = Arc::new(std::sync::Mutex::new(file));
                registry()
                    .with(create_filter())
                    .with(
                        fmt::layer()
                            .with_writer(move || {
                                let file = shared_file.clone();
                                Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
                            })
                            .with_ansi(false), // No color codes in file
                    )
                    .init();
            }
            Err(e) => {
                eprintln!(
                    "Warning: Could not create log file: {}. Falling back to stderr.",
                    e
                );
                registry()
                    .with(create_filter())
                    .with(fmt::layer().with_writer(std::io::stderr))
                    .init();
            }
        }
    } else {
        registry()
            .with(create_filter())
            .with(fmt::layer().with_writer(std::io::stderr))
            .init();
    }
}
