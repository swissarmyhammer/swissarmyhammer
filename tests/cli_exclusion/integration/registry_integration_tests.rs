//! Integration tests for tool registry with CLI exclusion detection
//!
//! These tests validate the integration between the ToolRegistry and CLI exclusion
//! detection system, ensuring proper registration and detection of excluded tools.

use std::sync::Arc;
use swissarmyhammer_tools::cli::{CliExclusionDetector, CliExclusionMarker};
use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use async_trait::async_trait;
use rmcp::model::{CallToolResult, RawContent, RawTextContent};
use rmcp::Error as McpError;
use serde_json::Value;
use super::super::common::test_utils::{ExcludedMockTool, IncludedMockTool, assert_exclusion_detection};

/// Integration test for ToolRegistry extension methods with CLI exclusion
#[tokio::test]
async fn test_tool_registry_exclusion_integration() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register a mix of excluded and included tools
    registry.register(Box::new(ExcludedMockTool::new("workflow_tool", "MCP workflow orchestration")));
    registry.register(Box::new(ExcludedMockTool::new("internal_tool", "Internal state management")));
    registry.register(Box::new(IncludedMockTool::new("user_tool")));
    registry.register(Box::new(IncludedMockTool::new("api_tool")));

    // Get CLI exclusion detector from registry
    let detector = registry.as_exclusion_detector();

    // Test exclusion detection
    assert_exclusion_detection(
        &detector,
        &["workflow_tool", "internal_tool"],
        &["user_tool", "api_tool"],
    );

    // Test metadata retrieval
    let all_metadata = detector.get_all_tool_metadata();
    assert_eq!(all_metadata.len(), 4);

    let excluded_metadata: Vec<_> = all_metadata
        .iter()
        .filter(|m| m.is_cli_excluded)
        .collect();
    let included_metadata: Vec<_> = all_metadata
        .iter()
        .filter(|m| !m.is_cli_excluded)
        .collect();

    assert_eq!(excluded_metadata.len(), 2);
    assert_eq!(included_metadata.len(), 2);

    // Verify reasons are captured properly
    for metadata in excluded_metadata {
        assert!(metadata.exclusion_reason.is_some());
    }

    for metadata in included_metadata {
        assert!(metadata.exclusion_reason.is_none());
    }
}

/// Test registry with real MCP tool implementations that use CLI exclusion
#[tokio::test]
async fn test_registry_with_real_excluded_tools() {
    let _env = IsolatedTestEnvironment::new();

    // Create a real tool that implements CLI exclusion
    #[derive(Default)]
    struct RealWorkflowTool;

    impl CliExclusionMarker for RealWorkflowTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("Real workflow tool - designed for MCP orchestration only")
        }
    }

    #[async_trait]
    impl McpTool for RealWorkflowTool {
        fn name(&self) -> &'static str {
            "real_workflow_tool"
        }

        fn description(&self) -> &'static str {
            "A real workflow management tool for MCP operations"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "description": "Workflow identifier"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["start", "pause", "resume", "stop"],
                        "description": "Workflow action to perform"
                    }
                },
                "required": ["workflow_id", "action"]
            })
        }

        async fn execute(
            &self,
            arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            let workflow_id = arguments
                .get("workflow_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let action = arguments
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            Ok(CallToolResult {
                content: vec![RawContent::Text(RawTextContent {
                    text: format!("Workflow {} action: {}", workflow_id, action),
                })],
                is_error: false,
                meta: None,
            })
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    // Create a real included tool
    #[derive(Default)]
    struct RealUserTool;

    #[async_trait]
    impl McpTool for RealUserTool {
        fn name(&self) -> &'static str {
            "real_user_tool"
        }

        fn description(&self) -> &'static str {
            "A real user-facing tool suitable for CLI"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "User name"
                    },
                    "email": {
                        "type": "string",
                        "description": "User email address"
                    }
                },
                "required": ["name"]
            })
        }

        async fn execute(
            &self,
            arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            let name = arguments
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            Ok(CallToolResult {
                content: vec![RawContent::Text(RawTextContent {
                    text: format!("Hello, {}!", name),
                })],
                is_error: false,
                meta: None,
            })
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    // Register both tools
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(RealWorkflowTool::default()));
    registry.register(Box::new(RealUserTool::default()));

    // Test that the registry can execute both tools
    let context = registry.create_test_context().await;

    // Execute the excluded tool (should work in MCP context)
    let workflow_args = serde_json::Map::from_iter(vec![
        ("workflow_id".to_string(), Value::String("test-workflow".to_string())),
        ("action".to_string(), Value::String("start".to_string())),
    ]);
    
    let workflow_result = registry
        .get_tool("real_workflow_tool")
        .unwrap()
        .execute(workflow_args, &context)
        .await;
    assert!(workflow_result.is_ok());

    // Execute the included tool
    let user_args = serde_json::Map::from_iter(vec![
        ("name".to_string(), Value::String("Test User".to_string())),
    ]);
    
    let user_result = registry
        .get_tool("real_user_tool")
        .unwrap()
        .execute(user_args, &context)
        .await;
    assert!(user_result.is_ok());

    // Test CLI exclusion detection
    let detector = registry.as_exclusion_detector();
    assert!(detector.is_cli_excluded("real_workflow_tool"));
    assert!(!detector.is_cli_excluded("real_user_tool"));

    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();
    
    assert_eq!(excluded_tools, vec!["real_workflow_tool"]);
    assert_eq!(eligible_tools, vec!["real_user_tool"]);
}

