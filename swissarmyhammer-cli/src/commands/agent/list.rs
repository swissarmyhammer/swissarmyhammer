//! Agent list command implementation

use crate::cli::OutputFormat;
use crate::context::CliContext;
use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Table};
use swissarmyhammer_config::agent::{AgentManager, AgentSource};

/// Execute the agent list command - shows all available agents
pub async fn execute_list_command(
    format: OutputFormat,
    context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!("Starting agent list command");

    // Load all agents using AgentManager
    let agents = match AgentManager::list_agents() {
        Ok(agents) => agents,
        Err(e) => {
            tracing::error!("Failed to load agents: {}", e);
            return Err(format!("Failed to discover agents: {}", e).into());
        }
    };

    // Use the provided format directly
    let output_format = format;

    // For table format, show summary information
    if matches!(output_format, OutputFormat::Table) {
        display_agent_summary_and_table(&agents, context.verbose)?;
    } else {
        // For JSON/YAML formats, just display the data directly
        let display_rows = super::display::agents_to_display_rows(agents, context.verbose);
        match display_rows {
            super::display::DisplayRows::Standard(items) => {
                display_items_with_format(&items, output_format)?
            }
            super::display::DisplayRows::Verbose(items) => {
                display_items_with_format(&items, output_format)?
            }
        }
    }

    Ok(())
}

/// Display agent summary information followed by a table
fn display_agent_summary_and_table(
    agents: &[swissarmyhammer_config::agent::AgentInfo],
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Count agents by source
    let mut builtin_count = 0;
    let mut project_count = 0;
    let mut user_count = 0;

    for agent in agents {
        match agent.source {
            AgentSource::Builtin => builtin_count += 1,
            AgentSource::Project => project_count += 1,
            AgentSource::User => user_count += 1,
        }
    }

    // Display summary information that tests expect
    println!("Agents: {}", agents.len());

    if builtin_count > 0 {
        println!("Built-in: {}", builtin_count);
    }
    if project_count > 0 {
        println!("Project: {}", project_count);
    }
    if user_count > 0 {
        println!("User: {}", user_count);
    }

    println!(); // Empty line before table

    // Display the table
    let display_rows = super::display::agents_to_display_rows(agents.to_vec(), verbose);
    match display_rows {
        super::display::DisplayRows::Standard(items) => {
            if items.is_empty() {
                println!("No agents available");
            } else {
                let table = create_table(&items, vec!["Name", "Description", "Source"], |item| {
                    vec![&item.name, &item.description, &item.source]
                });
                println!("{table}");
            }
        }
        super::display::DisplayRows::Verbose(items) => {
            if items.is_empty() {
                println!("No agents available");
            } else {
                let table = create_table(
                    &items,
                    vec!["Name", "Description", "Source", "Content Size"],
                    |item| {
                        vec![
                            &item.name,
                            &item.description,
                            &item.source,
                            &item.content_size,
                        ]
                    },
                );
                println!("{table}");
            }
        }
    }

    Ok(())
}

/// Create a table with given headers and row mapper function
fn create_table<'a, T, F>(items: &'a [T], headers: Vec<&str>, row_mapper: F) -> Table
where
    F: Fn(&'a T) -> Vec<&'a str>,
{
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(headers);

    for item in items {
        table.add_row(row_mapper(item));
    }

    table
}

/// Capitalize the first character of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Display items using the specified format (for JSON/YAML)
fn display_items_with_format<T>(
    items: &[T],
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    match format {
        OutputFormat::Table => {
            if items.is_empty() {
                println!("No items to display");
            } else {
                // Convert items to JSON for dynamic table building
                let json_items = serde_json::to_value(items)?;

                if let Some(array) = json_items.as_array() {
                    if let Some(first) = array.first() {
                        if let Some(obj) = first.as_object() {
                            let mut table = Table::new();
                            table.load_preset(UTF8_FULL);

                            // Add header row with capitalized keys
                            let headers: Vec<String> =
                                obj.keys().map(|k| capitalize_first(k)).collect();
                            table.set_header(headers);

                            // Add data rows
                            for item in array {
                                if let Some(obj) = item.as_object() {
                                    let row: Vec<String> = obj
                                        .values()
                                        .map(|v| match v {
                                            serde_json::Value::String(s) => s.clone(),
                                            serde_json::Value::Number(n) => n.to_string(),
                                            serde_json::Value::Bool(b) => b.to_string(),
                                            serde_json::Value::Null => "null".to_string(),
                                            _ => v.to_string(),
                                        })
                                        .collect();
                                    table.add_row(row);
                                }
                            }

                            println!("{table}");
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(items)?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(items)?;
            println!("{}", yaml);
        }
    }
    Ok(())
}
