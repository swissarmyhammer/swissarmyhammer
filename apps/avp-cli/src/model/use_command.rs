//! Set the active model.

use crate::AvpCliError;
use colored::Colorize;
use swissarmyhammer_config::model::{ModelError, ModelManager, ModelPaths};

pub fn run_use(name: &str) -> Result<(), AvpCliError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AvpCliError::Validation(
            "Model name cannot be empty".to_string(),
        ));
    }

    match ModelManager::use_agent(name, &ModelPaths::avp()) {
        Ok(()) => {
            println!(
                "{} Successfully set model to: {}",
                "✓".green(),
                name.green().bold()
            );

            if let Ok(info) = ModelManager::find_agent_by_name(name) {
                println!("   Source: {:?}", info.source);
                if let Some(desc) = &info.description {
                    println!("   Description: {}", desc.dimmed());
                }
            }

            Ok(())
        }
        Err(ModelError::NotFound(name)) => {
            eprintln!("{} Model '{}' not found", "✗".red(), name);

            if let Ok(available) = ModelManager::list_agents() {
                let suggestions: Vec<_> = available
                    .iter()
                    .filter(|a| a.name.contains(&name) || name.contains(&a.name))
                    .take(3)
                    .collect();

                if !suggestions.is_empty() {
                    eprintln!("\nDid you mean:");
                    for s in suggestions {
                        eprintln!("  • {}", s.name.cyan());
                    }
                } else {
                    eprintln!("\nAvailable models:");
                    for a in available.iter().take(5) {
                        eprintln!("  • {}", a.name.cyan());
                    }
                    if available.len() > 5 {
                        eprintln!("  ... and {} more", available.len() - 5);
                    }
                }
            }

            Err(AvpCliError::Validation(format!(
                "Model '{}' not found",
                name
            )))
        }
        Err(e) => Err(AvpCliError::Validation(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_name() {
        let result = run_use("");
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_name() {
        let result = run_use("   ");
        assert!(result.is_err());
    }
}
