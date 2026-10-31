//! Unit tests for the tool registry.

use super::validation::SchemaValidator;
use super::*;
use crate::mcp::tool_handlers::ToolHandlers;
use crate::{impl_default_doctorable, impl_empty_initializable};
use rmcp::model::RawContent;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use swissarmyhammer_config::model::ChatModelConfig;

/// Mock tool for testing
struct MockTool {
    name: &'static str,
    description: &'static str,
}

impl swissarmyhammer_common::health::Doctorable for MockTool {
    fn name(&self) -> &str {
        self.name
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        Vec::new()
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

crate::impl_empty_initializable!(MockTool);

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

#[test]
fn test_tool_registry_creation() {
    let registry = ToolRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_tool_registration() {
    let mut registry = ToolRegistry::new();
    let tool = MockTool {
        name: "test_tool",
        description: "A test tool",
    };

    registry.register(tool);

    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    assert!(registry.get_tool("test_tool").is_some());
    assert!(registry.get_tool("nonexistent").is_none());
}

#[test]
fn test_tool_lookup() {
    let mut registry = ToolRegistry::new();
    let tool = MockTool {
        name: "lookup_test",
        description: "A lookup test tool",
    };

    registry.register(tool);

    let retrieved_tool = registry.get_tool("lookup_test").unwrap();
    assert_eq!(
        <dyn McpTool as McpTool>::name(retrieved_tool),
        "lookup_test"
    );
    assert_eq!(retrieved_tool.description(), "A lookup test tool");
}

#[test]
fn test_multiple_tool_registration() {
    let mut registry = ToolRegistry::new();

    let tool1 = MockTool {
        name: "tool1",
        description: "First tool",
    };
    let tool2 = MockTool {
        name: "tool2",
        description: "Second tool",
    };

    registry.register(tool1);
    registry.register(tool2);

    assert_eq!(registry.len(), 2);
    assert!(registry.get_tool("tool1").is_some());
    assert!(registry.get_tool("tool2").is_some());

    let tool_names = registry.list_tool_names();
    assert!(tool_names.contains(&"tool1".to_string()));
    assert!(tool_names.contains(&"tool2".to_string()));
}

#[tokio::test]
async fn test_tool_execution() {
    use swissarmyhammer_git::GitOperations;
    use tokio::sync::Mutex;

    // Create temporary directory for test
    let _temp_dir = tempfile::tempdir().unwrap();

    // Create mock storage and handlers for context
    let git_ops: Arc<Mutex<Option<GitOperations>>> = Arc::new(Mutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ChatModelConfig::default());
    let context = ToolContext::new(tool_handlers, git_ops, agent_config);

    let tool = MockTool {
        name: "exec_test",
        description: "Execution test tool",
    };

    let result = tool.execute(serde_json::Map::new(), &context).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
    assert!(!call_result.content.is_empty());
}

#[test]
fn test_base_tool_impl_parse_arguments() {
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Debug)]
    struct TestArgs {
        name: String,
        count: Option<i32>,
    }

    let mut args = serde_json::Map::new();
    args.insert(
        "name".to_string(),
        serde_json::Value::String("test".to_string()),
    );
    args.insert(
        "count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(42)),
    );

    let parsed: TestArgs = BaseToolImpl::parse_arguments(args).unwrap();
    assert_eq!(parsed.name, "test");
    assert_eq!(parsed.count, Some(42));
}

#[test]
fn test_base_tool_impl_parse_arguments_error() {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestArgs {
        #[serde(rename = "required_field")]
        _required_field: String,
    }

    let args = serde_json::Map::new(); // Missing required field

    let result: std::result::Result<TestArgs, McpError> = BaseToolImpl::parse_arguments(args);
    assert!(result.is_err());
}

#[test]
fn test_base_tool_impl_create_success_response() {
    let response = BaseToolImpl::create_success_response("Success message");

    assert_eq!(response.is_error, Some(false));
    assert_eq!(response.content.len(), 1);

    if let RawContent::Text(text_content) = &response.content[0].raw {
        assert_eq!(text_content.text, "Success message");
    } else {
        panic!("Expected text content");
    }
}

#[test]
fn test_base_tool_impl_create_error_response() {
    let response = BaseToolImpl::create_error_response("Error message", None);

    assert_eq!(response.is_error, Some(true));
    assert_eq!(response.content.len(), 1);

    if let RawContent::Text(text_content) = &response.content[0].raw {
        assert_eq!(text_content.text, "Error message");
    } else {
        panic!("Expected text content");
    }
}

#[test]
fn test_base_tool_impl_create_error_response_with_details() {
    let response = BaseToolImpl::create_error_response(
        "Error message",
        Some("Additional details".to_string()),
    );

    assert_eq!(response.is_error, Some(true));
    assert_eq!(response.content.len(), 1);

    if let RawContent::Text(text_content) = &response.content[0].raw {
        assert_eq!(text_content.text, "Error message: Additional details");
    } else {
        panic!("Expected text content");
    }
}

/// Macro to create test tools with minimal boilerplate
///
/// This macro reduces code duplication by generating identical tool implementations
/// that differ only in their name and description.
macro_rules! test_tool {
    ($name:ident, $tool_name:literal, $description:literal) => {
        #[allow(dead_code)]
        struct $name;

        // Implement Doctorable for test tools (empty health checks)
        impl swissarmyhammer_common::health::Doctorable for $name {
            fn name(&self) -> &str {
                $tool_name
            }

            fn category(&self) -> &str {
                "tools"
            }

            fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
                Vec::new()
            }

            fn is_applicable(&self) -> bool {
                true
            }
        }

        crate::impl_empty_initializable!($name);

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
    FilesTool,
    "files",
    "Unified file operations (read, write, edit, glob, grep)"
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

#[test]
fn test_cli_category_extraction() {
    // Test known categories
    assert_eq!(FilesTool.cli_category(), Some("file"));
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
    assert_eq!(FilesTool.cli_name(), "files");
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
        FilesTool.cli_about(),
        Some("Unified file operations (read, write, edit, glob, grep)")
    );
    assert_eq!(MultiLineTool.cli_about(), Some("First line of description"));
}

#[test]
fn test_hidden_from_cli_default() {
    // Test default implementation returns false
    assert!(!FilesTool.hidden_from_cli());
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

#[test]
fn test_files_category_alias() {
    // Test that "files" prefix maps to "file" category
    let tool = FilesTool;
    assert_eq!(tool.cli_category(), Some("file"));
    assert_eq!(tool.cli_name(), "files");
}

// Test tools for validation testing
struct ValidTool;
struct InvalidSchemaTool;
struct MissingCategoryTool;

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

/// A tool whose description opens with an XML-style wrapper tag, the shape
/// the `agent` and `skill` tools use to fence their instructions.
struct TagWrappedDescriptionTool;

impl_default_doctorable!(TagWrappedDescriptionTool);
impl_empty_initializable!(TagWrappedDescriptionTool);

#[async_trait::async_trait]
impl McpTool for TagWrappedDescriptionTool {
    fn name(&self) -> &'static str {
        "tag_wrapped"
    }
    fn description(&self) -> &'static str {
        "<skills_instructions>\nYou have access to skills.\n</skills_instructions>"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }
    async fn execute(
        &self,
        _args: serde_json::Map<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("Test"))
    }
}

