//! Agent list command implementation

use crate::cli::OutputFormat;
use crate::context::CliContext;
use anyhow::Result;
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
            super::display::DisplayRows::Standard(items) => display_items_with_format(&items, output_format)?,
            super::display::DisplayRows::Verbose(items) => display_items_with_format(&items, output_format)?,
        }
    }

    Ok(())
}

/// Display agent summary information followed by a table
fn display_agent_summary_and_table(
    agents: &[swissarmyhammer_config::agent::AgentInfo], 
    verbose: bool
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
                println!(
                    "{}",
                    tabled::Table::new(&items).with(tabled::settings::Style::modern())
                );
            }
        }
        super::display::DisplayRows::Verbose(items) => {
            if items.is_empty() {
                println!("No agents available");
            } else {
                println!(
                    "{}",
                    tabled::Table::new(&items).with(tabled::settings::Style::modern())
                );
            }
        }
    }
    
    Ok(())
}

/// Display items using the specified format (for JSON/YAML)
fn display_items_with_format<T>(items: &[T], format: OutputFormat) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize + tabled::Tabled,
{
    match format {
        OutputFormat::Table => {
            if items.is_empty() {
                println!("No items to display");
            } else {
                println!(
                    "{}",
                    tabled::Table::new(items).with(tabled::settings::Style::modern())
                );
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


