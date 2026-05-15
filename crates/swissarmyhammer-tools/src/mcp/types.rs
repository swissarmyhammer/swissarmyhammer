//! Request and response types for MCP operations, along with constants

use serde::Deserialize;
use std::collections::HashMap;

/// Request structure for getting a prompt
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPromptRequest {
    /// Name of the prompt to retrieve
    pub name: String,
    /// Optional arguments for template rendering
    #[serde(default)]
    pub arguments: HashMap<String, String>,
}

/// Request structure for listing prompts
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListPromptsRequest {
    /// Optional filter by category
    pub category: Option<String>,
}

// WebFetchRequest is now provided by swissarmyhammer-web
pub use swissarmyhammer_web::WebFetchRequest;
