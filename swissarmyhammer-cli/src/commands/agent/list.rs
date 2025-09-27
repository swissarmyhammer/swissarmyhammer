//! Agent list command implementation

use crate::cli::OutputFormat;
use crate::context::CliContext;
use colored::Colorize;
use serde::Serialize;
use std::collections::HashMap;
use swissarmyhammer_config::agent::{AgentManager, AgentSource};
use tabled::Tabled;

/// Display row for agent information in table format
#[derive(Debug, Clone, Serialize, Tabled)]
pub struct AgentDisplayRow {
    #[tabled(rename = "Name")]
    pub name: String,
    #[tabled(rename = "Description")]
    pub description: String,
    #[tabled(rename = "Source")]
    pub source: String,
}

/// Execute the agent list command
pub async fn execute_list_command(
    format: Option<OutputFormat>,
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

    // Check if we have any agents
    if agents.is_empty() {
        match format.unwrap_or(OutputFormat::Table) {
            OutputFormat::Table => {
                println!("No agents found");
                return Ok(());
            }
            OutputFormat::Json => {
                let empty: Vec<AgentDisplayRow> = vec![];
                return context.display(empty).map_err(Into::into);
            }
            OutputFormat::Yaml => {
                let empty: Vec<AgentDisplayRow> = vec![];
                return context.display(empty).map_err(Into::into);
            }
        }
    }

    // Count agents by source for summary
    let mut source_counts = HashMap::new();
    for agent in &agents {
        *source_counts.entry(&agent.source).or_insert(0) += 1;
    }

    // Convert to display rows
    let display_rows: Vec<AgentDisplayRow> = agents
        .iter()
        .map(|agent| AgentDisplayRow {
            name: agent.name.clone(),
            description: agent
                .description
                .as_deref()
                .unwrap_or("No description")
                .to_string(),
            source: match agent.source {
                AgentSource::Builtin => "builtin".to_string(),
                AgentSource::Project => "project".to_string(),
                AgentSource::User => "user".to_string(),
            },
        })
        .collect();

    // Handle different output formats
    let output_format = format.unwrap_or(OutputFormat::Table);

    match output_format {
        OutputFormat::Table => {
            display_agents_table(&agents, &source_counts)?;
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&display_rows)
                .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&display_rows)
                .map_err(|e| format!("Failed to serialize to YAML: {}", e))?;
            println!("{}", yaml);
        }
    }

    tracing::debug!("Agent list command completed successfully");
    Ok(())
}

/// Display agents in table format with colors and summary
fn display_agents_table(
    agents: &[swissarmyhammer_config::agent::AgentInfo],
    source_counts: &HashMap<&AgentSource, usize>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Show summary line
    println!("🤖 Agents: {} total", agents.len());

    // Show source breakdown
    let builtin_count = source_counts.get(&AgentSource::Builtin).unwrap_or(&0);
    let project_count = source_counts.get(&AgentSource::Project).unwrap_or(&0);
    let user_count = source_counts.get(&AgentSource::User).unwrap_or(&0);

    println!(
        "📦 Built-in: {}, 📁 Project: {}, 👤 User: {}",
        builtin_count, project_count, user_count
    );

    if !agents.is_empty() {
        println!(); // Blank line before entries
    }

    // Display each agent with two-line format and colors
    for (i, agent) in agents.iter().enumerate() {
        if i > 0 {
            println!(); // Blank line between entries
        }

        // First line: Name | Description (colored by source)
        let name_desc = format!(
            "{} | {}",
            agent.name,
            agent.description.as_deref().unwrap_or("No description")
        );

        let colored_line = match agent.source {
            AgentSource::Builtin => name_desc.green(),
            AgentSource::Project => name_desc.yellow(),
            AgentSource::User => name_desc.blue(),
        };

        println!("{}", colored_line);

        // Second line: source info (dimmed)
        let source_name = match agent.source {
            AgentSource::Builtin => "builtin",
            AgentSource::Project => "project",
            AgentSource::User => "user",
        };
        println!("  {}", format!("source: {}", source_name).dimmed());
    }

    Ok(())
}
