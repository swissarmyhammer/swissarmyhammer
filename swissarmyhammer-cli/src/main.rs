use std::process;
mod cli;
mod dynamic_cli;
mod error;
mod exit_codes;
mod logging;
mod mcp_integration;
mod schema_conversion;
mod schema_validation;
mod signal_handler;
use dynamic_cli::CliBuilder;
use exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use logging::FileWriterGuard;
use mcp_integration::CliToolContext;
use std::sync::Arc;

#[tokio::main]
async fn main() {
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

    // Check if this is a serve command early to bypass heavy CLI operations during MCP mode
    let is_serve_command = std::env::args().any(|arg| arg == "serve");

    // For serve command, skip all CLI building and validation to avoid startup delays
    if is_serve_command {
        let exit_code = run_server().await;
        process::exit(exit_code);
    }

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
            eprintln!("{}", e);
            process::exit(EXIT_ERROR);
        }
    };

    // Handle dynamic command dispatch
    let exit_code = handle_dynamic_matches(matches, cli_tool_context).await;
    process::exit(exit_code);
}

#[cfg(feature = "dynamic-cli")]
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

#[cfg(feature = "dynamic-cli")]
async fn handle_dynamic_matches(
    matches: clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
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
        Some(("serve", _sub_matches)) => {
            tracing::debug!("Starting MCP server");
            run_server().await
        }
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

async fn run_server() -> i32 {
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;
    use swissarmyhammer::PromptLibrary;
    use swissarmyhammer_tools::McpServer;

    // Create library and server
    let library = PromptLibrary::new();
    let server = match McpServer::new(library) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("Failed to create MCP server: {}", e);
            return EXIT_ERROR;
        }
    };

    // Initialize the server before starting
    if let Err(e) = server.initialize().await {
        eprintln!("Failed to initialize MCP server: {}", e);
        return EXIT_ERROR;
    }

    // Handle server startup and shutdown
    match serve_server(server, stdio()).await {
        Ok(_) => {
            tracing::info!("MCP server completed successfully");
            EXIT_SUCCESS
        }
        Err(e) => {
            eprintln!("Server error: {}", e);
            EXIT_ERROR
        }
    }
}

#[cfg(feature = "dynamic-cli")]
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

#[cfg(feature = "dynamic-cli")]
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

#[cfg(feature = "dynamic-cli")]
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

#[cfg(feature = "dynamic-cli")]
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
