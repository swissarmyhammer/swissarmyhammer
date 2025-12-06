//! Agent list command implementation

use crate::cli::OutputFormat;
use crate::context::CliContext;
use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Table};
use swissarmyhammer_config::model::{ModelConfigSource, ModelManager};

/// Execute the agent list command - shows all available agents
pub async fn execute_list_command(
    format: OutputFormat,
    context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!("Starting agent list command");

    // Load all agents using ModelManager
    let agents = match ModelManager::list_agents() {
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
    agents: &[swissarmyhammer_config::model::ModelInfo],
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (builtin_count, project_count, user_count) = count_agents_by_source(agents);
    display_agent_counts(agents.len(), builtin_count, project_count, user_count);
    display_agent_table(agents, verbose)?;
    Ok(())
}

/// Count agents by source type
fn count_agents_by_source(
    agents: &[swissarmyhammer_config::model::ModelInfo],
) -> (usize, usize, usize) {
    let mut builtin_count = 0;
    let mut project_count = 0;
    let mut user_count = 0;

    for agent in agents {
        match agent.source {
            ModelConfigSource::Builtin => builtin_count += 1,
            ModelConfigSource::Project => project_count += 1,
            ModelConfigSource::User => user_count += 1,
        }
    }

    (builtin_count, project_count, user_count)
}

/// Display agent count summary
fn display_agent_counts(total: usize, builtin: usize, project: usize, user: usize) {
    println!("Models: {}", total);

    if builtin > 0 {
        println!("Built-in: {}", builtin);
    }
    if project > 0 {
        println!("Project: {}", project);
    }
    if user > 0 {
        println!("User: {}", user);
    }

    println!(); // Empty line before table
}

/// Display agent table based on display rows
fn display_agent_table(
    agents: &[swissarmyhammer_config::model::ModelInfo],
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    use comfy_table::*;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(headers.clone());

    // Set column widths based on header count
    // Standard view: Name (25), Description (40), Source (10)
    // Verbose view: Name (25), Description (40), Source (10), Content Size (12)
    if headers.len() == 3 {
        table.set_width(80);
        table
            .column_mut(0)
            .expect("Column 0 exists")
            .set_constraint(ColumnConstraint::LowerBoundary(Width::Fixed(25)));
        table
            .column_mut(1)
            .expect("Column 1 exists")
            .set_constraint(ColumnConstraint::UpperBoundary(Width::Fixed(40)));
        table
            .column_mut(2)
            .expect("Column 2 exists")
            .set_constraint(ColumnConstraint::LowerBoundary(Width::Fixed(10)));
    } else if headers.len() == 4 {
        table.set_width(90);
        table
            .column_mut(0)
            .expect("Column 0 exists")
            .set_constraint(ColumnConstraint::LowerBoundary(Width::Fixed(25)));
        table
            .column_mut(1)
            .expect("Column 1 exists")
            .set_constraint(ColumnConstraint::UpperBoundary(Width::Fixed(40)));
        table
            .column_mut(2)
            .expect("Column 2 exists")
            .set_constraint(ColumnConstraint::LowerBoundary(Width::Fixed(10)));
        table
            .column_mut(3)
            .expect("Column 3 exists")
            .set_constraint(ColumnConstraint::LowerBoundary(Width::Fixed(12)));
    }

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

/// Format a JSON value as a string
fn format_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => v.to_string(),
    }
}

/// Build a dynamic table from JSON items
fn build_dynamic_table(
    items: &serde_json::Value,
) -> Result<Table, Box<dyn std::error::Error + Send + Sync>> {
    let array = items.as_array().ok_or("Expected JSON array")?;

    if array.is_empty() {
        return Err("No items to display".into());
    }

    let first = array.first().ok_or("Array is empty")?;

    let first_obj = first.as_object().ok_or("Expected JSON object")?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);

    // Add header row with capitalized keys
    let headers: Vec<String> = first_obj.keys().map(|k| capitalize_first(k)).collect();
    table.set_header(headers);

    // Add data rows
    for item in array {
        let obj = item.as_object().ok_or("Expected JSON object in array")?;

        let row: Vec<String> = obj.values().map(format_json_value).collect();
        table.add_row(row);
    }

    Ok(table)
}

/// Display items as a table
fn display_as_table<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    if items.is_empty() {
        println!("No items to display");
        return Ok(());
    }

    let json_items = serde_json::to_value(items)?;
    let table = build_dynamic_table(&json_items)?;
    println!("{table}");
    Ok(())
}

/// Display items as JSON
fn display_as_json<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    let json = serde_json::to_string_pretty(items)?;
    println!("{}", json);
    Ok(())
}

/// Display items as YAML
fn display_as_yaml<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    let yaml = serde_yaml::to_string(items)?;
    println!("{}", yaml);
    Ok(())
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
        OutputFormat::Table => display_as_table(items),
        OutputFormat::Json => display_as_json(items),
        OutputFormat::Yaml => display_as_yaml(items),
    }
}
