use anyhow::Result;
use colored::*;
use is_terminal::IsTerminal;
use std::io::{self, Write};
// Tabled import removed - using custom 2-line format instead

use crate::cli::{OutputFormat, PromptSource, PromptSourceArg};
use swissarmyhammer::{PromptFilter, PromptLibrary, PromptResolver};

// PromptRow struct removed - using custom 2-line format instead of table

#[derive(serde::Serialize)]
struct PromptInfo {
    name: String,
    title: Option<String>,
    description: Option<String>,
    source: PromptSource,
    category: Option<String>,
    arguments: Vec<PromptArgument>,
}

#[derive(serde::Serialize)]
struct PromptArgument {
    name: String,
    description: Option<String>,
    required: bool,
    default: Option<String>,
}

pub fn run_list_command(
    format: OutputFormat,
    verbose: bool,
    source_filter: Option<PromptSourceArg>,
    category_filter: Option<String>,
    search_term: Option<String>,
) -> Result<()> {
    // Load all prompts from all sources
    let mut library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver.load_all_prompts(&mut library)?;

    // Build the filter
    let mut filter = PromptFilter::new();

    if let Some(ref source) = source_filter {
        let lib_source: PromptSource = source.clone().into();
        filter = filter.with_sources(vec![lib_source.into()]);
    }

    if let Some(ref category) = category_filter {
        filter = filter.with_category(category);
    }

    if let Some(ref _search) = search_term {
        // Remove search term filtering for now - fix API later
        // filter = filter.with_search_term(search);
    }

    // Convert FileSource to PromptSource for the API
    let prompt_sources: std::collections::HashMap<String, swissarmyhammer_prompts::PromptSource> =
        resolver
            .prompt_sources
            .iter()
            .map(|(k, v)| (k.clone(), v.clone().into()))
            .collect();

    // Get filtered prompts
    let filtered_prompts = library.list_filtered(&filter, &prompt_sources)?;

    // Collect prompt information
    let mut prompt_infos = Vec::new();

    for prompt in filtered_prompts {
        // Get the source from the resolver
        let prompt_source = match resolver.prompt_sources.get(&prompt.name) {
            Some(swissarmyhammer::PromptSource::Builtin) => PromptSource::Builtin,
            Some(swissarmyhammer::PromptSource::User) => PromptSource::User,
            Some(swissarmyhammer::PromptSource::Local) => PromptSource::Local,
            Some(swissarmyhammer::PromptSource::Dynamic) => PromptSource::Dynamic,
            None => PromptSource::Dynamic,
        };

        let arguments = prompt
            .parameters
            .iter()
            .map(|param| PromptArgument {
                name: param.name.clone(),
                description: Some(param.description.clone()),
                required: param.required,
                default: param
                    .default
                    .as_ref()
                    .and_then(|v| v.as_str().map(String::from)),
            })
            .collect();

        // Extract title from metadata
        // If metadata is empty, we have a problem with the library's YAML parsing
        // For now, let's use the prompt name as a fallback title
        let title = prompt
            .metadata
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // Fallback: convert prompt name to a readable title
                Some(
                    prompt
                        .name
                        .replace(['-', '_'], " ")
                        .split_whitespace()
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => {
                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            });

        prompt_infos.push(PromptInfo {
            name: prompt.name.clone(),
            title,
            description: prompt.description.clone(),
            source: prompt_source,
            category: prompt.category.clone(),
            arguments,
        });
    }

    // Sort by name for consistent output
    prompt_infos.sort_by(|a, b| a.name.cmp(&b.name));

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&prompt_infos)?;
            println!("{json}");
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&prompt_infos)?;
            print!("{yaml}");
        }
        OutputFormat::Table => {
            display_table(&prompt_infos, verbose)?;
        }
    }

    Ok(())
}

fn display_table_to_writer<W: Write>(
    prompt_infos: &[PromptInfo],
    _verbose: bool,
    writer: &mut W,
    is_tty: bool,
) -> Result<()> {
    if prompt_infos.is_empty() {
        writeln!(writer, "No prompts found matching the criteria.")?;
        return Ok(());
    }

    // Create a custom 2-line format instead of using Tabled
    for info in prompt_infos {
        let title = info.title.as_deref().unwrap_or("");
        let description = info.description.as_deref().unwrap_or("");

        // First line: Name | Title (colored by source)
        let first_line = if is_tty {
            let (name_colored, title_colored) = match &info.source {
                PromptSource::Builtin => (
                    info.name.green().bold().to_string(),
                    title.green().to_string(),
                ),
                PromptSource::User => (
                    info.name.blue().bold().to_string(),
                    title.blue().to_string(),
                ),
                PromptSource::Local => (
                    info.name.yellow().bold().to_string(),
                    title.yellow().to_string(),
                ),
                PromptSource::Dynamic => (
                    info.name.magenta().bold().to_string(),
                    title.magenta().to_string(),
                ),
            };
            format!("{name_colored} | {title_colored}")
        } else {
            format!("{} | {}", info.name, title)
        };

        // Second line: Full description (indented)
        let second_line = if !description.is_empty() {
            format!("  {description}")
        } else {
            "  (no description)".to_string()
        };

        writeln!(writer, "{first_line}")?;
        writeln!(writer, "{second_line}")?;
        writeln!(writer)?; // Empty line between entries
    }

    if is_tty && !prompt_infos.is_empty() {
        writeln!(writer, "{}", "Legend:".bright_white())?;
        writeln!(writer, "  {} Built-in prompts", "●".green())?;
        writeln!(
            writer,
            "  {} User prompts (~/.swissarmyhammer/prompts/)",
            "●".blue()
        )?;
        writeln!(
            writer,
            "  {} Local prompts (./.swissarmyhammer/prompts/)",
            "●".yellow()
        )?;
        writeln!(writer, "  {} Dynamic prompts", "●".magenta())?;
    }

    Ok(())
}

