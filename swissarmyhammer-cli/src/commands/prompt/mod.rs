//! Prompt command implementation
//!
//! Manages and tests prompts with support for listing, validating, testing, and searching

use crate::cli::PromptSubcommand;
use crate::error::{CliError, CliResult};
use crate::exit_codes::EXIT_SUCCESS;
use std::collections::HashMap;

use swissarmyhammer::{PromptFilter, PromptLibrary, PromptResolver};
use swissarmyhammer::common::{InteractivePrompts, Parameter, ParameterError, ParameterProvider, ParameterType};
use swissarmyhammer_config::TemplateContext;

/// Help text for the prompt command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the prompt command
pub async fn handle_command(
    subcommand: PromptSubcommand,
    template_context: &TemplateContext,
) -> i32 {
    match run_prompt_command(subcommand, template_context).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Prompt command failed: {}", e);
            e.exit_code
        }
    }
}

/// Main entry point for prompt command
async fn run_prompt_command(
    subcommand: PromptSubcommand,
    template_context: &TemplateContext,
) -> CliResult<()> {
    match subcommand {
        PromptSubcommand::List {
            format,
            verbose,
            source,
            category,
            search,
        } => run_list_command(format, verbose, source, category, search)
            .map_err(|e| CliError::new(e.to_string(), 1)),
        PromptSubcommand::Test {
            prompt_name,
            file,
            vars,
            raw,
            copy,
            save,
            debug,
        } => run_test_command(
            TestCommandConfig {
                prompt_name,
                _file: file,
                vars,
                _raw: raw,
                _copy: copy,
                _save: save,
                _debug: debug,
            },
            template_context,
        )
        .await
        .map_err(|e| CliError::new(e.to_string(), 1)),
        PromptSubcommand::Search { .. } => Err(CliError::new(
            "Search functionality has been removed as part of infrastructure cleanup".to_string(),
            1,
        )),
    }
}

/// Check if a prompt is a partial template that should not be displayed in the list.
///
/// Partial templates are identified by either:
/// 1. Starting with the `{% partial %}` marker
/// 2. Having a description containing "Partial template for reuse in other prompts"
fn is_partial_template(prompt: &swissarmyhammer::prompts::Prompt) -> bool {
    // Check if the template starts with the partial marker
    if prompt.template.trim().starts_with("{% partial %}") {
        return true;
    }

    // Check if the description indicates it's a partial template
    if let Some(description) = &prompt.description {
        if description.contains("Partial template for reuse in other prompts") {
            return true;
        }
    }

    false
}

/// Run the list command
fn run_list_command(
    format: crate::cli::OutputFormat,
    verbose: bool,
    source_filter: Option<crate::cli::PromptSourceArg>,
    category_filter: Option<String>,
    search_term: Option<String>,
) -> Result<(), anyhow::Error> {
    // Load all prompts from all sources
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)?;

    // Build the filter
    let mut filter = PromptFilter::new();

    if let Some(ref source) = source_filter {
        let lib_source: swissarmyhammer::PromptSource = source.clone().into();
        filter = filter.with_source(lib_source);
    }

    if let Some(ref category) = category_filter {
        filter = filter.with_category(category);
    }

    if let Some(ref term) = search_term {
        filter = filter.with_search_term(term);
    }

    // Apply filter and get prompts - pass empty file sources since we're using all sources
    let file_sources = HashMap::new();
    let all_prompts = library.list_filtered(&filter, &file_sources)?;

    // Filter out partial templates
    let prompts: Vec<_> = all_prompts
        .into_iter()
        .filter(|prompt| !is_partial_template(prompt))
        .collect();

    // Display results based on format
    match format {
        crate::cli::OutputFormat::Table => {
            println!("Available prompts:");
            for prompt in prompts {
                if verbose {
                    println!(
                        "  {} - {} ({})",
                        prompt.name,
                        prompt
                            .metadata
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No title"),
                        prompt
                            .metadata
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No description")
                    );
                } else {
                    println!("  {}", prompt.name);
                }
            }
        }
        crate::cli::OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&prompts)?;
            println!("{}", json);
        }
        crate::cli::OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&prompts)?;
            println!("{}", yaml);
        }
    }

    Ok(())
}

/// Configuration for the test command
struct TestCommandConfig {
    prompt_name: Option<String>,
    _file: Option<String>,
    vars: Vec<String>,
    _raw: bool,
    _copy: bool,
    _save: Option<String>,
    _debug: bool,
}