/// `cli_about` feeds clap's one-line command summary. A wrapper tag is not a
/// summary, so `sah tool --help` must show the first line of prose instead.
#[test]
fn test_cli_about_skips_wrapper_tag_lines() {
    assert_eq!(
        TagWrappedDescriptionTool.cli_about(),
        Some("You have access to skills.")
    );
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

#[test]
fn test_tool_validation_valid_tool() {
    let mut registry = ToolRegistry::new();
    let tool = ValidTool;
    registry.register(tool);

    let result = registry.validate_cli_tools();
    assert!(result.is_ok());
}

#[test]
fn test_tool_validation_invalid_schema() {
    let mut registry = ToolRegistry::new();
    let tool = InvalidSchemaTool;
    registry.register(tool);

    let result = registry.validate_cli_tools();
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        ToolValidationError::SchemaValidation { .. }
    ));
}

#[test]
fn test_tool_validation_missing_category() {
    let mut registry = ToolRegistry::new();
    let tool = MissingCategoryTool;
    registry.register(tool);

    let result = registry.validate_cli_tools();
    assert!(result.is_err());

    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
    assert!(matches!(
        errors[0],
        ToolValidationError::MissingCliCategory { .. }
    ));
}

#[test]
fn test_tool_validation_report() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);
    registry.register(InvalidSchemaTool);
    registry.register(MissingCategoryTool);

    let report = registry.validate_all_tools();

    assert_eq!(report.total_tools, 3);
    assert_eq!(report.valid_tools, 1);
    assert_eq!(report.invalid_tools, 2);
    assert!(!report.is_valid());
    assert!(!report.errors.is_empty());

    let summary = report.summary();
    assert!(summary.contains("✗"));
    assert!(summary.contains("2 of 3 tools"));
}

#[test]
fn test_tool_validation_warnings() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);
    registry.register(InvalidSchemaTool);

    let warnings = registry.get_tool_validation_warnings();

    // Should have warning for InvalidSchemaTool but not for ValidTool
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].tool_name, "invalid_schema_test");
}

#[test]
fn test_validation_error_display() {
    let error = ToolValidationError::SchemaValidation {
        tool_name: "test_tool".to_string(),
        error: ValidationError::UnsupportedSchemaType {
            schema_type: "object".to_string(),
            parameter: "test_param".to_string(),
        },
    };

    let display_str = format!("{}", error);
    assert!(display_str.contains("schema validation failed"));
    assert!(display_str.contains("test_tool"));
}

