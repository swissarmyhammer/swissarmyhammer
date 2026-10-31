//! CLI category, name, about text and hidden-tool behaviour.

use super::fixtures::*;
use crate::mcp::tool_registry::*;
use crate::{impl_default_doctorable, impl_empty_initializable};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

#[test]
fn test_cli_category_extraction() {
    // Test known categories
    assert_eq!(WebTool.cli_category(), Some("web"));
    assert_eq!(OutlineGenerateTool.cli_category(), Some("outline"));

    // Test unknown category
    assert_eq!(UnknownCategoryTool.cli_category(), None);

    // Test no underscore
    assert_eq!(NoUnderscoreTool.cli_category(), None);
}

#[test]
fn test_cli_name_extraction() {
    // Test action extraction
    assert_eq!(WebTool.cli_name(), "search_and_fetch");
    assert_eq!(OutlineGenerateTool.cli_name(), "generate");

    // Test unknown category still extracts action
    assert_eq!(UnknownCategoryTool.cli_name(), "something");

    // Test no underscore returns full name
    assert_eq!(NoUnderscoreTool.cli_name(), "noundercore");
}

#[test]
fn test_cli_about_extraction() {
    // Test first line extraction
    assert_eq!(
        KanbanTool.cli_about(),
        Some("Kanban board operations for task management")
    );
    assert_eq!(MultiLineTool.cli_about(), Some("First line of description"));
}

#[test]
fn test_hidden_from_cli_default() {
    // Test default implementation returns false
    assert!(!KanbanTool.hidden_from_cli());
    assert!(!UnknownCategoryTool.hidden_from_cli());
    assert!(!NoUnderscoreTool.hidden_from_cli());
}

#[test]
fn test_cli_integration_comprehensive() {
    // Test a tool that should not be visible in CLI (unknown category)
    let tool = UnknownCategoryTool;
    assert_eq!(tool.cli_category(), None);
    assert_eq!(tool.cli_name(), "something");
    assert_eq!(
        tool.cli_about(),
        Some("A tool with an unknown category prefix")
    );
    assert!(!tool.hidden_from_cli());
}

/// `cli_about` feeds clap's one-line command summary. A wrapper tag is not a
/// summary, so `sah tool --help` must show the first line of prose instead.
#[test]
fn test_cli_about_skips_wrapper_tag_lines() {
    assert_eq!(
        MockTool {
            name: "tag_wrapped",
            description:
                "<skills_instructions>\nYou have access to skills.\n</skills_instructions>",
        }
        .cli_about(),
        Some("You have access to skills.")
    );
}

// --- get_cli_categories / get_tools_for_category / get_cli_tools tests ---

#[test]
fn test_get_cli_categories_empty_registry() {
    let registry = ToolRegistry::new();
    let categories = registry.get_cli_categories();
    assert!(categories.is_empty());
}

#[test]
fn test_get_cli_categories_returns_sorted_unique() {
    let mut registry = ToolRegistry::new();
    // KanbanTool → category "kanban"
    // WebTool → category "web"
    // GitStatusTool and MockTool "git_log" → both category "git"
    registry.register(KanbanTool);
    registry.register(WebTool);
    registry.register(GitStatusTool);
    registry.register(MockTool {
        name: "git_log",
        description: "Show the commit log",
    });

    let categories = registry.get_cli_categories();
    // Should be sorted
    let mut sorted = categories.clone();
    sorted.sort();
    assert_eq!(categories, sorted);
    // Two tools share the "git" category, so it must be listed exactly once
    assert_eq!(categories.iter().filter(|c| *c == "git").count(), 1);
    assert!(categories.contains(&"kanban".to_string()));
    assert!(categories.contains(&"web".to_string()));
    assert!(categories.contains(&"git".to_string()));
}

#[test]
fn test_get_tools_for_category_returns_correct_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);
    registry.register(WebTool);
    registry.register(GitStatusTool);

    let kanban_tools = registry.get_tools_for_category("kanban");
    assert_eq!(kanban_tools.len(), 1);
    assert_eq!(<dyn McpTool as McpTool>::name(kanban_tools[0]), "kanban");

    let web_tools = registry.get_tools_for_category("web");
    assert_eq!(web_tools.len(), 1);
    assert_eq!(
        <dyn McpTool as McpTool>::name(web_tools[0]),
        "web_search_and_fetch"
    );
}

#[test]
fn test_get_tools_for_category_unknown_returns_empty() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);

    let tools = registry.get_tools_for_category("nonexistent_category");
    assert!(tools.is_empty());
}

