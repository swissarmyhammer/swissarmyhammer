//! End-to-end tests for complete CLI exclusion workflow
//!
//! These tests validate the entire workflow from tool registration with attributes
//! through CLI generation, ensuring all components work together seamlessly.

use std::sync::Arc;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
use swissarmyhammer_tools::cli::{CliExclusionMarker, ToolCliMetadata};
use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use async_trait::async_trait;
use rmcp::model::{CallToolResult, RawContent, RawTextContent};
use rmcp::Error as McpError;
use serde_json::Value;

/// Complete end-to-end test from tool definition to CLI command generation
#[tokio::test]
async fn test_complete_workflow_with_exclusion_attributes() {
    let _env = IsolatedTestEnvironment::new();

    // Step 1: Define tools with CLI exclusion attributes and traits

    /// Workflow management tool - should be excluded from CLI
    #[sah_marker_macros::cli_exclude]
    #[derive(Default)]
    struct WorkflowManagerTool;

    impl CliExclusionMarker for WorkflowManagerTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("Workflow management requires MCP protocol coordination")
        }
    }

    #[async_trait]
    impl McpTool for WorkflowManagerTool {
        fn name(&self) -> &'static str {
            "workflow_manager"
        }

        fn description(&self) -> &'static str {
            "Manages complex workflows requiring MCP coordination"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "description": "Workflow identifier"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["create", "start", "stop", "status"],
                        "description": "Operation to perform"
                    },
                    "parameters": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Workflow-specific parameters"
                    }
                },
                "required": ["workflow_id", "operation"]
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
            let operation = arguments
                .get("operation") 
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            Ok(CallToolResult {
                content: vec![RawContent::Text(RawTextContent {
                    text: format!("Workflow '{}' operation '{}' completed", workflow_id, operation),
                })],
                is_error: false,
                meta: None,
            })
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    /// User memo tool - should be included in CLI
    #[derive(Default)]
    struct UserMemoTool;

    #[async_trait] 
    impl McpTool for UserMemoTool {
        fn name(&self) -> &'static str {
            "user_memo"
        }

        fn description(&self) -> &'static str {
            "Create and manage user memos - suitable for CLI usage"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Memo title"
                    },
                    "content": {
                        "type": "string",
                        "description": "Memo content"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional tags"
                    }
                },
                "required": ["title", "content"]
            })
        }

        async fn execute(
            &self,
            arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            let title = arguments
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled");
            let content = arguments
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            Ok(CallToolResult {
                content: vec![RawContent::Text(RawTextContent {
                    text: format!("Created memo '{}': {}", title, content),
                })],
                is_error: false,
                meta: None,
            })
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    /// Internal state tool - should be excluded from CLI
    #[sah_marker_macros::cli_exclude]
    #[derive(Default)]
    struct InternalStateTool;

    impl CliExclusionMarker for InternalStateTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("Internal state management - not for direct user access")
        }
    }

    #[async_trait]
    impl McpTool for InternalStateTool {
        fn name(&self) -> &'static str {
            "internal_state"
        }

        fn description(&self) -> &'static str {
            "Manages internal application state"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "state_key": {"type": "string"},
                    "state_value": {"type": "string"}
                },
                "required": ["state_key"]
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("State updated"))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    /// File utility tool - should be included in CLI
    #[derive(Default)]
    struct FileUtilityTool;

    #[async_trait]
    impl McpTool for FileUtilityTool {
        fn name(&self) -> &'static str {
            "file_utility"
        }

        fn description(&self) -> &'static str {
            "File operations suitable for CLI usage"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["read", "write", "delete", "list"],
                        "description": "File operation"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content for write operations"
                    }
                },
                "required": ["path", "operation"]
            })
        }

        async fn execute(
            &self,
            arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            let path = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let operation = arguments
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            Ok(CallToolResult {
                content: vec![RawContent::Text(RawTextContent {
                    text: format!("File operation '{}' on '{}' completed", operation, path),
                })],
                is_error: false,
                meta: None,
            })
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    // Step 2: Register all tools in the registry
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(WorkflowManagerTool::default()));
    registry.register(Box::new(UserMemoTool::default()));
    registry.register(Box::new(InternalStateTool::default()));
    registry.register(Box::new(FileUtilityTool::default()));

    // Step 3: Verify all tools are registered and functional in MCP context
    assert_eq!(registry.len(), 4);
    let context = registry.create_test_context().await;

    // Test MCP execution for all tools (exclusion shouldn't affect MCP functionality)
    let workflow_result = registry
        .get_tool("workflow_manager")
        .unwrap()
        .execute(
            serde_json::Map::from_iter(vec![
                ("workflow_id".to_string(), Value::String("test-workflow".to_string())),
                ("operation".to_string(), Value::String("start".to_string())),
            ]),
            &context,
        )
        .await;
    assert!(workflow_result.is_ok());

    let memo_result = registry
        .get_tool("user_memo")
        .unwrap()
        .execute(
            serde_json::Map::from_iter(vec![
                ("title".to_string(), Value::String("Test Memo".to_string())),
                ("content".to_string(), Value::String("Test content".to_string())),
            ]),
            &context,
        )
        .await;
    assert!(memo_result.is_ok());

    // Step 4: Test CLI exclusion detection
    let detector = registry.as_exclusion_detector();

    // Verify exclusion detection matches our expectations
    assert!(detector.is_cli_excluded("workflow_manager"));
    assert!(detector.is_cli_excluded("internal_state"));
    assert!(!detector.is_cli_excluded("user_memo"));
    assert!(!detector.is_cli_excluded("file_utility"));

    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();

    let mut expected_excluded = vec!["workflow_manager", "internal_state"];
    expected_excluded.sort();
    let mut actual_excluded = excluded_tools.clone();
    actual_excluded.sort();
    assert_eq!(actual_excluded, expected_excluded);

    let mut expected_eligible = vec!["user_memo", "file_utility"];
    expected_eligible.sort();
    let mut actual_eligible = eligible_tools.clone();
    actual_eligible.sort();
    assert_eq!(actual_eligible, expected_eligible);

    // Step 5: Test CLI command generation
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();

    // Should generate commands only for eligible tools
    assert_eq!(commands.len(), 2);

    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    assert!(command_tool_names.contains(&&"user_memo".to_string()));
    assert!(command_tool_names.contains(&&"file_utility".to_string()));
    assert!(!command_tool_names.contains(&&"workflow_manager".to_string()));
    assert!(!command_tool_names.contains(&&"internal_state".to_string()));

    // Step 6: Verify command structure and quality
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.tool_name.is_empty());
        assert!(!command.description.is_empty());

        // Commands should have CLI-friendly names
        assert!(!command.name.contains('_'), "Command name should use kebab-case");

        // Commands should have proper argument structure
        for arg in &command.arguments {
            assert!(!arg.name.is_empty());
            assert!(!arg.description.is_empty());
            assert!(!arg.name.contains('_'), "Argument name should use kebab-case");
        }

        // Required arguments should come before optional ones
        let mut found_optional = false;
        for arg in &command.arguments {
            if !arg.required {
                found_optional = true;
            } else if found_optional {
                panic!("Required argument found after optional argument in {}", command.name);
            }
        }
    }

    // Step 7: Test with different CLI generation configurations
    let configs = vec![
        ("default", GenerationConfig::default()),
        (
            "with_prefix",
            GenerationConfig {
                command_prefix: Some("app".to_string()),
                ..Default::default()
            },
        ),
        (
            "grouped",
            GenerationConfig {
                naming_strategy: NamingStrategy::GroupByDomain,
                use_subcommands: true,
                ..Default::default()
            },
        ),
    ];

    for (config_name, config) in configs {
        let generator = CliGenerator::new(Arc::from(registry.clone())).with_config(config);
        let result = generator.generate_commands();
        
        assert!(result.is_ok(), "Config '{}' should succeed", config_name);
        let config_commands = result.unwrap();

        // All configs should respect exclusions
        for command in &config_commands {
            assert!(
                ["user_memo", "file_utility"].contains(&command.tool_name.as_str()),
                "Config '{}' should only generate commands for eligible tools",
                config_name
            );
        }

        // Test config-specific formatting
        if config_name == "with_prefix" {
            for command in &config_commands {
                assert!(
                    command.name.starts_with("app-"),
                    "Prefixed command should start with 'app-': {}",
                    command.name
                );
            }
        }
    }
}