#[test]
fn test_individual_tool_validation() {
    let registry = ToolRegistry::new();

    // Test valid tool
    let valid_tool = ValidTool;
    let result = registry.validate_tool(&valid_tool);
    assert!(result.is_ok());

    // Test invalid schema tool
    let invalid_tool = InvalidSchemaTool;
    let result = registry.validate_tool(&invalid_tool);
    assert!(result.is_err());

    // Test missing category tool
    let missing_cat_tool = MissingCategoryTool;
    let result = registry.validate_tool(&missing_cat_tool);
    assert!(result.is_err());
}

/// Helper function to assert error severity with descriptive message
fn assert_error_severity<E: Severity>(error: E, expected: ErrorSeverity, context: &str) {
    assert_eq!(
        error.severity(),
        expected,
        "{} should be {:?}",
        context,
        expected
    );
}

#[test]
fn test_tool_validation_error_severity() {
    // Test Critical severity for schema validation
    let schema_error = ToolValidationError::SchemaValidation {
        tool_name: "test".to_string(),
        error: ValidationError::InvalidSchema {
            message: "test".to_string(),
        },
    };
    assert_error_severity(schema_error, ErrorSeverity::Critical, "Schema validation");

    // Test Error severity for configuration issues
    let missing_category = ToolValidationError::MissingCliCategory {
        tool_name: "test".to_string(),
    };
    assert_error_severity(
        missing_category,
        ErrorSeverity::Error,
        "Missing CLI category",
    );

    let invalid_cli_name = ToolValidationError::InvalidCliName {
        tool_name: "test".to_string(),
        cli_name: "123invalid".to_string(),
        reason: "starts with number".to_string(),
    };
    assert_error_severity(invalid_cli_name, ErrorSeverity::Error, "Invalid CLI name");

    let invalid_description = ToolValidationError::InvalidDescription {
        tool_name: "test".to_string(),
        reason: "too short".to_string(),
    };
    assert_error_severity(
        invalid_description,
        ErrorSeverity::Error,
        "Invalid description",
    );

    let name_conflict = ToolValidationError::NameConflict {
        tool_name: "test".to_string(),
        conflicting_tool: "other".to_string(),
    };
    assert_error_severity(name_conflict, ErrorSeverity::Error, "Name conflict");
}

#[test]
fn test_validation_error_severity() {
    // Test Critical severity for invalid schema
    let invalid_schema = ValidationError::InvalidSchema {
        message: "schema is malformed".to_string(),
    };
    assert_error_severity(invalid_schema, ErrorSeverity::Critical, "Invalid schema");

    // Test Error severity for schema issues
    let unsupported_type = ValidationError::UnsupportedSchemaType {
        schema_type: "object".to_string(),
        parameter: "param".to_string(),
    };
    assert_error_severity(
        unsupported_type,
        ErrorSeverity::Error,
        "Unsupported schema type",
    );

    let missing_field = ValidationError::MissingSchemaField {
        field: "type".to_string(),
    };
    assert_error_severity(missing_field, ErrorSeverity::Error, "Missing schema field");
}

// --- disabled_tools tests ---

#[test]
fn test_default_all_enabled() {
    // Fresh registry — no tools should be disabled
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });
    assert!(registry.is_tool_enabled("git"));
    assert!(registry.disabled_tools().is_empty());
    assert_eq!(registry.list_tools().len(), 1);
}

#[test]
fn test_disabled_tool_excluded_from_list_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    registry.set_tool_enabled("git", false);

    assert!(!registry.is_tool_enabled("git"));
    assert!(registry.list_tools().is_empty());
    assert!(registry.list_tool_names().is_empty());
    assert_eq!(registry.iter_tools().count(), 0);
}

/// `tools.yaml` disabling must also remove the tool from the generated
/// `sah tool ...` command tree, not just from lookup.
///
/// The CLI builds its subcommands from `get_cli_categories` and
/// `get_tools_for_category`. When those ignored `disabled_tools`, a
/// disabled tool stayed in `sah tool --help` while `get_tool` refused it,
/// so the CLI advertised a command whose only possible outcome was
/// "Tool 'x' not found" — with `x` listed among the available tools.
#[test]
fn test_disabled_tool_excluded_from_cli_command_tree() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    // Enabled: it is a CLI command.
    assert!(registry.get_cli_categories().contains(&"git".to_string()));
    assert_eq!(registry.get_tools_for_category("git").len(), 1);
    assert_eq!(registry.get_cli_tools().len(), 1);

    registry.set_tool_enabled("git", false);

    // Disabled: the command must be gone, matching `get_tool`.
    assert!(
        registry.get_cli_categories().is_empty(),
        "a disabled tool must not contribute a `sah tool` category"
    );
    assert!(
        registry.get_tools_for_category("git").is_empty(),
        "a disabled tool must not appear under its category"
    );
    assert!(
        registry.get_cli_tools().is_empty(),
        "a disabled tool must not be listed as a CLI tool"
    );
    assert!(
        registry.get_tool("git").is_none(),
        "guards the premise: lookup already refuses a disabled tool"
    );
}

