//! Agent management tool
//!
//! Exposes subagent definitions via MCP so coding agents can discover and delegate
//! to specialized subagents. The tool's description dynamically includes
//! `<available_agents>` so the host agent knows about subagents without them
//! being in the system prompt.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::Value;
use std::sync::Arc;
use swissarmyhammer_agents::{
    parse_input, AgentContext, AgentLibrary, AgentOperation, ExecutionResult, ListAgents, Operation,
    SearchAgent, UseAgent,
};
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;
use tokio::sync::RwLock;

// Static operation instances for metadata access
static LIST_AGENTS: Lazy<ListAgents> = Lazy::new(ListAgents::new);
static USE_AGENT: Lazy<UseAgent> = Lazy::new(|| UseAgent::new(""));
static SEARCH_AGENT: Lazy<SearchAgent> = Lazy::new(|| SearchAgent::new(""));

/// All agent operations for schema generation
static AGENT_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*LIST_AGENTS as &dyn Operation,
        &*USE_AGENT as &dyn Operation,
        &*SEARCH_AGENT as &dyn Operation,
    ]
});

/// Static description prefix (the `<available_agents>` section is appended dynamically)
static DESCRIPTION_PREFIX: &str = include_str!("description.md");

/// MCP tool for agent discovery and activation
pub struct AgentMcpTool {
    /// Dynamic description including available agents
    description: String,
    /// Shared agent library
    library: Arc<RwLock<AgentLibrary>>,
    /// Prompt library for rendering agent templates with partials
    prompt_library: Arc<RwLock<PromptLibrary>>,
}

impl AgentMcpTool {
    /// Create a new AgentMcpTool with a pre-loaded agent library and prompt library for rendering
    pub fn new(
        library: Arc<RwLock<AgentLibrary>>,
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

/// Build the full tool description including `<available_agents>` listing
fn build_description(library: &Arc<RwLock<AgentLibrary>>) -> String {
    let agents_xml = match library.try_read() {
        Ok(lib) => {
            let agents = lib.list();
            if agents.is_empty() {
                String::new()
            } else {
                let mut xml = String::from("\n<available_agents>\n");
                for agent in &agents {
                    xml.push_str(&format!(
                        "<agent>\n  <name>{}</name>\n  <description>{}</description>\n  <location>{}</location>\n</agent>\n",
                        agent.name, agent.description, agent.source
                    ));
                }
                xml.push_str("</available_agents>\n");
                xml
            }
        }
        Err(_) => String::new(),
    };

    format!("{}{}", DESCRIPTION_PREFIX, agents_xml)
}

impl swissarmyhammer_common::health::Doctorable for AgentMcpTool {
    fn name(&self) -> &str {
        "Agent"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        use swissarmyhammer_common::health::HealthCheck;

        let cat = self.category();
        match self.library.try_read() {
            Ok(lib) => {
                let agents = lib.list();
                if agents.is_empty() {
                    vec![HealthCheck::warning(
                        "Agent library",
                        "No agents loaded in library",
                        Some("Run 'sah init' to install agents".to_string()),
                        cat,
                    )]
                } else {
                    vec![HealthCheck::ok(
                        "Agent library",
                        format!("{} agents available", agents.len()),
                        cat,
                    )]
                }
            }
            Err(_) => {
                vec![HealthCheck::warning(
                    "Agent library",
                    "Could not read agent library (lock held)",
                    None,
                    cat,
                )]
            }
        }
    }
}

#[async_trait]
impl McpTool for AgentMcpTool {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn description(&self) -> &'static str {
        // Leak the string so it lives for 'static — registered once per process lifetime.
        let leaked: &'static str = Box::leak(self.description.clone().into_boxed_str());
        leaked
    }

    fn schema(&self) -> serde_json::Value {
        swissarmyhammer_agents::generate_agent_mcp_schema(&AGENT_OPERATIONS)
    }

    fn operations(&self) -> &'static [&'static dyn Operation] {
        let ops: &[&'static dyn Operation] = &AGENT_OPERATIONS;
        // SAFETY: AGENT_OPERATIONS is a static Lazy<Vec<...>> initialized once
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
        let ctx = AgentContext::new(self.library.clone());

        let input = Value::Object(arguments);
        let operation = parse_input(input).map_err(|e| {
            McpError::invalid_params(format!("Failed to parse agent operation: {}", e), None)
        })?;

        let is_use_op = matches!(&operation, AgentOperation::Use(_));

        let result = match operation {
            AgentOperation::List(op) => {
                use swissarmyhammer_agents::Execute;
                op.execute(&ctx).await
            }
            AgentOperation::Use(op) => {
                use swissarmyhammer_agents::Execute;
                op.execute(&ctx).await
            }
            AgentOperation::Search(op) => {
                use swissarmyhammer_agents::Execute;
                op.execute(&ctx).await
            }
        };

        match result {
            ExecutionResult::Unlogged { value } => {
                // For Use operations, render instructions through the prompt library's
                // Liquid template engine so {% include %} partials are resolved
                let value = if is_use_op {
                    self.render_agent_instructions(value).await
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
                format!("agent operation failed: {}", error),
                None,
            )),
        }
    }
}

impl AgentMcpTool {
    /// Render agent instructions through the prompt library's Liquid template engine.
    async fn render_agent_instructions(&self, mut value: Value) -> Value {
        if let Some(instructions) = value.get("instructions").and_then(|v| v.as_str()) {
            let template_context = TemplateContext::new();
            let prompt_lib = self.prompt_library.read().await;
            match prompt_lib.render_text(instructions, &template_context) {
                Ok(rendered) => {
                    value["instructions"] = Value::String(rendered);
                }
                Err(e) => {
                    tracing::warn!("Failed to render agent template: {e}");
                }
            }
        }
        value
    }
}

/// Register agent tools with the tool registry
pub fn register_agent_tools(
    registry: &mut ToolRegistry,
    library: Arc<RwLock<AgentLibrary>>,
    prompt_library: Arc<RwLock<PromptLibrary>>,
) {
    registry.register(AgentMcpTool::new(library, prompt_library));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_prompt_library() -> Arc<RwLock<PromptLibrary>> {
        Arc::new(RwLock::new(PromptLibrary::default()))
    }

    #[tokio::test]
    async fn test_agent_tool_schema() {
        let library = Arc::new(RwLock::new(AgentLibrary::new()));
        let tool = AgentMcpTool::new(library, default_prompt_library());

        let schema = tool.schema();
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn test_agent_tool_description_with_agents() {
        let library = Arc::new(RwLock::new(AgentLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = AgentMcpTool::new(library, default_prompt_library());
        let desc = tool.description();
        assert!(desc.contains("available_agents"));
        assert!(desc.contains("test"));
    }

    #[tokio::test]
    async fn test_agent_tool_list() {
        let library = Arc::new(RwLock::new(AgentLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = AgentMcpTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "list agent"})).unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_tool_use() {
        let library = Arc::new(RwLock::new(AgentLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = AgentMcpTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "use agent", "name": "test"})).unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_tool_search() {
        let library = Arc::new(RwLock::new(AgentLibrary::new()));
        {
            let mut lib = library.write().await;
            lib.load_defaults();
        }

        let tool = AgentMcpTool::new(library, default_prompt_library());
        let ctx = crate::test_utils::create_test_context().await;

        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"op": "search agent", "query": "test"}))
                .unwrap();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());
    }
}
