//! Prompt show command - display detailed information about a single prompt.

use anyhow::{bail, Result};
use swissarmyhammer::{PromptLibrary, PromptResolver};

use super::cli::ShowCommand;

/// Execute the show command: display detailed info about a prompt.
pub async fn execute_show_command(
    cmd: ShowCommand,
    context: &crate::context::CliContext,
) -> Result<()> {
    if cmd.prompt_name.is_empty() {
        bail!("Prompt name is required. Usage: sah prompt show <name>");
    }

    let mut resolver = PromptResolver::new();
    let mut library = PromptLibrary::new();
    resolver.load_all_prompts(&mut library)?;

    let prompt = library.get(&cmd.prompt_name).map_err(|_| {
        anyhow::anyhow!(
            "Prompt '{}' not found. Run 'sah prompt list' to see available prompts.",
            cmd.prompt_name
        )
    })?;

    let file_source = resolver.prompt_sources.get(&cmd.prompt_name);

    match context.format {
        crate::cli::OutputFormat::Json => {
            let detail = PromptDetail::from_prompt(&prompt, file_source);
            println!("{}", serde_json::to_string_pretty(&detail)?);
        }
        crate::cli::OutputFormat::Yaml => {
            let detail = PromptDetail::from_prompt(&prompt, file_source);
            println!("{}", serde_yaml::to_string(&detail)?);
        }
        _ => {
            print_prompt_detail(&prompt, file_source, context.verbose);
        }
    }

    Ok(())
}

/// Serializable prompt detail for JSON/YAML output.
#[derive(serde::Serialize)]
struct PromptDetail {
    name: String,
    title: String,
    description: String,
    source: String,
    source_path: Option<String>,
    category: Option<String>,
    tags: Vec<String>,
    parameters: Vec<ParameterDetail>,
    template: String,
}

#[derive(serde::Serialize)]
struct ParameterDetail {
    name: String,
    description: String,
    required: bool,
    default: Option<String>,
}

impl PromptDetail {
    fn from_prompt(
        prompt: &swissarmyhammer_prompts::Prompt,
        file_source: Option<&swissarmyhammer::FileSource>,
    ) -> Self {
        Self {
            name: prompt.name.clone(),
            title: prompt
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("No title")
                .to_string(),
            description: prompt
                .description
                .clone()
                .unwrap_or_else(|| "No description".to_string()),
            source: file_source
                .map(|s| s.display_emoji())
                .unwrap_or("Built-in")
                .to_string(),
            source_path: prompt.source.as_ref().map(|p| p.display().to_string()),
            category: prompt.category.clone(),
            tags: prompt.tags.clone(),
            parameters: prompt
                .parameters
                .iter()
                .map(|p| ParameterDetail {
                    name: p.name.clone(),
                    description: p.description.clone(),
                    required: p.required,
                    default: p.default.as_ref().map(|v| v.to_string()),
                })
                .collect(),
            template: prompt.template.clone(),
        }
    }
}

fn print_prompt_detail(
    prompt: &swissarmyhammer_prompts::Prompt,
    file_source: Option<&swissarmyhammer::FileSource>,
    verbose: bool,
) {
    let title = prompt
        .metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("No title");
    let description = prompt.description.as_deref().unwrap_or("No description");
    let source_label = file_source.map(|s| s.display_emoji()).unwrap_or("Built-in");

    println!("Name:        {}", prompt.name);
    println!("Title:       {}", title);
    println!("Description: {}", description);
    println!("Source:      {}", source_label);

    if let Some(path) = &prompt.source {
        println!("Path:        {}", path.display());
    }

    if let Some(category) = &prompt.category {
        println!("Category:    {}", category);
    }

    if !prompt.tags.is_empty() {
        println!("Tags:        {}", prompt.tags.join(", "));
    }

    if !prompt.parameters.is_empty() {
        println!();
        println!("Parameters:");
        for param in &prompt.parameters {
            let required = if param.required {
                "required"
            } else {
                "optional"
            };
            let default = param
                .default
                .as_ref()
                .map(|v| format!(" (default: {})", v))
                .unwrap_or_default();
            println!(
                "  {} - {} [{}{}]",
                param.name, param.description, required, default
            );
        }
    }

    if verbose || prompt.template.len() <= 500 {
        println!();
        println!("Template:");
        println!("{}", prompt.template);
    } else {
        println!();
        println!("Template (first 500 chars, use --verbose for full):");
        let truncated: String = prompt.template.chars().take(500).collect();
        println!("{}", truncated);
        println!("...");
    }
}