/// Run the test command
async fn run_test_command(
    config: TestCommandConfig,
    template_context: &TemplateContext,
) -> Result<(), anyhow::Error> {
    let prompt_name = config
        .prompt_name
        .ok_or_else(|| anyhow::anyhow!("Prompt name is required"))?;

    // Load all prompts
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)?;

    // Get the specific prompt to access its parameter definitions
    let prompt = library.get(&prompt_name)
        .map_err(|e| anyhow::anyhow!("Failed to get prompt '{}': {}", prompt_name, e))?;

    // Parse variables from command line arguments  
    let mut cli_arguments = HashMap::new();
    for var in config.vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            cli_arguments.insert(parts[0].to_string(), serde_json::Value::String(parts[1].to_string()));
        }
    }

    // Use InteractivePrompts to collect any missing parameters
    // For CLI testing, we want to be more interactive and prompt for optional parameters too
    // Force interactive mode for CLI testing by creating with explicit non_interactive=false
    let interactive_prompts = InteractivePrompts::with_max_attempts(false, 3);
    let all_arguments = prompt_for_all_missing_parameters(
        &interactive_prompts,
        prompt.get_parameters(),
        &cli_arguments
    ).map_err(|e| anyhow::anyhow!("Failed to collect parameters: {}", e))?;

    // Create a template context with CLI arguments having highest precedence
    let mut final_context = template_context.clone();
    for (key, value) in &all_arguments {
        final_context.set(key.clone(), value.clone());
    }

    // Render the prompt with the merged context
    // The library's render_prompt_with_env_and_context method will use both the context and arguments,
    // but since we've already merged CLI args into the context with highest precedence, we can pass empty arguments
    let empty_arguments = HashMap::new();
    let rendered = library.render_prompt_with_env_and_context(
        &prompt_name,
        &final_context,
        &empty_arguments,
    )?;
    println!("{}", rendered);

    Ok(())
}

