//! List skills operation wrapper

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_skills::{Execute, ListSkills, SkillContext, SkillLibrary};
use tokio::sync::RwLock;

/// Execute the list skill operation
pub async fn execute_list(
    _arguments: serde_json::Map<String, serde_json::Value>,
    library: &Arc<RwLock<SkillLibrary>>,
) -> Result<CallToolResult, McpError> {
    let ctx = SkillContext::new(library.clone());
    let op = ListSkills::new();
    super::convert_result(op.execute(&ctx).await)
}
