//! Registration, lookup, execution, categories and base tool helpers.

use super::fixtures::MockTool;
use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::*;
use rmcp::model::RawContent;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_config::model::ChatModelConfig;

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

// --- create_fully_registered_tool_registry tests ---
//
// `create_fully_registered_tool_registry` registers the real tools, and a
// tool constructor may root state under the process CWD — the crate
// directory under `cargo nextest`. A `CurrentDirGuard` pins the CWD to a
// throwaway temp dir for the call so these tests don't leave stray
// directories behind in the crate source tree.

/// Build the fully registered registry with the CWD pinned to a throwaway temp
/// dir for the duration of the call.
async fn isolated_fully_registered_registry() -> ToolRegistry {
    let cwd_dir = tempfile::TempDir::new().expect("temp dir for isolated tool state");
    let _guard = swissarmyhammer_common::test_utils::CurrentDirGuard::new(cwd_dir.path())
        .expect("chdir guard");
    create_fully_registered_tool_registry().await
}

#[tokio::test]
async fn test_create_fully_registered_tool_registry_is_non_empty() {
    let registry = isolated_fully_registered_registry().await;
    assert!(
        !registry.is_empty(),
        "fully registered registry should not be empty"
    );
}

#[tokio::test]
async fn test_create_fully_registered_tool_registry_has_cli_tools() {
    let registry = isolated_fully_registered_registry().await;
    let cli_tools = registry.get_cli_tools();
    assert!(
        !cli_tools.is_empty(),
        "fully registered registry should have CLI tools"
    );
}

#[tokio::test]
async fn test_create_fully_registered_tool_registry_validation() {
    let registry = isolated_fully_registered_registry().await;
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
    let registry = isolated_fully_registered_registry().await;
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

#[test]
fn test_registered_files_tool_reports_replacement_category() {
    let mut registry = ToolRegistry::new();
    register_file_tools(&mut registry);

    let tool = registry
        .get_tool("files")
        .expect("files tool is registered");

    assert_eq!(
        McpTool::category(tool),
        ToolCategory::Replacement { native: "Edit" }
    );
    assert_ne!(McpTool::category(tool), ToolCategory::Agent);
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

#[test]
fn test_tool_category_default_is_shared() {
    assert_eq!(ToolCategory::default(), ToolCategory::Shared);
}