/// Test complete workflow with real SwissArmyHammer tools
#[tokio::test]
async fn test_e2e_with_real_swissarmyhammer_tools() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register real SwissArmyHammer tools
    swissarmyhammer_tools::register_memo_tools(&mut registry);
    swissarmyhammer_tools::register_file_tools(&mut registry);
    swissarmyhammer_tools::register_issue_tools(&mut registry);
    
    // Register tools known to be excluded
    use swissarmyhammer_tools::mcp::register_abort_tools;
    register_abort_tools(&mut registry);

    println!("E2E test with {} total tools", registry.len());

    // Test MCP functionality for some tools
    let context = registry.create_test_context().await;
    
    // Test memo creation (should be CLI-eligible)
    if let Some(memo_create_tool) = registry.get_tool("memo_create") {
        let memo_args = serde_json::Map::from_iter(vec![
            ("title".to_string(), Value::String("E2E Test Memo".to_string())),
            ("content".to_string(), Value::String("Testing complete workflow".to_string())),
        ]);
        
        let result = memo_create_tool.execute(memo_args, &context).await;
        assert!(result.is_ok(), "Memo creation should work in MCP context");
    }

    // Test abort creation (should be CLI-excluded but MCP-functional) 
    if let Some(abort_tool) = registry.get_tool("abort_create") {
        let abort_args = serde_json::Map::from_iter(vec![
            ("reason".to_string(), Value::String("E2E test abort".to_string())),
        ]);
        
        let result = abort_tool.execute(abort_args, &context).await;
        assert!(result.is_ok(), "Abort tool should work in MCP context");
    }

    // Test CLI exclusion detection
    let detector = registry.as_exclusion_detector();
    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();

    println!("Excluded tools: {:?}", excluded_tools);
    println!("Eligible tools: {} total", eligible_tools.len());

    // Verify known exclusions
    assert!(
        excluded_tools.contains(&"abort_create".to_string()),
        "abort_create should be excluded"
    );

    if registry.get_tool("issue_work").is_some() {
        assert!(
            excluded_tools.contains(&"issue_work".to_string()),
            "issue_work should be excluded"
        );
    }

    if registry.get_tool("issue_merge").is_some() {
        assert!(
            excluded_tools.contains(&"issue_merge".to_string()),
            "issue_merge should be excluded"
        );
    }

    // Test CLI generation
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();

    println!("Generated {} CLI commands", commands.len());
    assert!(!commands.is_empty(), "Should generate some CLI commands");

    // Verify exclusions are respected
    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    
    for excluded_tool in &excluded_tools {
        assert!(
            !command_tool_names.contains(&excluded_tool),
            "Excluded tool '{}' should not have a CLI command",
            excluded_tool
        );
    }

    // Verify command quality
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.tool_name.is_empty());
        assert!(!command.description.is_empty());
        
        // Should have CLI-appropriate naming
        assert!(!command.name.contains('_'));
        
        // Should have reasonable argument structure
        for arg in &command.arguments {
            assert!(!arg.name.is_empty());
            assert!(!arg.description.is_empty());
        }
    }

    // Test a few sample CLI commands for structure
    if let Some(memo_command) = commands.iter().find(|c| c.tool_name.contains("memo")) {
        println!("Sample memo command: {}", memo_command.name);
        assert!(!memo_command.arguments.is_empty(), "Memo commands should have arguments");
    }

    if let Some(file_command) = commands.iter().find(|c| c.tool_name.contains("files")) {
        println!("Sample file command: {}", file_command.name);
        // File commands may or may not have arguments depending on the specific tool
    }
}