#[test]
fn test_get_cli_tools_excludes_unknown_category() {
    let mut registry = ToolRegistry::new();
    // KanbanTool has category, UnknownCategoryTool has no category
    registry.register(KanbanTool);
    registry.register(UnknownCategoryTool);

    let cli_tools = registry.get_cli_tools();
    let cli_names: Vec<&str> = cli_tools
        .iter()
        .map(|t| <dyn McpTool as McpTool>::name(*t))
        .collect();
    assert!(cli_names.contains(&"kanban"));
    // UnknownCategoryTool has no cli_category — should be excluded
    assert!(!cli_names.contains(&"unknown_something"));
}

#[test]
fn test_get_cli_tools_empty_when_no_tool_has_cli_category() {
    // Tools without a cli_category won't appear in get_cli_tools
    let mut registry = ToolRegistry::new();
    registry.register(UnknownCategoryTool);
    registry.register(NoUnderscoreTool);

    let cli_tools = registry.get_cli_tools();
    assert!(cli_tools.is_empty());
}

// --- get_tool_by_cli_name tests ---

#[test]
fn test_get_tool_by_cli_name_found() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);

    let tool = registry.get_tool_by_cli_name(CliCategory("kanban"), CliName("kanban"));
    assert!(tool.is_some());
    assert_eq!(<dyn McpTool as McpTool>::name(tool.unwrap()), "kanban");
}

#[test]
fn test_get_tool_by_cli_name_not_found() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);

    assert!(registry
        .get_tool_by_cli_name(CliCategory("kanban"), CliName("nonexistent"))
        .is_none());
    assert!(registry
        .get_tool_by_cli_name(CliCategory("nonexistent"), CliName("kanban"))
        .is_none());
}

#[test]
fn test_get_tool_by_cli_name_git_status() {
    let mut registry = ToolRegistry::new();
    registry.register(GitStatusTool);

    let tool = registry.get_tool_by_cli_name(CliCategory("git"), CliName("status"));
    assert!(tool.is_some());
}

// --- matches_cli_criteria tests ---

#[test]
fn test_matches_cli_criteria_positive() {
    let registry = ToolRegistry::new();
    let tool = KanbanTool;
    assert!(registry.matches_cli_criteria(&tool, CliCategory("kanban"), CliName("kanban")));
}

#[test]
fn test_matches_cli_criteria_wrong_category() {
    let registry = ToolRegistry::new();
    let tool = KanbanTool;
    assert!(!registry.matches_cli_criteria(&tool, CliCategory("wrong"), CliName("kanban")));
}

#[test]
fn test_matches_cli_criteria_wrong_name() {
    let registry = ToolRegistry::new();
    let tool = KanbanTool;
    assert!(!registry.matches_cli_criteria(&tool, CliCategory("kanban"), CliName("wrong")));
}

// --- Hidden tool for testing ---

struct HiddenTool;
impl_default_doctorable!(HiddenTool);
impl_empty_initializable!(HiddenTool);

#[async_trait::async_trait]
impl McpTool for HiddenTool {
    fn name(&self) -> &'static str {
        "kanban_hidden"
    }
    fn description(&self) -> &'static str {
        "A hidden tool"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }
    async fn execute(
        &self,
        _args: serde_json::Map<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("hidden"))
    }
    fn hidden_from_cli(&self) -> bool {
        true
    }
}

#[test]
fn test_matches_cli_criteria_hidden_tool() {
    let registry = ToolRegistry::new();
    let tool = HiddenTool;
    assert!(!registry.matches_cli_criteria(&tool, CliCategory("kanban"), CliName("hidden")));
}

#[test]
fn test_hidden_tool_excluded_from_cli_categories() {
    let mut registry = ToolRegistry::new();
    registry.register(HiddenTool);

    let categories = registry.get_cli_categories();
    assert!(categories.is_empty());
}

#[test]
fn test_hidden_tool_excluded_from_cli_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(HiddenTool);

    let tools = registry.get_cli_tools();
    assert!(tools.is_empty());
}

// --- cli_about with empty description ---

#[test]
fn test_cli_about_empty_description() {
    let tool = MockTool {
        name: "empty_desc",
        description: "",
    };
    // Empty description — cli_about returns None because lines() yields nothing
    let about = tool.cli_about();
    assert_eq!(about, None);
}

// --- cli_about with only headers ---

#[test]
fn test_cli_about_only_headers() {
    let tool = MockTool {
        name: "header_only",
        description: "# Header\n## Subheader\n### Another",
    };
    // All lines are headers, so cli_about falls back to first line
    let about = tool.cli_about();
    assert_eq!(about, Some("# Header"));
}

// --- validate_cli_requirements_for_tool on hidden tool ---

#[test]
fn test_validate_cli_requirements_hidden_tool_skipped() {
    let registry = ToolRegistry::new();
    let tool = HiddenTool;
    let errors = registry.validate_cli_requirements_for_tool(&tool);
    assert!(errors.is_empty());
}

#[test]
fn test_cli_newtypes_display_their_contents() {
    assert_eq!(CliCategory("git").to_string(), "git");
    assert_eq!(CliName("status").to_string(), "status");
}