#[test]
fn test_disabled_tool_blocked_from_get_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    assert!(registry.get_tool("git").is_some());
    registry.set_tool_enabled("git", false);
    assert!(registry.get_tool("git").is_none());

    // Re-enabling should restore access
    registry.set_tool_enabled("git", true);
    assert!(registry.get_tool("git").is_some());
}

#[test]
fn test_set_all_disabled_then_enable_one() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });
    registry.register(MockTool {
        name: "kanban",
        description: "kanban tool",
    });

    // Disable everything
    registry.set_all_enabled(false);
    assert!(registry.list_tools().is_empty());

    // Re-enable only git
    registry.set_tool_enabled("git", true);

    let names = registry.list_tool_names();
    assert!(names.contains(&"git".to_string()));
    assert!(!names.contains(&"kanban".to_string()));
    assert_eq!(registry.list_tools().len(), 1);
}

#[test]
fn test_list_tools_always_load_meta() {
    // Every tool returned by list_tools() must have _meta with
    // "anthropic/alwaysLoad": true so Claude Code loads them eagerly.
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "alpha",
        description: "First tool",
    });
    registry.register(MockTool {
        name: "beta",
        description: "Second tool",
    });

    let tools = registry.list_tools();
    assert!(!tools.is_empty(), "expected at least one tool");

    for tool in &tools {
        let meta = tool
            .meta
            .as_ref()
            .unwrap_or_else(|| panic!("tool '{}' is missing _meta", tool.name));
        let always_load = meta
            .0
            .get("anthropic/alwaysLoad")
            .unwrap_or_else(|| panic!("tool '{}' _meta missing 'anthropic/alwaysLoad'", tool.name));
        assert_eq!(
            always_load,
            &serde_json::Value::Bool(true),
            "tool '{}' has wrong value for 'anthropic/alwaysLoad'",
            tool.name
        );
    }
}

#[test]
fn test_list_tools_deterministic_sorted_order() {
    // The rendered tools/list must be byte-identical across sibling
    // sessions so the KV prefix-reuse donor's LCP stays full. The registry
    // is HashMap-backed, so the emission point must sort by tool name to be
    // deterministic. Register in deliberately non-sorted insertion order;
    // assert the emitted order is sorted by name and stable across calls.
    //
    // Asserts exact ordered equality (not set-equality) so it fails under
    // HashMap iteration order / hash randomization.
    let names_in = [
        "kanban",
        "git",
        "web",
        "files",
        "code_context",
        "review",
        "ralph",
        "skill",
        "agent",
    ];
    let mut registry = ToolRegistry::new();
    for name in names_in {
        registry.register(MockTool {
            name,
            description: "tool",
        });
    }

    let mut expected = names_in.to_vec();
    expected.sort_unstable();

    let first: Vec<String> = registry
        .list_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    let second: Vec<String> = registry
        .list_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();

    // Two successive calls return an identical ordered sequence.
    assert_eq!(
        first, second,
        "successive list_tools() calls returned different orders"
    );
    // And the order is the expected stable ordering (sorted by name).
    assert_eq!(
        first, expected,
        "list_tools() order is not sorted by tool name"
    );

    // list_tools_filtered's other entry point (host composition) must be
    // sorted too — share the same emission guarantee.
    let for_host: Vec<String> = registry
        .list_tools_for_host(crate::mcp::host::Host::Claude)
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(
        for_host, expected,
        "list_tools_for_host() order is not sorted by tool name"
    );
}

/// The unified `files` tool is a `Replacement` for the native edit surface,
/// so the primary per-client serve advertises it to Claude (which gets
/// `Shared` + `Replacement`) but not to unknown clients (which get `Shared`
/// only). Regression for the bug where `files` was `Agent`-category and
/// therefore stripped from every host — including Claude — leaving Claude
/// with its native edit surface denied and no served replacement.
#[test]
fn files_replacement_served_to_claude_not_unknown() {
    use crate::mcp::host::Host;

    let mut registry = ToolRegistry::new();
    super::register_file_tools(&mut registry);

    let names_for = |host: Host| -> Vec<String> {
        registry
            .list_tools_for_host(host)
            .iter()
            .map(|t| t.name.to_string())
            .collect()
    };

    assert!(
        names_for(Host::Claude).iter().any(|n| n == "files"),
        "Claude must be served the `files` replacement"
    );
    assert!(
        !names_for(Host::Other).iter().any(|n| n == "files"),
        "unknown clients must not be served `files` via the primary serve"
    );
}

// --- ToolContext builder method tests ---

#[test]
fn test_tool_context_with_prompt_library() {
    use swissarmyhammer_templating::TemplateLibrary;
    use tokio::sync::RwLock;

    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    assert!(ctx.prompt_library.is_none());

    let library = Arc::new(RwLock::new(TemplateLibrary::new()));
    let ctx = ctx.with_prompt_library(library);
    assert!(ctx.prompt_library.is_some());
}