/// Test error scenarios in end-to-end workflow
#[tokio::test]
async fn test_e2e_error_scenarios() {
    let _env = IsolatedTestEnvironment::new();

    // Test with completely empty registry
    let empty_registry = Arc::new(ToolRegistry::new());
    let detector = empty_registry.as_exclusion_detector();
    let generator = CliGenerator::new(empty_registry);

    assert!(detector.get_excluded_tools().is_empty());
    assert!(detector.get_cli_eligible_tools().is_empty());

    let commands = generator.generate_commands().unwrap();
    assert!(commands.is_empty());

    // Test with registry containing only excluded tools
    let mut excluded_only_registry = ToolRegistry::new();
    
    #[sah_marker_macros::cli_exclude]
    #[derive(Default)]
    struct ExcludedOnlyTool;

    impl CliExclusionMarker for ExcludedOnlyTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("All tools excluded test")
        }
    }

    #[async_trait]
    impl McpTool for ExcludedOnlyTool {
        fn name(&self) -> &'static str {
            "excluded_only"
        }

        fn description(&self) -> &'static str {
            "Tool for testing all-excluded scenario"
        }

        fn schema(&self) -> Value {
            serde_json::json!({})
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("Excluded tool executed"))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    excluded_only_registry.register(Box::new(ExcludedOnlyTool::default()));

    let detector = excluded_only_registry.as_exclusion_detector();
    let generator = CliGenerator::new(Arc::new(excluded_only_registry));

    assert_eq!(detector.get_excluded_tools().len(), 1);
    assert!(detector.get_cli_eligible_tools().is_empty());

    let commands = generator.generate_commands().unwrap();
    assert!(commands.is_empty(), "No commands should be generated when all tools are excluded");

    // Test with invalid CLI configuration
    let mut registry = ToolRegistry::new();
    
    #[derive(Default)]
    struct SimpleIncludedTool;

    #[async_trait]
    impl McpTool for SimpleIncludedTool {
        fn name(&self) -> &'static str {
            "simple_tool"
        }

        fn description(&self) -> &'static str {
            "Simple tool for error testing"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("Simple tool executed"))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    registry.register(Box::new(SimpleIncludedTool::default()));

    // Test invalid configuration
    let invalid_config = GenerationConfig {
        command_prefix: Some("".to_string()), // Invalid empty prefix
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(registry)).with_config(invalid_config);
    let result = generator.generate_commands();
    
    assert!(result.is_err(), "Invalid configuration should cause error");
}

