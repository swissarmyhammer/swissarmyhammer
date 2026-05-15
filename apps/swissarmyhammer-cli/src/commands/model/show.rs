//! Model show command - display current model configuration

use crate::context::CliContext;
use colored::Colorize;
use comfy_table::Cell;
use swissarmyhammer_config::model::{ModelManager, ModelPaths};

pub async fn execute_show_command(
    _context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}", "Current Model:".bold());

    let mut table = swissarmyhammer_doctor::new_table();

    table.set_header(vec!["Model", "Source"]);

    let (agent_name, source) = match ModelManager::get_agent(&ModelPaths::sah()) {
        Ok(Some(name)) => {
            let source = ModelManager::find_agent_by_name(&name)
                .map(|info| format!("{:?}", info.source))
                .unwrap_or_else(|_| "unknown".to_string());
            (name, source)
        }
        Ok(None) => ("claude-code (default)".to_string(), "default".to_string()),
        Err(_) => ("(error)".to_string(), "error".to_string()),
    };

    table.add_row(vec![Cell::new(agent_name), Cell::new(source)]);

    println!("{table}");
    Ok(())
}