#[test]
fn test_tool_context_with_plan_sender() {
    use crate::mcp::plan_notifications::{PlanNotification, PlanSender};
    use tokio::sync::mpsc;

    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    assert!(ctx.plan_sender.is_none());

    let (tx, _rx) = mpsc::unbounded_channel::<PlanNotification>();
    let sender = PlanSender::new(tx);
    let ctx = ctx.with_plan_sender(sender);
    assert!(ctx.plan_sender.is_some());
}

#[test]
fn test_tool_context_with_tool_registry() {
    use tokio::sync::RwLock;

    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    assert!(ctx.tool_registry.is_none());

    let registry = Arc::new(RwLock::new(ToolRegistry::new()));
    let ctx = ctx.with_tool_registry(registry);
    assert!(ctx.tool_registry.is_some());
}

#[test]
fn test_tool_context_with_working_dir() {
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    assert!(ctx.working_dir.is_none());

    let dir = std::path::PathBuf::from("/tmp/test");
    let ctx = ctx.with_working_dir(dir.clone());
    assert_eq!(ctx.working_dir, Some(dir));
}

#[test]
fn test_session_root_prefers_working_dir() {
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());

    // With a working dir set, session_root returns it verbatim — never the
    // process CWD.
    let board_dir = std::path::PathBuf::from("/tmp/some-board");
    let ctx =
        ToolContext::new(tool_handlers, git_ops, agent_config).with_working_dir(board_dir.clone());
    assert_eq!(ctx.session_root(), board_dir);
}

#[tokio::test]
async fn test_tool_context_set_mcp_server() {
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);

    // Initially no server set
    let guard = ctx.mcp_server.read().await;
    assert!(guard.is_none());
}

// --- call_tool tests ---

#[tokio::test]
async fn test_call_tool_no_registry_returns_error() {
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);

    // No tool registry set — should return internal error
    let result = ctx.call_tool("anything", serde_json::json!({})).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        format!("{:?}", err).contains("tool registry not available"),
        "expected 'tool registry not available' in error, got: {:?}",
        err
    );
}

#[tokio::test]
async fn test_call_tool_missing_tool_returns_error() {
    use tokio::sync::RwLock;

    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let registry = Arc::new(RwLock::new(ToolRegistry::new()));
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config).with_tool_registry(registry);

    let result = ctx
        .call_tool("nonexistent_tool", serde_json::json!({}))
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("not found in registry"),
        "expected 'not found in registry' in error, got: {msg}"
    );
}

#[tokio::test]
async fn test_call_tool_non_object_params_returns_error() {
    use tokio::sync::RwLock;

    let mut inner = ToolRegistry::new();
    inner.register(MockTool {
        name: "echo_tool",
        description: "Echoes input",
    });
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let registry = Arc::new(RwLock::new(inner));
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config).with_tool_registry(registry);

    // Pass an array instead of object — should fail
    let result = ctx
        .call_tool("echo_tool", serde_json::json!(["not", "an", "object"]))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_call_tool_success() {
    use tokio::sync::RwLock;

    let mut inner = ToolRegistry::new();
    inner.register(MockTool {
        name: "echo_tool",
        description: "Echoes input",
    });
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let registry = Arc::new(RwLock::new(inner));
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config).with_tool_registry(registry);

    let result = ctx.call_tool("echo_tool", serde_json::json!({})).await;
    assert!(result.is_ok());
    let call_result = result.unwrap();
    assert_eq!(call_result.is_error, Some(false));
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
    // FilesTool → category "file"
    // GitStatusTool → category "git"
    registry.register(KanbanTool);
    registry.register(FilesTool);
    registry.register(GitStatusTool);

    let categories = registry.get_cli_categories();
    // Should be sorted
    let mut sorted = categories.clone();
    sorted.sort();
    assert_eq!(categories, sorted);
    assert!(categories.contains(&"kanban".to_string()));
    assert!(categories.contains(&"file".to_string()));
    assert!(categories.contains(&"git".to_string()));
}

#[test]
fn test_get_tools_for_category_returns_correct_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);
    registry.register(FilesTool);
    registry.register(GitStatusTool);

    let kanban_tools = registry.get_tools_for_category("kanban");
    assert_eq!(kanban_tools.len(), 1);
    assert_eq!(<dyn McpTool as McpTool>::name(kanban_tools[0]), "kanban");

    let file_tools = registry.get_tools_for_category("file");
    assert_eq!(file_tools.len(), 1);
    assert_eq!(<dyn McpTool as McpTool>::name(file_tools[0]), "files");
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
fn test_get_cli_tools_empty_when_all_hidden() {
    // Tools without a cli_category won't appear in get_cli_tools
    let mut registry = ToolRegistry::new();
    registry.register(UnknownCategoryTool);
    registry.register(NoUnderscoreTool);

    let cli_tools = registry.get_cli_tools();
    assert!(cli_tools.is_empty());
}

// --- send_mcp_log tests ---

#[tokio::test]
async fn test_send_mcp_log_no_peer_is_noop() {
    use rmcp::model::LoggingLevel;

    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    // No peer — send_mcp_log should be a no-op and not panic
    send_mcp_log(&ctx, LoggingLevel::Info, "test", "hello".to_string()).await;
}