/// Test performance characteristics of end-to-end workflow
#[tokio::test]
async fn test_e2e_performance() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register a reasonable number of tools to test performance
    for i in 0..100 {
        let tool_name = format!("perf_tool_{}", i);
        
        if i % 4 == 0 {
            // Every 4th tool is excluded
            registry.register(Box::new(super::super::common::test_utils::ExcludedMockTool::new(
                tool_name,
                "Performance test exclusion"
            )));
        } else {
            // Other tools are included
            registry.register(Box::new(super::super::common::test_utils::IncludedMockTool::new(tool_name)));
        }
    }

    let start_time = std::time::Instant::now();

    // Test complete workflow timing
    let detector = registry.as_exclusion_detector();
    let detection_time = start_time.elapsed();

    let generation_start = std::time::Instant::now();
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();
    let generation_time = generation_start.elapsed();

    let total_time = start_time.elapsed();

    // Verify results are correct
    assert_eq!(commands.len(), 75); // 75 out of 100 tools should be eligible (100 - 25 excluded)

    // Performance assertions (generous limits for CI)
    assert!(
        detection_time.as_millis() < 1000,
        "Exclusion detection took too long: {}ms",
        detection_time.as_millis()
    );
    assert!(
        generation_time.as_millis() < 2000,
        "CLI generation took too long: {}ms", 
        generation_time.as_millis()
    );
    assert!(
        total_time.as_millis() < 3000,
        "Total E2E workflow took too long: {}ms",
        total_time.as_millis()
    );

    println!(
        "E2E Performance: Detection={}ms, Generation={}ms, Total={}ms",
        detection_time.as_millis(),
        generation_time.as_millis(),
        total_time.as_millis()
    );
}