/// Test registry behavior with large numbers of tools
#[tokio::test]
async fn test_registry_scalability_with_exclusions() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register many tools with a pattern of exclusions
    for i in 0..1000 {
        let tool_name = format!("tool_{}", i);
        
        if i % 3 == 0 {
            // Every third tool is excluded
            registry.register(Box::new(ExcludedMockTool::new(
                tool_name,
                "Scalability test exclusion",
            )));
        } else {
            // Other tools are included
            registry.register(Box::new(IncludedMockTool::new(tool_name)));
        }
    }

    assert_eq!(registry.len(), 1000);

    // Test exclusion detection performance
    let start_time = std::time::Instant::now();
    let detector = registry.as_exclusion_detector();
    let detector_creation_time = start_time.elapsed();

    // Detector creation should be reasonably fast
    assert!(
        detector_creation_time.as_millis() < 1000,
        "Detector creation took too long: {}ms",
        detector_creation_time.as_millis()
    );

    // Test bulk query performance
    let query_start = std::time::Instant::now();
    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();
    let query_time = query_start.elapsed();

    // Bulk queries should be fast
    assert!(
        query_time.as_millis() < 500,
        "Bulk queries took too long: {}ms",
        query_time.as_millis()
    );

    // Verify counts are correct
    assert_eq!(excluded_tools.len(), 334); // Every 3rd tool (0, 3, 6, ... 999) = 334 tools
    assert_eq!(eligible_tools.len(), 666); // Remaining tools
    assert_eq!(excluded_tools.len() + eligible_tools.len(), 1000);

    // Test individual query performance
    let individual_start = std::time::Instant::now();
    for i in (0..100).step_by(10) {
        let tool_name = format!("tool_{}", i);
        let is_excluded = detector.is_cli_excluded(&tool_name);
        let should_be_excluded = i % 3 == 0;
        assert_eq!(is_excluded, should_be_excluded);
    }
    let individual_time = individual_start.elapsed();

    // Individual queries should be very fast
    assert!(
        individual_time.as_micros() < 10000, // 10ms for 10 queries
        "Individual queries took too long: {}μs",
        individual_time.as_micros()
    );
}

/// Test registry integration with concurrent access
#[tokio::test]
async fn test_concurrent_registry_access() {
    use std::sync::Arc;
    use tokio::task;

    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register tools
    for i in 0..50 {
        if i % 2 == 0 {
            registry.register(Box::new(ExcludedMockTool::new(
                format!("excluded_{}", i),
                "Concurrent test exclusion",
            )));
        } else {
            registry.register(Box::new(IncludedMockTool::new(format!("included_{}", i))));
        }
    }

    let detector = Arc::new(registry.as_exclusion_detector());

    // Spawn multiple concurrent tasks accessing the detector
    let mut tasks = Vec::new();
    
    for task_id in 0..10 {
        let detector_clone = detector.clone();
        let task = task::spawn(async move {
            // Each task performs different types of queries
            match task_id % 3 {
                0 => {
                    // Test individual exclusion queries
                    for i in 0..50 {
                        let tool_name = if i % 2 == 0 {
                            format!("excluded_{}", i)
                        } else {
                            format!("included_{}", i)
                        };
                        let is_excluded = detector_clone.is_cli_excluded(&tool_name);
                        let should_be_excluded = i % 2 == 0;
                        assert_eq!(is_excluded, should_be_excluded);
                    }
                }
                1 => {
                    // Test bulk excluded query
                    let excluded = detector_clone.get_excluded_tools();
                    assert_eq!(excluded.len(), 25); // Half of 50 tools
                }
                2 => {
                    // Test bulk eligible query
                    let eligible = detector_clone.get_cli_eligible_tools();
                    assert_eq!(eligible.len(), 25); // Half of 50 tools
                }
                _ => unreachable!(),
            }
            
            task_id
        });
        
        tasks.push(task);
    }

    // Wait for all tasks to complete
    let results = futures::future::join_all(tasks).await;
    
    // All tasks should complete successfully
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Task {} failed", i);
        assert_eq!(result.unwrap(), i);
    }
}

