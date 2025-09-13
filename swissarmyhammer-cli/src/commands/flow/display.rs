use serde::Serialize;
use swissarmyhammer::Workflow;
use tabled::Tabled;

/// Returns a default description when the provided description is empty
fn get_description_or_default(description: &str) -> String {
    if description.is_empty() {
        "No description".to_string()
    } else {
        description.to_string()
    }
}

#[derive(Tabled, Serialize)]
pub struct WorkflowInfo {
    #[tabled(rename = "Workflow")]
    pub name: String,

    #[tabled(rename = "Description")]
    pub description: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseWorkflowInfo {
    #[tabled(rename = "Workflow")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Actions")]
    pub action_count: String,
}

impl From<&Workflow> for WorkflowInfo {
    fn from(workflow: &Workflow) -> Self {
        Self {
            name: workflow.name.as_str().to_string(),
            description: get_description_or_default(&workflow.description),
        }
    }
}

impl From<&Workflow> for VerboseWorkflowInfo {
    fn from(workflow: &Workflow) -> Self {
        let title = workflow
            .metadata
            .get("title")
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Convert workflow name to a readable title
                workflow
                    .name
                    .as_str()
                    .replace(['-', '_'], " ")
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => {
                                first.to_uppercase().collect::<String>() + chars.as_str()
                            }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            });

        Self {
            name: workflow.name.as_str().to_string(),
            title,
            description: get_description_or_default(&workflow.description),
            action_count: workflow.states.len().to_string(),
        }
    }
}