// --- create_fully_registered_tool_registry tests ---
//
// `create_fully_registered_tool_registry` registers the real tools, and a
// tool constructor may root state under the process CWD — the crate
// directory under `cargo nextest`. A `CurrentDirGuard` pins the CWD to a
// throwaway temp dir for the call so these tests don't leave stray
// directories behind in the crate source tree.

#[tokio::test]
async fn test_create_fully_registered_tool_registry_is_non_empty() {
    let cwd_dir = tempfile::TempDir::new().expect("temp dir for isolated tool state");
    let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(cwd_dir.path())
        .expect("chdir guard");
    let registry = create_fully_registered_tool_registry().await;
    assert!(
        !registry.is_empty(),
        "fully registered registry should not be empty"
    );
}

#[tokio::test]
async fn test_create_fully_registered_tool_registry_has_cli_tools() {
    let cwd_dir = tempfile::TempDir::new().expect("temp dir for isolated tool state");
    let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(cwd_dir.path())
        .expect("chdir guard");
    let registry = create_fully_registered_tool_registry().await;
    let cli_tools = registry.get_cli_tools();
    assert!(
        !cli_tools.is_empty(),
        "fully registered registry should have CLI tools"
    );
}

#[tokio::test]
async fn test_create_fully_registered_tool_registry_validation() {
    let cwd_dir = tempfile::TempDir::new().expect("temp dir for isolated tool state");
    let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(cwd_dir.path())
        .expect("chdir guard");
    let registry = create_fully_registered_tool_registry().await;
    let report = registry.validate_all_tools();
    // All registered tools should pass validation
    assert!(
        report.is_valid(),
        "all tools in fully registered registry should be valid: {:?}",
        report.errors
    );
}

/// The `question` tool was removed: neither the fully registered registry
/// nor the known-tool-name list may still carry it.
#[tokio::test]
async fn question_tool_is_not_registered() {
    let cwd_dir = tempfile::TempDir::new().expect("temp dir for isolated tool state");
    let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(cwd_dir.path())
        .expect("chdir guard");
    let registry = create_fully_registered_tool_registry().await;
    assert!(
        registry.get_tool("question").is_none(),
        "fully registered registry must not register a `question` tool"
    );
    assert!(
        !crate::mcp::tool_config::KNOWN_TOOL_NAMES.contains(&"question"),
        "KNOWN_TOOL_NAMES must not list the removed `question` tool"
    );
}

// --- category() tests ---

/// Mock tool that reports the `Agent` category.
struct MockAgentTool;
impl_default_doctorable!(MockAgentTool);
impl_empty_initializable!(MockAgentTool);

#[async_trait::async_trait]
impl McpTool for MockAgentTool {
    fn name(&self) -> &'static str {
        "files"
    }
    fn description(&self) -> &'static str {
        "An agent tool"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }
    async fn execute(
        &self,
        _args: serde_json::Map<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        Ok(BaseToolImpl::create_success_response("agent"))
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Agent
    }
}

#[test]
fn test_category_override_reports_agent() {
    let tool = MockAgentTool;
    assert_eq!(McpTool::category(&tool), ToolCategory::Agent);
}

/// Covers the derived `PartialEq`/`Ne` equality semantics of `ToolCategory`:
/// two `Replacement` variants with the same native name compare equal, and a
/// `Replacement` is never equal to an `Agent`.
#[test]
fn test_tool_category_equality_semantics() {
    assert_eq!(
        ToolCategory::Replacement { native: "Bash" },
        ToolCategory::Replacement { native: "Bash" }
    );
    assert_ne!(
        ToolCategory::Replacement { native: "Bash" },
        ToolCategory::Agent
    );
}

// --- get_tool_by_cli_name tests ---

#[test]
fn test_get_tool_by_cli_name_found() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);

    let tool = registry.get_tool_by_cli_name("kanban", "kanban");
    assert!(tool.is_some());
    assert_eq!(<dyn McpTool as McpTool>::name(tool.unwrap()), "kanban");
}

#[test]
fn test_get_tool_by_cli_name_not_found() {
    let mut registry = ToolRegistry::new();
    registry.register(KanbanTool);

    assert!(registry
        .get_tool_by_cli_name("kanban", "nonexistent")
        .is_none());
    assert!(registry
        .get_tool_by_cli_name("nonexistent", "kanban")
        .is_none());
}

#[test]
fn test_get_tool_by_cli_name_git_status() {
    let mut registry = ToolRegistry::new();
    registry.register(GitStatusTool);

    let tool = registry.get_tool_by_cli_name("git", "status");
    assert!(tool.is_some());
}

// --- matches_cli_criteria tests ---

#[test]
fn test_matches_cli_criteria_positive() {
    let registry = ToolRegistry::new();
    let tool = KanbanTool;
    assert!(registry.matches_cli_criteria(&tool, "kanban", "kanban"));
}