/// Test registry error handling with malformed tools
#[tokio::test] 
async fn test_registry_error_handling() {
    let _env = IsolatedTestEnvironment::new();

    // Create a tool with an invalid schema (but valid Rust implementation)
    #[derive(Default)]
    struct MalformedTool;

    #[async_trait]
    impl McpTool for MalformedTool {
        fn name(&self) -> &'static str {
            "malformed_tool"
        }

        fn description(&self) -> &'static str {
            "Tool with invalid schema for testing error handling"
        }

        fn schema(&self) -> Value {
            // Return an invalid schema structure
            serde_json::json!({
                "type": "not_a_valid_type",
                "properties": "this_should_be_an_object",
                "required": 123 // Should be an array
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response(
                "Malformed tool executed successfully"
            ))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MalformedTool::default()));
    registry.register(Box::new(IncludedMockTool::new("good_tool")));

    // Registry should handle malformed tools gracefully
    assert_eq!(registry.len(), 2);
    
    // CLI exclusion detection should still work for valid tools
    let detector = registry.as_exclusion_detector();
    assert!(!detector.is_cli_excluded("good_tool"));
    assert!(!detector.is_cli_excluded("malformed_tool")); // Default to not excluded
    
    // Tool should still be executable in MCP context
    let context = registry.create_test_context().await;
    let result = registry
        .get_tool("malformed_tool")
        .unwrap()
        .execute(serde_json::Map::new(), &context)
        .await;
    
    assert!(result.is_ok());
}

/// Test that CLI exclusion doesn't affect MCP tool functionality
#[tokio::test]
async fn test_exclusion_does_not_affect_mcp_functionality() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register an excluded tool
    registry.register(Box::new(ExcludedMockTool::new("excluded_tool", "Test exclusion")));

    // The tool should still be available for MCP operations
    assert!(registry.get_tool("excluded_tool").is_some());
    
    let tools = registry.list_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "excluded_tool");

    // The tool should still be executable
    let context = registry.create_test_context().await;
    let result = registry
        .get_tool("excluded_tool")
        .unwrap()
        .execute(serde_json::Map::new(), &context)
        .await;
    
    assert!(result.is_ok());

    // Only CLI generation should be affected by exclusion
    let detector = registry.as_exclusion_detector();
    assert!(detector.is_cli_excluded("excluded_tool"));
    assert!(detector.get_cli_eligible_tools().is_empty());
    assert_eq!(detector.get_excluded_tools(), vec!["excluded_tool"]);
}

/// Test registry extension method consistency
#[tokio::test]
async fn test_registry_extension_method_consistency() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register tools
    registry.register(Box::new(ExcludedMockTool::new("excluded1", "Reason 1")));
    registry.register(Box::new(ExcludedMockTool::new("excluded2", "Reason 2")));
    registry.register(Box::new(IncludedMockTool::new("included1")));
    registry.register(Box::new(IncludedMockTool::new("included2")));

    // Get detector multiple times
    let detector1 = registry.as_exclusion_detector();
    let detector2 = registry.as_exclusion_detector();

    // Results should be consistent between different detector instances
    assert_eq!(detector1.get_excluded_tools(), detector2.get_excluded_tools());
    assert_eq!(detector1.get_cli_eligible_tools(), detector2.get_cli_eligible_tools());

    let metadata1 = detector1.get_all_tool_metadata();
    let metadata2 = detector2.get_all_tool_metadata();
    assert_eq!(metadata1.len(), metadata2.len());

    // Individual queries should be consistent
    for tool_name in registry.list_tool_names() {
        assert_eq!(
            detector1.is_cli_excluded(&tool_name),
            detector2.is_cli_excluded(&tool_name)
        );
    }
}