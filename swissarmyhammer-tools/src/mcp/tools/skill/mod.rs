//! Skill management tool
//!
//! Replicates Claude Code's skill system for llama-agent using the Operations pattern.
//! The tool's description dynamically includes `<available_skills>` so the agent
//! knows about skills without them being in the system prompt.

use crate::mcp::tool_registry::{AgentTool, BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::Value;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_skills::{
    parse_input, ExecutionResult, ListSkills, Operation, SearchSkill, SkillContext, SkillLibrary,
    SkillOperation, UseSkill,
};
use tokio::sync::RwLock;

// Static operation instances for metadata access
static LIST_SKILLS: Lazy<ListSkills> = Lazy::new(ListSkills::new);
static USE_SKILL: Lazy<UseSkill> = Lazy::new(|| UseSkill::new(""));
static SEARCH_SKILL: Lazy<SearchSkill> = Lazy::new(|| SearchSkill::new(""));

/// All skill operations for schema generation
static SKILL_OPERATIONS: Lazy<&'static [&'static dyn Operation]> = Lazy::new(|| {
    let ops: Vec<&'static dyn Operation> = vec![
        &*LIST_SKILLS as &dyn Operation,
        &*USE_SKILL as &dyn Operation,
        &*SEARCH_SKILL as &dyn Operation,
    ];
    Box::leak(ops.into_boxed_slice())
});

/// Static description prefix (the `<available_skills>` section is appended dynamically)
static DESCRIPTION_PREFIX: &str = include_str!("description.md");

/// MCP tool for skill discovery and activation
pub struct SkillTool {
    /// Static description computed once at construction time
    description: &'static str,
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
        let description: &'static str = Box::leak(description.into_boxed_str());
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
        self.description
    }

    fn schema(&self) -> serde_json::Value {
        swissarmyhammer_skills::generate_skill_mcp_schema(&SKILL_OPERATIONS)
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        *SKILL_OPERATIONS
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

        // Track whether this is a Use operation (needs template rendering)
        let is_use_op = matches!(&operation, SkillOperation::Use(_));

        // Execute the operation
        let result = match operation {
            SkillOperation::List(op) => {
                use swissarmyhammer_skills::Execute;
                op.execute(&ctx).await
            }
            SkillOperation::Use(op) => {
                use swissarmyhammer_skills::Execute;
                op.execute(&ctx).await
            }
            SkillOperation::Search(op) => {
                use swissarmyhammer_skills::Execute;
                op.execute(&ctx).await
            }
        };

        // Convert result to CallToolResult
        match result {
            ExecutionResult::Unlogged { value } => {
                // For Use operations, render instructions through the prompt library's
                // Liquid template engine so {% include %} partials are resolved
                let value = if is_use_op {
                    self.render_skill_instructions(value).await
                } else {
                    value
                };
                Ok(BaseToolImpl::create_success_response(
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
                ))
            }
            ExecutionResult::Logged { value, .. } => Ok(BaseToolImpl::create_success_response(
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
            )),
            ExecutionResult::Failed { error, .. } => Err(McpError::internal_error(
                format!("skill operation failed: {}", error),
                None,
            )),
        }
    }
}

impl SkillTool {
    /// Render skill instructions through the prompt library's Liquid template engine.
    ///
    /// This enables skills to use `{% include %}` partials from the prompt library
    /// (e.g., `{% include "_partials/detected-projects" %}`), rendering them as if
    /// they were prompts — the only difference being the frontmatter format.
    async fn render_skill_instructions(&self, mut value: Value) -> Value {
        if let Some(instructions) = value.get("instructions").and_then(|v| v.as_str()) {
            let template_context = TemplateContext::new();
            let prompt_lib = self.prompt_library.read().await;
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
}
