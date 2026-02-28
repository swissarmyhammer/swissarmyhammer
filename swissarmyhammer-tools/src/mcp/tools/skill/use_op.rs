//! Use skill operation wrapper (with template rendering)

use crate::mcp::tool_registry::BaseToolImpl;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::Value;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_skills::{Execute, ExecutionResult, SkillContext, SkillLibrary, UseSkill};
use tokio::sync::RwLock;

/// Execute the use skill operation, rendering templates through the prompt library
pub async fn execute_use(
    arguments: serde_json::Map<String, serde_json::Value>,
    library: &Arc<RwLock<SkillLibrary>>,
    prompt_library: &Arc<RwLock<PromptLibrary>>,
) -> Result<CallToolResult, McpError> {
    let name = arguments
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required field: 'name'", None))?;

    let ctx = SkillContext::new(library.clone());
    let op = UseSkill::new(name);

    match op.execute(&ctx).await {
        ExecutionResult::Unlogged { value } => {
            let value = render_skill_instructions(value, prompt_library).await;
            Ok(BaseToolImpl::create_success_response(
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
            ))
        }
        ExecutionResult::Logged { value, .. } => {
            let value = render_skill_instructions(value, prompt_library).await;
            Ok(BaseToolImpl::create_success_response(
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
            ))
        }
        ExecutionResult::Failed { error, .. } => Err(McpError::internal_error(
            format!("skill operation failed: {}", error),
            None,
        )),
    }
}

/// Render skill instructions through the prompt library's Liquid template engine.
///
/// This enables skills to use `{% include %}` partials from the prompt library
/// (e.g., `{% include "_partials/detected-projects" %}`), rendering them as if
/// they were prompts.
async fn render_skill_instructions(
    mut value: Value,
    prompt_library: &Arc<RwLock<PromptLibrary>>,
) -> Value {
    if let Some(instructions) = value.get("instructions").and_then(|v| v.as_str()) {
        let template_context = TemplateContext::new();
        let prompt_lib = prompt_library.read().await;
        match prompt_lib.render_text(instructions, &template_context) {
            Ok(rendered) => {
                value["instructions"] = Value::String(rendered);
            }
            Err(e) => {
                tracing::warn!("Failed to render skill template: {e}");
                // Fall through with raw instructions on render failure
            }
        }
    }
    value
}
