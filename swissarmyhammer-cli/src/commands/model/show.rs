//! Agent show command - display current use case assignments

use crate::context::CliContext;
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use swissarmyhammer_config::model::{AgentUseCase, ModelManager};

/// Helper function to retrieve agent name and source for a given use case
fn get_agent_info_for_use_case(use_case: AgentUseCase) -> (String, String) {
    match ModelManager::get_agent_for_use_case(use_case) {
        Ok(Some(name)) => {
            let source = ModelManager::find_agent_by_name(&name)
                .map(|info| format!("{:?}", info.source))
                .unwrap_or_else(|_| "unknown".to_string());
            (name, source)
        }
        Ok(None) => ("(default)".to_string(), "default".to_string()),
        Err(_) => ("(error)".to_string(), "error".to_string()),
    }
}

pub async fn execute_show_command(
    _context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}", "Agent Use Case Assignments:".bold());

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec!["Use Case", "Agent", "Source"]);

    // Show each use case
    for use_case in [
        AgentUseCase::Root,
        AgentUseCase::Rules,
        AgentUseCase::Workflows,
    ] {
        let (agent_name, source) = get_agent_info_for_use_case(use_case);

        table.add_row(vec![
            Cell::new(use_case.to_string()),
            Cell::new(agent_name),
            Cell::new(source),
        ]);
    }

    println!("{table}");
    Ok(())
}
