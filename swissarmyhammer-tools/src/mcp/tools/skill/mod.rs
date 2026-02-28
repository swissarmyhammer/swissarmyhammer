//! Unified skill operations tool for MCP
//!
//! This module provides a single `skill` tool that dispatches between operations:
//! - `list skill`: List all available skills with their descriptions
//! - `use skill`: Activate a skill by loading its full instructions
//! - `search skill`: Search for skills by name or description
//!
//! Follows the Operation pattern from `swissarmyhammer-operations`.

mod list;
mod search;
mod use_op;

use crate::mcp::tool_registry::{AgentTool, BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_operations::Operation;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_skills::{
    ExecutionResult, ListSkills, SearchSkill, SkillError, SkillLibrary, UseSkill,
};
use tokio::sync::RwLock;

// Static operation instances for schema generation
static LIST_SKILLS: Lazy<ListSkills> = Lazy::new(ListSkills::new);
static USE_SKILL: Lazy<UseSkill> = Lazy::new(|| UseSkill::new(""));
static SEARCH_SKILL: Lazy<SearchSkill> = Lazy::new(|| SearchSkill::new(""));

/// All skill operations for schema generation
static SKILL_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*LIST_SKILLS as &dyn Operation,
        &*USE_SKILL as &dyn Operation,
        &*SEARCH_SKILL as &dyn Operation,
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
    /// Prompt library for rendering skill templates with partials
    prompt_library: Arc<RwLock<PromptLibrary>>,
}

impl SkillTool {
    /// Create a new SkillTool with a pre-loaded skill library and prompt library for rendering
    pub fn new(
        library: Arc<RwLock<SkillLibrary>>,
        prompt_library: Arc<RwLock<PromptLibrary>>,
    ) -> Self {
        let description = build_description(&library);
        Self {
            description,
            library,
            prompt_library,
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

/// Convert a skill ExecutionResult into an MCP CallToolResult
fn convert_result(
    result: ExecutionResult<serde_json::Value, SkillError>,
) -> Result<CallToolResult, McpError> {
    match result {
        ExecutionResult::Unlogged { value } => Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        )),
        ExecutionResult::Logged { value, .. } => Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        )),
        ExecutionResult::Failed { error, .. } => Err(McpError::internal_error(
            format!("skill operation failed: {}", error),
            None,
        )),
    }
}

impl swissarmyhammer_common::health::Doctorable for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        use swissarmyhammer_common::health::HealthCheck;

        let mut checks = Vec::new();
        let cat = self.category();

        // Determine project root for checking installed skills
        let project_root = swissarmyhammer_common::utils::find_git_repository_root()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let skills_dir = project_root.join(".claude").join("skills");

        // Use the library to get the list of expected skills
        match self.library.try_read() {
            Ok(lib) => {
                let skills = lib.list();
                if skills.is_empty() {
                    checks.push(HealthCheck::warning(
                        "Skills library",
                        "No skills loaded in library",
                        Some("Run 'sah init' to install skills".to_string()),
                        cat,
                    ));
                    return checks;
                }

                let mut missing = Vec::new();
                for skill in &skills {
                    let skill_md = skills_dir.join(skill.name.as_str()).join("SKILL.md");
                    if !skill_md.exists() {
                        missing.push(skill.name.as_str().to_string());
                    }
                }

                if missing.is_empty() {
                    checks.push(HealthCheck::ok(
                        "Skills installation",
                        format!(
                            "All {} skills installed in {}",
                            skills.len(),
                            skills_dir.display()
                        ),
                        cat,
                    ));
                } else {
                    checks.push(HealthCheck::warning(
                        "Skills installation",
                        format!("Missing skills: {}", missing.join(", ")),
                        Some("Run 'sah init' to install skills".to_string()),
                        cat,
                    ));
                }
            }
            Err(_) => {
                checks.push(HealthCheck::warning(
                    "Skills library",
                    "Could not read skill library (locked)",
                    None,
                    cat,
                ));
            }
        }

        checks
    }
}

#[async_trait]
impl AgentTool for SkillTool {}

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

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
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
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        // Strip the "op" key from arguments before passing to handlers
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "list skill" => list::execute_list(args, &self.library).await,
            "use skill" | "get skill" | "load skill" | "activate skill" | "invoke skill" => {
                use_op::execute_use(args, &self.library, &self.prompt_library).await
            }
            "search skill" | "find skill" | "lookup skill" => {
                search::execute_search(args, &self.library).await
            }
            "" => {
                // Infer operation from present keys
                if args.contains_key("name") {
                    use_op::execute_use(args, &self.library, &self.prompt_library).await
                } else if args.contains_key("query") {
                    search::execute_search(args, &self.library).await
                } else {
                    list::execute_list(args, &self.library).await
                }
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'list skill', 'use skill', 'search skill'",
                    other
                ),
                None,
            )),
        }
    }
}

/// Register skill tools with the tool registry
pub fn register_skill_tools(
    registry: &mut ToolRegistry,
    library: Arc<RwLock<SkillLibrary>>,
    prompt_library: Arc<RwLock<PromptLibrary>>,
) {
    registry.register(SkillTool::new(library, prompt_library));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_prompt_library() -> Arc<RwLock<PromptLibrary>> {
        Arc::new(RwLock::new(PromptLibrary::default()))
    }

    #[tokio::test]
    async fn test_skill_tool_schema() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        let tool = SkillTool::new(library, default_prompt_library());

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

        let tool = SkillTool::new(library, default_prompt_library());
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

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "list skill"})).unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_tool_use() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "use skill", "name": "plan"})).unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_tool_get_backward_compat() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "get skill", "name": "plan"})).unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_tool_search() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "search skill", "query": "plan"}))
                .unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_use_renders_partials() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        // Use a default prompt library — render_text() loads all builtins including partials
        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        // The "test" skill includes {% include "_partials/test-driven-development" %}
        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "use skill", "name": "test"})).unwrap();
        let result = tool
            .execute(args, &ctx)
            .await
            .expect("use skill should succeed");

        let content = result
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");

        // "TDD Cycle" only exists in the _partials/test-driven-development partial
        assert!(
            content.contains("TDD Cycle"),
            "Rendered instructions should contain partial content 'TDD Cycle'"
        );

        // Raw {% include %} tags should be resolved, not passed through
        assert!(
            !content.contains("{% include"),
            "Rendered output should not contain raw Liquid include tags"
        );
    }

    #[tokio::test]
    async fn test_skill_tool_schema_has_op_field() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        let tool = SkillTool::new(library, default_prompt_library());
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert!(op_enum.contains(&serde_json::json!("list skill")));
        assert!(op_enum.contains(&serde_json::json!("use skill")));
        assert!(op_enum.contains(&serde_json::json!("search skill")));
    }

    #[tokio::test]
    async fn test_skill_tool_unknown_op() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "op".to_string(),
            serde_json::Value::String("invalid op".to_string()),
        );

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_skill_tool_infer_use_from_name_key() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        // When "name" key is present but no "op", should infer use
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("plan".to_string()),
        );

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_tool_infer_search_from_query_key() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        // When "query" key is present but no "op", should infer search
        let mut args = serde_json::Map::new();
        args.insert(
            "query".to_string(),
            serde_json::Value::String("plan".to_string()),
        );

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skill_tool_infer_list_from_empty() {
        let library = Arc::new(RwLock::new(SkillLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = SkillTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        // Empty args should infer list
        let args = serde_json::Map::new();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }
}
