//! Prompt command implementation
//!
//! Manages and tests prompts with support for listing, validating, testing, and searching

use crate::cli::PromptSubcommand;
use crate::error::{CliError, CliResult};
use crate::exit_codes::EXIT_SUCCESS;
use std::collections::HashMap;
use swissarmyhammer::{PromptFilter, PromptLibrary, PromptResolver};

/// Help text for the prompt command
pub const DESCRIPTION: &str = include_str!("description.md");

/// Handle the prompt command
pub async fn handle_command(subcommand: PromptSubcommand) -> i32 {
    match run_prompt_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Prompt command failed: {}", e);
            e.exit_code
        }
    }
}

/// Main entry point for prompt command
async fn run_prompt_command(subcommand: PromptSubcommand) -> CliResult<()> {
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
        } => run_test_command(prompt_name, file, vars, raw, copy, save, debug)
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

/// Run the test command
async fn run_test_command(
    prompt_name: Option<String>,
    _file: Option<String>,
    vars: Vec<String>,
    _raw: bool,
    _copy: bool,
    _save: Option<String>,
    _debug: bool,
) -> Result<(), anyhow::Error> {
    let prompt_name = prompt_name.ok_or_else(|| anyhow::anyhow!("Prompt name is required"))?;

    // Load all prompts
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)?;

    // Parse variables
    let mut arguments = HashMap::new();
    for var in vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() == 2 {
            arguments.insert(parts[0].to_string(), parts[1].to_string());
        }
    }

    // Render the prompt
    let rendered = library.render_prompt(&prompt_name, &arguments)?;
    println!("{}", rendered);

    Ok(())
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
        let result = run_prompt_command(subcommand).await;
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
        let result = run_prompt_command(subcommand).await;
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
        let result = run_prompt_command(subcommand).await;
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