#[test]
fn test_matches_cli_criteria_wrong_category() {
    let registry = ToolRegistry::new();
    let tool = KanbanTool;
    assert!(!registry.matches_cli_criteria(&tool, "wrong", "kanban"));
}

#[test]
fn test_matches_cli_criteria_wrong_name() {
    let registry = ToolRegistry::new();
    let tool = KanbanTool;
    assert!(!registry.matches_cli_criteria(&tool, "kanban", "wrong"));
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
    assert!(!registry.matches_cli_criteria(&tool, "kanban", "hidden"));
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

// --- category default test ---

#[test]
fn test_default_category_is_shared() {
    let tool = MockTool {
        name: "test",
        description: "test",
    };
    assert_eq!(McpTool::category(&tool), ToolCategory::Shared);
}

#[test]
fn test_default_operations_empty() {
    let tool = MockTool {
        name: "test",
        description: "test",
    };
    assert!(tool.operations().is_empty());
}

// --- ValidationError Display tests ---

#[test]
fn test_validation_error_invalid_schema_display() {
    let err = ValidationError::InvalidSchema {
        message: "schema is broken".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("invalid schema structure"));
    assert!(s.contains("schema is broken"));
}

#[test]
fn test_validation_error_missing_field_display() {
    let err = ValidationError::MissingSchemaField {
        field: "properties".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("missing required schema field"));
    assert!(s.contains("properties"));
}

#[test]
fn test_validation_error_unsupported_type_display() {
    let err = ValidationError::UnsupportedSchemaType {
        schema_type: "object".to_string(),
        parameter: "nested".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("unsupported schema type"));
    assert!(s.contains("object"));
    assert!(s.contains("nested"));
}

// --- ToolValidationError Display tests ---

#[test]
fn test_tool_validation_error_missing_category_display() {
    let err = ToolValidationError::MissingCliCategory {
        tool_name: "my_tool".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("my_tool"));
    assert!(s.contains("missing CLI category"));
}

#[test]
fn test_tool_validation_error_invalid_cli_name_display() {
    let err = ToolValidationError::InvalidCliName {
        tool_name: "my_tool".to_string(),
        cli_name: "".to_string(),
        reason: "CLI name cannot be empty".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("my_tool"));
    assert!(s.contains("invalid CLI name"));
}

#[test]
fn test_tool_validation_error_invalid_description_display() {
    let err = ToolValidationError::InvalidDescription {
        tool_name: "my_tool".to_string(),
        reason: "too short".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("my_tool"));
    assert!(s.contains("invalid description"));
}

#[test]
fn test_tool_validation_error_name_conflict_display() {
    let err = ToolValidationError::NameConflict {
        tool_name: "tool_a".to_string(),
        conflicting_tool: "tool_b".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("tool_a"));
    assert!(s.contains("tool_b"));
    assert!(s.contains("conflicts"));
}

// --- ToolValidationReport tests ---

#[test]
fn test_validation_report_valid_summary() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);

    let report = registry.validate_all_tools();
    assert!(report.is_valid());
    let summary = report.summary();
    assert!(summary.contains("All"));
    assert!(summary.contains("1 tools"));
}

// --- SchemaValidator edge cases ---

#[test]
fn test_schema_validator_non_object_schema() {
    let schema = serde_json::json!("not an object");
    let result = SchemaValidator::validate_schema(&schema);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::InvalidSchema { .. }
    ));
}

#[test]
fn test_schema_validator_missing_properties() {
    let schema = serde_json::json!({"type": "object"});
    let result = SchemaValidator::validate_schema(&schema);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ValidationError::MissingSchemaField { .. }
    ));
}

#[test]
fn test_schema_validator_supported_types() {
    for type_name in &["string", "integer", "number", "boolean", "array"] {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "param": {"type": type_name}
            }
        });
        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_ok(), "type '{}' should be valid", type_name);
    }
}

#[test]
fn test_schema_validator_unsupported_type_null() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"type": "null"}
        }
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert!(result.is_err());
}

#[test]
fn test_schema_validator_property_without_type() {
    // Properties without a "type" field should pass validation
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"description": "no type specified"}
        }
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert!(result.is_ok());
}

#[test]
fn test_schema_validator_property_not_object() {
    // A property value that is not an object should pass (graceful)
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "param": "just a string"
        }
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert!(result.is_ok());
}

#[test]
fn test_schema_validator_properties_not_object() {
    // properties field that isn't an object
    let schema = serde_json::json!({
        "type": "object",
        "properties": "not an object"
    });
    let result = SchemaValidator::validate_schema(&schema);
    // validate_properties returns Ok when properties isn't an object
    assert!(result.is_ok());
}

// --- ToolValidationSeverity tests ---

#[test]
fn test_validation_severity_equality() {
    assert_eq!(
        ToolValidationSeverity::Warning,
        ToolValidationSeverity::Warning
    );
    assert_eq!(ToolValidationSeverity::Error, ToolValidationSeverity::Error);
    assert_eq!(
        ToolValidationSeverity::Critical,
        ToolValidationSeverity::Critical
    );
    assert_ne!(
        ToolValidationSeverity::Warning,
        ToolValidationSeverity::Error
    );
}

