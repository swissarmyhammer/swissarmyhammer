use serde::Serialize;
use swissarmyhammer::Workflow;
use tabled::Tabled;

/// Convert FileSource to emoji representation for consistent display across all listing commands.
/// This ensures all three table displays (prompt, flow, agent) use the same emoji mapping:
/// - 📦 Built-in: System-provided built-in items
/// - 📁 Project: Project-specific items from .swissarmyhammer directory  
/// - 👤 User: User-specific items from user's home directory
fn file_source_to_emoji(source: Option<&swissarmyhammer::FileSource>) -> &'static str {
    match source {
        Some(swissarmyhammer::FileSource::Builtin) => "📦 Built-in",
        Some(swissarmyhammer::FileSource::Local) => "📁 Project", 
        Some(swissarmyhammer::FileSource::User) => "👤 User",
        Some(swissarmyhammer::FileSource::Dynamic) | None => "📦 Built-in", // Default fallback
    }
}

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

    #[tabled(rename = "Source")]
    pub source: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseWorkflowInfo {
    #[tabled(rename = "Workflow")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Source")]
    pub source: String,

    #[tabled(rename = "Actions")]
    pub action_count: String,
}

impl From<&Workflow> for WorkflowInfo {
    fn from(workflow: &Workflow) -> Self {
        Self {
            name: workflow.name.as_str().to_string(),
            description: get_description_or_default(&workflow.description),
            source: "Unknown".to_string(), // Fallback when source info not available
        }
    }
}

impl WorkflowInfo {
    /// Create WorkflowInfo with FileSource information for emoji-based source display
    pub fn from_workflow_with_source(
        workflow: &Workflow,
        file_source: Option<&swissarmyhammer::FileSource>
    ) -> Self {
        Self {
            name: workflow.name.as_str().to_string(),
            description: get_description_or_default(&workflow.description),
            source: file_source_to_emoji(file_source).to_string(),
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
            source: "Unknown".to_string(), // Fallback when source info not available
            action_count: workflow.states.len().to_string(),
        }
    }
}

impl VerboseWorkflowInfo {
    /// Create VerboseWorkflowInfo with FileSource information for emoji-based source display
    pub fn from_workflow_with_source(
        workflow: &Workflow,
        file_source: Option<&swissarmyhammer::FileSource>
    ) -> Self {
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
            source: file_source_to_emoji(file_source).to_string(),
            action_count: workflow.states.len().to_string(),
        }
    }
}