/// Custom parameter collection for CLI testing that prompts for missing parameters
/// This uses simple stdin/stdout prompting for CLI testing
fn prompt_for_all_missing_parameters(
    _interactive_prompts: &InteractivePrompts,
    parameters: &[Parameter],
    existing_values: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, serde_json::Value>, ParameterError> {
    use std::io::{self, Write, IsTerminal};
    

    
    let mut resolved = existing_values.clone();
    
    // Check if we're in a terminal - if not, just use defaults
    if !io::stdin().is_terminal() {
        // Non-interactive mode - use defaults for optional parameters
        for param in parameters {
            if !resolved.contains_key(&param.name) {
                if let Some(default) = &param.default {
                    resolved.insert(param.name.clone(), default.clone());
                } else if param.required {
                    return Err(ParameterError::MissingRequired {
                        name: param.name.clone(),
                    });
                }
            }
        }
        return Ok(resolved);
    }
    
    for param in parameters {
        if resolved.contains_key(&param.name) {
            // Parameter already provided via CLI, skip
            continue;
        }
        
        // For CLI testing, prompt for parameters (both required and optional)
        // Show a simple prompt with default value if available
        let prompt_text = if let Some(default) = &param.default {
            let default_str = match default {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => default.to_string(),
            };
            format!("Enter {} ({}) [{}]: ", param.name, param.description, default_str)
        } else {
            format!("Enter {} ({}): ", param.name, param.description)
        };
        
        print!("{}", prompt_text);
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(|e| ParameterError::ValidationFailed {
            message: format!("Failed to read input: {}", e),
        })?;
        
        let input = input.trim();
        
        let value = if input.is_empty() {
            // Use default if available
            if let Some(default) = &param.default {
                default.clone()
            } else if param.required {
                return Err(ParameterError::MissingRequired {
                    name: param.name.clone(),
                });
            } else {
                // Optional parameter with no default - skip
                continue;
            }
        } else {
            // Convert input to appropriate type based on parameter type
            match param.parameter_type {
                ParameterType::String => serde_json::Value::String(input.to_string()),
                ParameterType::Boolean => {
                    let bool_val = input.to_lowercase();
                    match bool_val.as_str() {
                        "true" | "t" | "yes" | "y" | "1" => serde_json::Value::Bool(true),
                        "false" | "f" | "no" | "n" | "0" => serde_json::Value::Bool(false),
                        _ => return Err(ParameterError::ValidationFailed {
                            message: format!("Invalid boolean value: '{}'. Use true/false, yes/no, or 1/0.", input),
                        }),
                    }
                }
                ParameterType::Number => {
                    let num: f64 = input.parse().map_err(|_| ParameterError::ValidationFailed {
                        message: format!("Invalid number: '{}'", input),
                    })?;
                    serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap())
                }
                ParameterType::Choice => {
                    if let Some(choices) = &param.choices {
                        if choices.contains(&input.to_string()) {
                            serde_json::Value::String(input.to_string())
                        } else {
                            return Err(ParameterError::ValidationFailed {
                                message: format!("Invalid choice '{}'. Valid options: {}", input, choices.join(", ")),
                            });
                        }
                    } else {
                        serde_json::Value::String(input.to_string())
                    }
                }
                ParameterType::MultiChoice => {
                    // For simplicity, accept comma-separated values
                    let selected: Vec<String> = input.split(',').map(|s| s.trim().to_string()).collect();
                    if let Some(choices) = &param.choices {
                        for choice in &selected {
                            if !choices.contains(choice) {
                                return Err(ParameterError::ValidationFailed {
                                    message: format!("Invalid choice '{}'. Valid options: {}", choice, choices.join(", ")),
                                });
                            }
                        }
                    }
                    serde_json::Value::Array(selected.into_iter().map(serde_json::Value::String).collect())
                }
            }
        };
        
        resolved.insert(param.name.clone(), value);
    }
    
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::PromptSubcommand;

    #[tokio::test]
    async fn test_run_prompt_command_list() {
        // Create a List subcommand with minimal arguments
        let subcommand = PromptSubcommand::List {
            format: crate::cli::OutputFormat::Table,
            verbose: false,
            source: None,
            category: None,
            search: None,
        };

        // Run the command - we expect it to succeed
        let test_context = TemplateContext::new();
        let result = run_prompt_command(subcommand, &test_context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_prompt_command_search() {
        // Create a Search subcommand with a simple query
        let subcommand = PromptSubcommand::Search {
            query: "test".to_string(),
            r#in: None,
            regex: false,
            fuzzy: false,
            case_sensitive: false,
            source: None,
            has_arg: None,
            no_args: false,
            full: false,
            format: crate::cli::OutputFormat::Table,
            highlight: true,
            limit: None,
        };

        // Run the command - should return an error since search was removed
        let test_context = TemplateContext::new();
        let result = run_prompt_command(subcommand, &test_context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_prompt_command_test_with_invalid_prompt() {
        // Create a Test subcommand with a non-existent prompt
        let subcommand = PromptSubcommand::Test {
            prompt_name: Some("non_existent_prompt_12345".to_string()),
            file: None,
            vars: vec![],
            raw: false,
            copy: false,
            save: None,
            debug: false,
        };

        // Run the command - should return an error
        let test_context = TemplateContext::new();
        let result = run_prompt_command(subcommand, &test_context).await;
        assert!(result.is_err());

        // Verify the error has the expected exit code
        if let Err(e) = result {
            assert_eq!(e.exit_code, 1);
        }
    }

    #[test]
    fn test_is_partial_template() {
        use swissarmyhammer::prompts::Prompt;

        // Test template with partial marker
        let partial_prompt = Prompt {
            name: "test-partial".to_string(),
            description: None,
            category: None,
            tags: vec![],
            template: "{% partial %}\nThis is a partial template".to_string(),
            parameters: vec![],
            source: None,
            metadata: Default::default(),
        };
        assert!(is_partial_template(&partial_prompt));

        // Test template with partial description
        let partial_desc_prompt = Prompt {
            name: "test-partial-desc".to_string(),
            description: Some("Partial template for reuse in other prompts".to_string()),
            category: None,
            tags: vec![],
            template: "Regular template content".to_string(),
            parameters: vec![],
            source: None,
            metadata: Default::default(),
        };
        assert!(is_partial_template(&partial_desc_prompt));

        // Test regular template
        let regular_prompt = Prompt {
            name: "test-regular".to_string(),
            description: Some("A regular prompt".to_string()),
            category: None,
            tags: vec![],
            template: "This is a regular template".to_string(),
            parameters: vec![],
            source: None,
            metadata: Default::default(),
        };
        assert!(!is_partial_template(&regular_prompt));
    }
}