// --- ToolValidationWarning tests ---

#[test]
fn test_validation_warning_fields() {
    let warning = ToolValidationWarning {
        tool_name: "test_tool".to_string(),
        error: ToolValidationError::MissingCliCategory {
            tool_name: "test_tool".to_string(),
        },
        severity: ToolValidationSeverity::Warning,
    };
    assert_eq!(warning.tool_name, "test_tool");
    assert_eq!(warning.severity, ToolValidationSeverity::Warning);
}

// --- ToolContext session_id tests ---

#[test]
fn test_tool_context_has_unique_session_id() {
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx1 = ToolContext::new(tool_handlers.clone(), git_ops.clone(), agent_config.clone());
    let ctx2 = ToolContext::new(tool_handlers, git_ops, agent_config);

    // Each context should have a unique session_id
    assert!(!ctx1.session_id.is_empty());
    assert!(!ctx2.session_id.is_empty());
    assert_ne!(ctx1.session_id, ctx2.session_id);
}

// --- ToolContext with_peer test ---

#[test]
fn test_tool_context_with_peer_sets_peer() {
    let tool_handlers = Arc::new(ToolHandlers::new());
    let git_ops = Arc::new(tokio::sync::Mutex::new(None));
    let agent_config = Arc::new(swissarmyhammer_config::model::ChatModelConfig::default());
    let ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    assert!(ctx.peer.is_none());
    // We can't easily create a real Peer, but we verify the initial state
}

// --- set_all_enabled(true) clears all ---

#[test]
fn test_set_all_enabled_true_clears_disabled() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "tool_a",
        description: "A",
    });
    registry.register(MockTool {
        name: "tool_b",
        description: "B",
    });

    registry.set_all_enabled(false);
    assert_eq!(registry.list_tool_names().len(), 0);

    registry.set_all_enabled(true);
    assert_eq!(registry.list_tool_names().len(), 2);
    assert!(registry.disabled_tools().is_empty());
}

// --- iter_tools respects disabled ---

#[test]
fn test_iter_tools_respects_disabled() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "keep_me",
        description: "kept",
    });
    registry.register(MockTool {
        name: "hide_me",
        description: "hidden",
    });

    registry.set_tool_enabled("hide_me", false);
    let names: Vec<&str> = registry
        .iter_tools()
        .map(<dyn McpTool as McpTool>::name)
        .collect();
    assert!(names.contains(&"keep_me"));
    assert!(!names.contains(&"hide_me"));
}

// --- cli_about with empty description ---

#[test]
fn test_cli_about_empty_description() {
    struct EmptyDescTool;
    impl_default_doctorable!(EmptyDescTool);
    impl_empty_initializable!(EmptyDescTool);

    #[async_trait::async_trait]
    impl McpTool for EmptyDescTool {
        fn name(&self) -> &'static str {
            "empty_desc"
        }
        fn description(&self) -> &'static str {
            ""
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(
            &self,
            _args: serde_json::Map<String, serde_json::Value>,
            _ctx: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("empty"))
        }
    }

    let tool = EmptyDescTool;
    // Empty description — cli_about returns None because lines() yields nothing
    let about = tool.cli_about();
    assert_eq!(about, None);
}

// --- cli_about with only headers ---

#[test]
fn test_cli_about_only_headers() {
    struct HeaderOnlyTool;
    impl_default_doctorable!(HeaderOnlyTool);
    impl_empty_initializable!(HeaderOnlyTool);

    #[async_trait::async_trait]
    impl McpTool for HeaderOnlyTool {
        fn name(&self) -> &'static str {
            "header_only"
        }
        fn description(&self) -> &'static str {
            "# Header\n## Subheader\n### Another"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(
            &self,
            _args: serde_json::Map<String, serde_json::Value>,
            _ctx: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("headers"))
        }
    }

    let tool = HeaderOnlyTool;
    // All lines are headers, so cli_about falls back to first line
    let about = tool.cli_about();
    assert_eq!(about, Some("# Header"));
}

// --- ToolRegistry::default() test ---

#[test]
fn test_tool_registry_default() {
    let registry = ToolRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert!(registry.disabled_tools().is_empty());
}

// --- Tool re-registration replaces existing ---

#[test]
fn test_register_replaces_existing_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "dupe",
        description: "first",
    });
    registry.register(MockTool {
        name: "dupe",
        description: "second",
    });

    // Should still have 1 tool, not 2
    assert_eq!(registry.len(), 1);
    let tool = registry.get_tool("dupe").unwrap();
    assert_eq!(tool.description(), "second");
}

// --- validate_cli_requirements_for_tool on hidden tool ---

#[test]
fn test_validate_cli_requirements_hidden_tool_skipped() {
    let registry = ToolRegistry::new();
    let tool = HiddenTool;
    let errors = registry.validate_cli_requirements_for_tool(&tool);
    assert!(errors.is_empty());
}
