//! Show current model configuration.

use crate::AvpCliError;
use colored::Colorize;
use comfy_table::Cell;
use swissarmyhammer_config::model::{ModelManager, ModelPaths};

pub fn run_show() -> Result<(), AvpCliError> {
    println!("{}", "Current Model:".bold());

    let mut table = swissarmyhammer_doctor::new_table();
    table.set_header(vec!["Model", "Source"]);

    let (model_name, source) = match ModelManager::get_agent(&ModelPaths::avp()) {
        Ok(Some(name)) => {
            let source = ModelManager::find_agent_by_name(&name)
                .map(|info| format!("{:?}", info.source))
                .unwrap_or_else(|_| "unknown".to_string());
            (name, source)
        }
        Ok(None) => ("claude-code (default)".to_string(), "default".to_string()),
        Err(_) => ("(error)".to_string(), "error".to_string()),
    };

    table.add_row(vec![Cell::new(model_name), Cell::new(source)]);
    println!("{table}");

    Ok(())
}
