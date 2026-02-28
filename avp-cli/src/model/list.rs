//! List available models.

use crate::AvpCliError;
use swissarmyhammer_config::model::ModelManager;

pub fn run_list() -> Result<(), AvpCliError> {
    let models = ModelManager::list_agents().map_err(|e| AvpCliError::Validation(e.to_string()))?;

    if models.is_empty() {
        println!("No models found.");
        return Ok(());
    }

    let mut table = swissarmyhammer_doctor::new_table();
    table.set_header(vec!["Name", "Source", "Description"]);

    for model in &models {
        let source = match model.source {
            swissarmyhammer_config::model::ModelConfigSource::Builtin => "Built-in",
            swissarmyhammer_config::model::ModelConfigSource::Project => "Project",
            swissarmyhammer_config::model::ModelConfigSource::GitRoot => "Git Root",
            swissarmyhammer_config::model::ModelConfigSource::User => "User",
        };
        let description = model.description.as_deref().unwrap_or("-");
        table.add_row(vec![&model.name, source, description]);
    }

    println!("{table}");

    // Summary counts
    let builtin = models
        .iter()
        .filter(|m| {
            matches!(
                m.source,
                swissarmyhammer_config::model::ModelConfigSource::Builtin
            )
        })
        .count();
    let project = models
        .iter()
        .filter(|m| {
            matches!(
                m.source,
                swissarmyhammer_config::model::ModelConfigSource::Project
                    | swissarmyhammer_config::model::ModelConfigSource::GitRoot
            )
        })
        .count();
    let user = models
        .iter()
        .filter(|m| {
            matches!(
                m.source,
                swissarmyhammer_config::model::ModelConfigSource::User
            )
        })
        .count();

    println!(
        "\nModels: {}  Built-in: {}  Project: {}  User: {}",
        models.len(),
        builtin,
        project,
        user
    );

    Ok(())
}
