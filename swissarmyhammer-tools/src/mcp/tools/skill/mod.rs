//! Skill management tool
//!
//! Replicates Claude Code's skill system for llama-agent using the Operations pattern.
//! The tool's description dynamically includes `<available_skills>` so the agent
//! knows about skills without them being in the system prompt.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::Value;
use std::sync::Arc;
use swissarmyhammer_skills::{
    parse_input, ExecutionResult, GetSkill, ListSkills, Operation, SkillContext, SkillLibrary,
    SkillOperation,
};
use tokio::sync::RwLock;

// Static operation instances for metadata access
static LIST_SKILLS: Lazy<ListSkills> = Lazy::new(ListSkills::new);
static GET_SKILL: Lazy<GetSkill> = Lazy::new(|| GetSkill::new(""));

/// All skill operations for schema generation
static SKILL_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*LIST_SKILLS as &dyn Operation,
        &*GET_SKILL as &dyn Operation,
    ]
});

/// Static description prefix (the `<available_skills>` section is appended dynamically)
static DESCRIPTION_PREFIX: &str = include_str!("description.md");

/// MCP tool for skill discovery and activation
pub struct SkillTool {
    /// Dynamic description including available skills
    description: String,
    /// Shared skill library
    library: Arc<RwLock<SkillLibrary>>,
}

impl SkillTool {
    /// Create a new SkillTool with a pre-loaded skill library
    pub fn new(library: Arc<RwLock<SkillLibrary>>) -> Self {
        // Build the dynamic description with available skills
        let description = build_description(&library);
        Self {
            description,
            library,
        }
    }
}

/// Build the full tool description including `<available_skills>` listing
fn build_description(library: &Arc<RwLock<SkillLibrary>>) -> String {
    // We need to block on reading the library to build the description at registration time.
    // This is safe because registration happens during server startup.
    let skills_xml = match library.try_read() {
        Ok(lib) => {
            let skills = lib.list();
            if skills.is_empty() {
                String::new()
            } else {
                let mut xml = String::from("\n<available_skills>\n");
                for skill in &skills {
                    xml.push_str(&format!(
                        "<skill>\n  <name>{}</name>\n  <description>{}</description>\n  <location>{}</location>\n</skill>\n",
                        skill.name, skill.description, skill.source
                    ));
                }
                xml.push_str("</available_skills>\n");
                xml
            }
        }
        Err(_) => String::new(),
    };

    format!("{}{}", DESCRIPTION_PREFIX, skills_xml)
}

// No health checks needed
crate::impl_empty_doctorable!(SkillTool);

#[async_trait]
impl McpTool for SkillTool {
    fn name(&self) -> &'static str {
        "skill"
    }

    fn description(&self) -> &'static str {
        // We need 'static but our description is dynamic.
        // Leak the string so it lives for 'static — this is fine since
        // the tool is registered once and lives for the process lifetime.
        // SAFETY: This leaks a small amount of memory (once per process).
        let leaked: &'static str = Box::leak(self.description.clone().into_boxed_str());
        leaked
    }

    fn schema(&self) -> serde_json::Value {
        swissarmyhammer_skills::generate_skill_mcp_schema(&SKILL_OPERATIONS)
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        let ops: &[&'static dyn Operation] = &SKILL_OPERATIONS;
        // SAFETY: SKILL_OPERATIONS is a static Lazy<Vec<...>> initialized once
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let ctx = SkillContext::new(self.library.clone());

        // Parse the input to determine operation
        let input = Value::Object(arguments);
        let operation = parse_input(input).map_err(|e| {
            McpError::invalid_params(format!("Failed to parse skill operation: {}", e), None)
        })?;

        // Execute the operation
        let result = match operation {
            SkillOperation::List(op) => {
                use swissarmyhammer_skills::Execute;
                op.execute(&ctx).await
            }
            SkillOperation::Get(op) => {
                use swissarmyhammer_skills::Execute;
                op.execute(&ctx).await
            }
        };

        // Convert result to CallToolResult
        match result {
            ExecutionResult::Unlogged { value } => Ok(BaseToolImpl::create_success_response(
                serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| value.to_string()),
            )),
            ExecutionResult::Logged { value, .. } => Ok(BaseToolImpl::create_success_response(
                serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|_| value.to_string()),
            )),
            ExecutionResult::Failed { error, .. } => Err(McpError::internal_error(
                format!("skill operation failed: {}", error),
                None,
            )),
        }
    }
}

/// Register skill tools with the tool registry
pub fn register_skill_tools(registry: &mut ToolRegistry, library: Arc<RwLock<SkillLibrary>>) {
    registry.register(SkillTool::new(library));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_tool_schema() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        let tool = SkillTool::new(library);

        let schema = tool.schema();
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn test_skill_tool_description_with_skills() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library);
        let desc = tool.description();
        assert!(desc.contains("available_skills"));
        assert!(desc.contains("plan"));
    }

    #[tokio::test]
    async fn test_skill_tool_list() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library);
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "list skill"})).unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_tool_get() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library);
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "get skill", "name": "plan"}))
                .unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }
}