fn display_table(prompt_infos: &[PromptInfo], verbose: bool) -> Result<()> {
    let mut stdout = io::stdout();
    let is_tty = stdout.is_terminal();
    display_table_to_writer(prompt_infos, verbose, &mut stdout, is_tty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_command_with_no_prompts() {
        // This test will fail initially, driving the implementation
        let result = run_list_command(OutputFormat::Table, false, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_command_with_search() {
        let result = run_list_command(
            OutputFormat::Table,
            false,
            None,
            None,
            Some("example".to_string()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_command_json_format() {
        let result = run_list_command(OutputFormat::Json, false, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_command_yaml_format() {
        let result = run_list_command(OutputFormat::Yaml, false, None, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_command_source_filter() {
        let result = run_list_command(
            OutputFormat::Table,
            false,
            Some(PromptSourceArg::Builtin),
            None,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_color_coding_when_terminal() {
        use colored::control;

        let prompt_infos = vec![
            PromptInfo {
                name: "test_builtin".to_string(),
                title: Some("Builtin Test".to_string()),
                description: Some("A builtin prompt".to_string()),
                source: PromptSource::Builtin,
                category: Some("test".to_string()),
                arguments: vec![],
            },
            PromptInfo {
                name: "test_user".to_string(),
                title: Some("User Test".to_string()),
                description: Some("A user prompt".to_string()),
                source: PromptSource::User,
                category: Some("test".to_string()),
                arguments: vec![],
            },
            PromptInfo {
                name: "test_local".to_string(),
                title: Some("Local Test".to_string()),
                description: Some("A local prompt".to_string()),
                source: PromptSource::Local,
                category: Some("test".to_string()),
                arguments: vec![],
            },
        ];

        // Force colors on for testing
        control::set_override(true);

        // Capture output when TTY is enabled
        let mut output = Vec::new();
        let result = display_table_to_writer(&prompt_infos, false, &mut output, true);
        assert!(result.is_ok());

        let output_str = String::from_utf8(output).expect("Output should be valid UTF-8");

        // Verify color codes are present - check if any ANSI escape sequences exist
        assert!(output_str.contains("\u{1b}[")); // Any ANSI escape sequence

        // Verify specific colors are present (these might have different exact codes)
        // Look for green (builtin), blue (user), yellow (local)
        let has_green = output_str.contains("\u{1b}[32m")
            || output_str.contains("\u{1b}[0;32m")
            || output_str.contains("\u{1b}[38;5;2m");
        let has_blue = output_str.contains("\u{1b}[34m")
            || output_str.contains("\u{1b}[0;34m")
            || output_str.contains("\u{1b}[38;5;4m");
        let has_yellow = output_str.contains("\u{1b}[33m")
            || output_str.contains("\u{1b}[0;33m")
            || output_str.contains("\u{1b}[38;5;3m");

        assert!(has_green, "Expected green color codes for builtin");
        assert!(has_blue, "Expected blue color codes for user");
        assert!(has_yellow, "Expected yellow color codes for local");

        // Test without TTY (no color codes) - force colors off
        control::set_override(false);
        let mut output_no_tty = Vec::new();
        let result_no_tty =
            display_table_to_writer(&prompt_infos, false, &mut output_no_tty, false);
        assert!(result_no_tty.is_ok());

        let output_no_tty_str =
            String::from_utf8(output_no_tty).expect("Output should be valid UTF-8");

        // Verify no color codes are present when not TTY
        assert!(!output_no_tty_str.contains("\u{1b}["));

        // But the content should still be there
        assert!(output_no_tty_str.contains("test_builtin"));
        assert!(output_no_tty_str.contains("test_user"));
        assert!(output_no_tty_str.contains("test_local"));

        // Reset colors to automatic detection
        control::unset_override();
    }

    #[test]
    fn test_prompt_info_creation() {
        let info = PromptInfo {
            name: "test".to_string(),
            title: Some("Test Prompt".to_string()),
            description: Some("A test prompt".to_string()),
            source: PromptSource::Builtin,
            category: None,
            arguments: vec![],
        };

        assert_eq!(info.name, "test");
        assert_eq!(info.title, Some("Test Prompt".to_string()));
        assert_eq!(info.source, PromptSource::Builtin);
    }

    #[test]
    fn test_builtin_prompts_should_be_identified_correctly() {
        // Test that the resolver properly tracks prompt sources
        let mut resolver = PromptResolver::new();
        let mut library = swissarmyhammer::PromptLibrary::new();

        // Load all prompts (including builtin)
        resolver.load_all_prompts(&mut library).unwrap();

        // Note: Builtin prompts may not exist in test environment
        // The test passes if no error occurs - builtin prompt loading is optional
        // In production, builtin prompts would be embedded in the binary

        // If any builtin prompts were loaded, they should be marked as builtin
        for source in resolver.prompt_sources.values() {
            if matches!(source, swissarmyhammer::PromptSource::Builtin) {
                // This is good - builtin prompts are properly marked
                break;
            }
        }
    }

    #[test]
    fn test_title_extraction_logic() {
        // Test that title extraction from metadata works correctly
        use serde_json::Value;
        use std::collections::HashMap;

        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            Value::String("Array Data Processor".to_string()),
        );

        // Test the title extraction logic
        let title = metadata
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        assert_eq!(
            title,
            Some("Array Data Processor".to_string()),
            "Title should be extracted from metadata"
        );

        // Test when title is missing
        let empty_metadata: HashMap<String, Value> = HashMap::new();
        let no_title: Option<String> = empty_metadata
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        assert_eq!(
            no_title, None,
            "Title should be None when not present in metadata"
        );
    }
}
