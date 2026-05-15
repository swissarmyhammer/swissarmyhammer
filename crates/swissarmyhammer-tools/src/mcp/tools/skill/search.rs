//! Search skills operation wrapper

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_skills::{Execute, SearchSkill, SkillContext, SkillLibrary};
use tokio::sync::RwLock;

/// Execute the search skill operation
pub async fn execute_search(
    arguments: serde_json::Map<String, serde_json::Value>,
    library: &Arc<RwLock<SkillLibrary>>,
) -> Result<CallToolResult, McpError> {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required field: 'query'", None))?;

    let ctx = SkillContext::new(library.clone());
    let op = SearchSkill::new(query);
    super::convert_result(op.execute(&ctx).await)
}
