//! Agent show command - display current use case assignments

use crate::context::CliContext;
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use swissarmyhammer_config::agent::{AgentManager, AgentUseCase};

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
        let agent_name = match AgentManager::get_agent_for_use_case(use_case) {
            Ok(Some(name)) => name,
            Ok(None) => "(default)".to_string(),
            Err(_) => "(error)".to_string(),
        };

        // Try to get agent info for source
        let source = if let Ok(Some(ref name)) = AgentManager::get_agent_for_use_case(use_case) {
            if let Ok(agent_info) = AgentManager::find_agent_by_name(name) {
                format!("{:?}", agent_info.source)
            } else {
                "unknown".to_string()
            }
        } else {
            "default".to_string()
        };

        table.add_row(vec![
            Cell::new(use_case.to_string()),
            Cell::new(agent_name),
            Cell::new(source),
        ]);
    }

    println!("{table}");
    Ok(())
}
