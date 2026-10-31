//! Tool doubles shared by the tool registry test modules.

use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolCategory, ToolContext};
use crate::{impl_default_doctorable, impl_empty_initializable};
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use std::sync::Arc;

/// Build a bare `ToolContext` with default handlers, no git state and the
/// default agent config.
pub(super) fn new_context() -> ToolContext {
    ToolContext::new(
        Arc::new(ToolHandlers::new()),
        Arc::new(tokio::sync::Mutex::new(None)),
        Arc::new(swissarmyhammer_config::model::ChatModelConfig::default()),
    )
}

/// Mock tool for testing
pub(super) struct MockTool {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
}

impl_default_doctorable!(MockTool);
impl_empty_initializable!(MockTool);

#[async_trait::async_trait]
impl McpTool for MockTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Mock tool {} executed",
            self.name
        ))]))
    }
}

/// Native host tool the [`ReplacementMockTool`] claims to supersede.
pub(super) const REPLACEMENT_MOCK_NATIVE: &str = "Edit";

/// Mock tool in the `Replacement` category, for per-host composition tests.
pub(super) struct ReplacementMockTool;

impl_default_doctorable!(ReplacementMockTool);
impl_empty_initializable!(ReplacementMockTool);

#[async_trait::async_trait]
impl McpTool for ReplacementMockTool {
    fn name(&self) -> &'static str {
        "replacement_mock"
    }

    fn description(&self) -> &'static str {
        "Replacement-category mock tool"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Replacement {
            native: REPLACEMENT_MOCK_NATIVE,
        }
    }

    async fn execute(
        &self,
        _arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("replacement"))
    }
}

/// Macro to create test tools with minimal boilerplate
///
/// This macro reduces code duplication by generating identical tool implementations
/// that differ only in their name and description.
macro_rules! test_tool {
    ($name:ident, $tool_name:literal, $description:literal) => {
        #[allow(dead_code)]
        pub(super) struct $name;

        $crate::impl_default_doctorable!($name);
        $crate::impl_empty_initializable!($name);

        #[async_trait::async_trait]
        impl McpTool for $name {
            fn name(&self) -> &'static str {
                $tool_name
            }
            fn description(&self) -> &'static str {
                $description
            }
            fn schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            async fn execute(
                &self,
                _args: serde_json::Map<String, serde_json::Value>,
                _ctx: &ToolContext,
            ) -> std::result::Result<CallToolResult, McpError> {
                Ok(BaseToolImpl::create_success_response("Test"))
            }
        }
    };
}

// Test tools for CLI integration testing
test_tool!(
    KanbanTool,
    "kanban",
    "Kanban board operations for task management"
);
test_tool!(
    WebTool,
    "web_search_and_fetch",
    "Search the web and fetch URL content"
);
test_tool!(GitStatusTool, "git_status", "Show the working tree status");
test_tool!(
    OutlineGenerateTool,
    "outline_generate",
    "Generate structured code overviews using Tree-sitter parsing"
);
test_tool!(
    UnknownCategoryTool,
    "unknown_something",
    "A tool with an unknown category prefix"
);
test_tool!(
    NoUnderscoreTool,
    "noundercore",
    "A tool without underscore in name"
);
test_tool!(
    MultiLineTool,
    "multi_line",
    "First line of description\nSecond line should not appear\nThird line also should not appear"
);

// Test tools for validation testing
pub(super) struct ValidTool;
pub(super) struct InvalidSchemaTool;
pub(super) struct MissingCategoryTool;

impl_default_doctorable!(ValidTool);
impl_default_doctorable!(InvalidSchemaTool);
impl_default_doctorable!(MissingCategoryTool);
impl_empty_initializable!(ValidTool);
impl_empty_initializable!(InvalidSchemaTool);
impl_empty_initializable!(MissingCategoryTool);

#[async_trait::async_trait]
impl McpTool for ValidTool {
    fn name(&self) -> &'static str {
        "valid_test"
    }
    fn description(&self) -> &'static str {
        "A valid test tool"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "param": {"type": "string", "description": "Test parameter"}
            },
            "required": ["param"]
        })
    }
    async fn execute(
        &self,
        _args: serde_json::Map<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("Test"))
    }
    fn cli_category(&self) -> Option<&'static str> {
        Some("test")
    }
}

#[async_trait::async_trait]
impl McpTool for InvalidSchemaTool {
    fn name(&self) -> &'static str {
        "invalid_schema_test"
    }
    fn description(&self) -> &'static str {
        "A tool with invalid schema"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",  // Not supported
                    "properties": {}
                }
            }
        })
    }
    async fn execute(
        &self,
        _args: serde_json::Map<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("Test"))
    }
    fn cli_category(&self) -> Option<&'static str> {
        Some("test")
    }
}

#[async_trait::async_trait]
impl McpTool for MissingCategoryTool {
    fn name(&self) -> &'static str {
        "missing_category_test"
    }
    fn description(&self) -> &'static str {
        "A tool missing CLI category"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "param": {"type": "string"}
            }
        })
    }
    async fn execute(
        &self,
        _args: serde_json::Map<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("Test"))
    }
    fn cli_category(&self) -> Option<&'static str> {
        None // Missing category but not hidden from CLI
    }
}
