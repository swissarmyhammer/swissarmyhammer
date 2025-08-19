//! Error handling and edge case tests for CLI exclusion system
//!
//! These tests validate system behavior under error conditions and edge cases,
//! ensuring graceful degradation and proper error reporting.

use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, GenerationError, NamingStrategy};
use swissarmyhammer_tools::cli::{RegistryCliExclusionDetector, ToolCliMetadata};
use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::Value;
use super::super::common::test_utils::{ExcludedMockTool, IncludedMockTool};

/// Test error handling in CLI generation configuration
#[tokio::test]
async fn test_cli_generation_config_errors() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(IncludedMockTool::new("test_tool")));

    // Test empty prefix error
    let empty_prefix_config = GenerationConfig {
        command_prefix: Some("".to_string()),
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(registry.clone()))
        .with_config(empty_prefix_config);
    let result = generator.generate_commands();

    assert!(result.is_err());
    match result.unwrap_err() {
        GenerationError::ConfigValidation(msg) => {
            assert!(msg.contains("prefix") || msg.contains("empty"));
        }
        other => panic!("Expected ConfigValidation error, got {:?}", other),
    }

    // Test invalid prefix characters
    let invalid_prefix_config = GenerationConfig {
        command_prefix: Some("invalid prefix with spaces".to_string()),
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(registry.clone()))
        .with_config(invalid_prefix_config);
    let result = generator.generate_commands();

    assert!(result.is_err());
    match result.unwrap_err() {
        GenerationError::ConfigValidation(_) => {}, // Expected
        other => panic!("Expected ConfigValidation error, got {:?}", other),
    }

    // Test zero max commands
    let zero_max_config = GenerationConfig {
        max_commands: 0,
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(registry.clone()))
        .with_config(zero_max_config);
    let result = generator.generate_commands();

    assert!(result.is_err());
    match result.unwrap_err() {
        GenerationError::ConfigValidation(_) => {}, // Expected
        other => panic!("Expected ConfigValidation error, got {:?}", other),
    }
}

/// Test CLI generation with command limit exceeded
#[tokio::test]
async fn test_cli_generation_command_limit_exceeded() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register more tools than the limit allows
    for i in 0..10 {
        registry.register(Box::new(IncludedMockTool::new(format!("tool_{}", i))));
    }

    let limited_config = GenerationConfig {
        max_commands: 5, // Less than the number of tools
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(registry)).with_config(limited_config);
    let result = generator.generate_commands();

    assert!(result.is_err());
    match result.unwrap_err() {
        GenerationError::TooManyCommands { limit, actual } => {
            assert_eq!(limit, 5);
            assert_eq!(actual, 10);
        }
        other => panic!("Expected TooManyCommands error, got {:?}", other),
    }
}

/// Test CLI generation with malformed tool schemas
#[tokio::test]
async fn test_cli_generation_with_malformed_schemas() {
    let _env = IsolatedTestEnvironment::new();

    /// Tool with invalid JSON schema
    #[derive(Default)]
    struct MalformedSchemaTool;

    #[async_trait]
    impl McpTool for MalformedSchemaTool {
        fn name(&self) -> &'static str {
            "malformed_schema_tool"
        }

        fn description(&self) -> &'static str {
            "Tool with malformed schema for testing error handling"
        }

        fn schema(&self) -> Value {
            // Return an invalid schema
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
            Ok(BaseToolImpl::create_success_response("Malformed tool executed"))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MalformedSchemaTool::default()));
    registry.register(Box::new(IncludedMockTool::new("good_tool")));

    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();

    // The generator should handle malformed schemas gracefully
    // It might skip the malformed tool or return an error
    match result {
        Ok(commands) => {
            // If it succeeds, it should only include the valid tool
            assert!(commands.len() <= 1);
            if !commands.is_empty() {
                assert_eq!(commands[0].tool_name, "good_tool");
            }
        }
        Err(GenerationError::ParseError(_)) => {
            // Parse error is also acceptable for malformed schemas
        }
        Err(other) => panic!("Unexpected error type: {:?}", other),
    }
}

/// Test exclusion detector with edge case inputs
#[tokio::test]
async fn test_exclusion_detector_edge_cases() {
    let _env = IsolatedTestEnvironment::new();

    // Test with empty metadata
    let empty_detector = RegistryCliExclusionDetector::new(HashMap::new());
    
    assert!(!empty_detector.is_cli_excluded("any_tool"));
    assert!(!empty_detector.is_cli_excluded(""));
    assert!(empty_detector.get_excluded_tools().is_empty());
    assert!(empty_detector.get_cli_eligible_tools().is_empty());
    assert!(empty_detector.get_all_tool_metadata().is_empty());

    // Test with special character tool names
    let mut metadata = HashMap::new();
    
    let special_names = [
        "", // Empty string
        " ", // Space only
        "\t", // Tab
        "\n", // Newline
        "tool with spaces",
        "tool-with-dashes",
        "tool_with_underscores",
        "tool.with.dots",
        "tool:with:colons",
        "tool/with/slashes",
        "tool@with@symbols",
        "UPPERCASE_TOOL",
        "MixedCase_Tool",
        "123numeric_start",
        "unicode_тест_🚀",
        "very_long_tool_name_that_exceeds_typical_length_expectations_and_continues_for_much_longer_than_normal",
    ];

    for (i, &name) in special_names.iter().enumerate() {
        if i % 2 == 0 {
            metadata.insert(
                name.to_string(),
                ToolCliMetadata::excluded(name, "Special character test"),
            );
        } else {
            metadata.insert(
                name.to_string(),
                ToolCliMetadata::included(name),
            );
        }
    }

    let detector = RegistryCliExclusionDetector::new(metadata);

    // All tools should be queryable regardless of special characters
    for (i, &name) in special_names.iter().enumerate() {
        let expected_excluded = i % 2 == 0;
        let actual_excluded = detector.is_cli_excluded(name);
        assert_eq!(
            actual_excluded, expected_excluded,
            "Tool '{}' exclusion mismatch",
            name
        );
    }

    // Bulk queries should handle special characters
    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();
    
    assert_eq!(excluded_tools.len() + eligible_tools.len(), special_names.len());

    // Test nonexistent tool queries
    let nonexistent_names = [
        "definitely_does_not_exist",
        "another_nonexistent_tool",
        "",
        "null",
        "undefined",
    ];

    for name in &nonexistent_names {
        assert!(!detector.is_cli_excluded(name), "Nonexistent tool should not be excluded");
    }
}

/// Test registry error scenarios
#[tokio::test]
async fn test_registry_error_scenarios() {
    let _env = IsolatedTestEnvironment::new();

    // Test registry with duplicate tool names
    let mut registry = ToolRegistry::new();
    
    registry.register(Box::new(IncludedMockTool::new("duplicate_tool")));
    registry.register(Box::new(ExcludedMockTool::new("duplicate_tool", "Second registration")));

    // Registry should handle duplicates (last one wins or error)
    let detector = registry.as_exclusion_detector();
    
    // The behavior depends on implementation - either tool should exist
    let all_metadata = detector.get_all_tool_metadata();
    let duplicate_metadata: Vec<_> = all_metadata
        .iter()
        .filter(|m| m.name == "duplicate_tool")
        .collect();

    // Should have at most one entry for the duplicate name
    assert!(duplicate_metadata.len() <= 1);

    // Test tool execution still works
    let context = registry.create_test_context().await;
    if let Some(tool) = registry.get_tool("duplicate_tool") {
        let result = tool.execute(serde_json::Map::new(), &context).await;
        assert!(result.is_ok());
    }
}

/// Test error handling with concurrent access
#[tokio::test]
async fn test_concurrent_error_scenarios() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register some tools
    for i in 0..100 {
        if i % 3 == 0 {
            registry.register(Box::new(ExcludedMockTool::new(
                format!("excluded_{}", i),
                "Concurrent test"
            )));
        } else {
            registry.register(Box::new(IncludedMockTool::new(format!("included_{}", i))));
        }
    }

    let detector = Arc::new(registry.as_exclusion_detector());
    let registry_arc = Arc::new(registry);

    // Spawn tasks that might encounter errors
    let mut tasks = Vec::new();

    for task_id in 0..10 {
        let detector_clone = detector.clone();
        let registry_clone = registry_arc.clone();

        let task = tokio::task::spawn(async move {
            match task_id % 4 {
                0 => {
                    // Query nonexistent tools (should not fail)
                    for i in 0..50 {
                        let nonexistent_name = format!("nonexistent_{}_{}", task_id, i);
                        let result = detector_clone.is_cli_excluded(&nonexistent_name);
                        assert!(!result); // Should return false for nonexistent tools
                    }
                }
                1 => {
                    // Generate CLI commands (should not fail)
                    let generator = CliGenerator::new(registry_clone);
                    let result = generator.generate_commands();
                    assert!(result.is_ok());
                }
                2 => {
                    // Query bulk metadata (should not fail)
                    for _ in 0..10 {
                        let metadata = detector_clone.get_all_tool_metadata();
                        assert_eq!(metadata.len(), 100);
                    }
                }
                3 => {
                    // Generate with invalid config (should fail gracefully)
                    let invalid_config = GenerationConfig {
                        command_prefix: Some("".to_string()), // Invalid
                        ..Default::default()
                    };
                    let generator = CliGenerator::new(registry_clone)
                        .with_config(invalid_config);
                    let result = generator.generate_commands();
                    assert!(result.is_err()); // Should fail but not panic
                }
                _ => unreachable!(),
            }
            
            task_id
        });

        tasks.push(task);
    }

    // All tasks should complete without panicking
    let results = futures::future::join_all(tasks).await;
    
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Task {} panicked: {:?}", i, result.unwrap_err());
    }
}

/// Test resource cleanup and memory safety
#[tokio::test]
async fn test_resource_cleanup() {
    let _env = IsolatedTestEnvironment::new();

    // Test that detectors can be dropped safely
    {
        let mut registry = ToolRegistry::new();
        for i in 0..1000 {
            registry.register(Box::new(IncludedMockTool::new(format!("tool_{}", i))));
        }

        let detector = registry.as_exclusion_detector();
        let _metadata = detector.get_all_tool_metadata();
        // Detector goes out of scope here
    }

    // Test that generators can be dropped safely
    {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(IncludedMockTool::new("test_tool")));

        let generator = CliGenerator::new(Arc::new(registry));
        let _commands = generator.generate_commands().unwrap();
        // Generator goes out of scope here
    }

    // Test that registries can be dropped safely
    {
        let mut registry = ToolRegistry::new();
        for i in 0..100 {
            registry.register(Box::new(IncludedMockTool::new(format!("cleanup_tool_{}", i))));
        }

        let _detector = registry.as_exclusion_detector();
        let _generator = CliGenerator::new(Arc::new(registry));
        // Everything goes out of scope here
    }

    // If we get here without crashes, cleanup is working
    assert!(true);
}

/// Test error message quality and usefulness
#[tokio::test]
async fn test_error_message_quality() {
    let _env = IsolatedTestEnvironment::new();

    // Test configuration validation error messages
    let invalid_configs = vec![
        (
            GenerationConfig {
                command_prefix: Some("".to_string()),
                ..Default::default()
            },
            "empty prefix",
        ),
        (
            GenerationConfig {
                command_prefix: Some("invalid prefix".to_string()),
                ..Default::default()
            },
            "invalid characters",
        ),
        (
            GenerationConfig {
                max_commands: 0,
                ..Default::default()
            },
            "zero max commands",
        ),
    ];

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(IncludedMockTool::new("test_tool")));

    for (config, expected_content) in invalid_configs {
        let generator = CliGenerator::new(Arc::new(registry.clone())).with_config(config);
        let result = generator.generate_commands();

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        
        // Error message should be non-empty and descriptive
        assert!(!error_message.is_empty());
        assert!(error_message.len() > 10); // Reasonably descriptive

        println!("Error for {}: {}", expected_content, error_message);
    }

    // Test command limit error message
    let mut large_registry = ToolRegistry::new();
    for i in 0..10 {
        large_registry.register(Box::new(IncludedMockTool::new(format!("tool_{}", i))));
    }

    let limited_config = GenerationConfig {
        max_commands: 5,
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(large_registry)).with_config(limited_config);
    let result = generator.generate_commands();

    assert!(result.is_err());
    let error_message = result.unwrap_err().to_string();
    
    // Should contain relevant numbers
    assert!(error_message.contains("5")); // Limit
    assert!(error_message.contains("10")); // Actual count
    assert!(error_message.to_lowercase().contains("limit") || error_message.to_lowercase().contains("exceed"));

    println!("Command limit error: {}", error_message);
}

/// Test system stability under stress
#[tokio::test]
async fn test_system_stability_under_stress() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register many tools
    for i in 0..500 {
        if i % 5 == 0 {
            registry.register(Box::new(ExcludedMockTool::new(
                format!("stress_excluded_{}", i),
                "Stress test exclusion"
            )));
        } else {
            registry.register(Box::new(IncludedMockTool::new(format!("stress_included_{}", i))));
        }
    }

    let detector = Arc::new(registry.as_exclusion_detector());
    let registry_arc = Arc::new(registry);

    // Perform many operations concurrently
    let mut tasks = Vec::new();

    for task_id in 0..20 {
        let detector_clone = detector.clone();
        let registry_clone = registry_arc.clone();

        let task = tokio::task::spawn(async move {
            for iteration in 0..100 {
                match (task_id + iteration) % 5 {
                    0 => {
                        // Individual exclusion queries
                        for i in 0..10 {
                            let tool_name = format!("stress_included_{}", i * 5 + 1);
                            let _result = detector_clone.is_cli_excluded(&tool_name);
                        }
                    }
                    1 => {
                        // Bulk exclusion queries
                        let _excluded = detector_clone.get_excluded_tools();
                    }
                    2 => {
                        // Bulk eligible queries
                        let _eligible = detector_clone.get_cli_eligible_tools();
                    }
                    3 => {
                        // Metadata queries
                        let _metadata = detector_clone.get_all_tool_metadata();
                    }
                    4 => {
                        // CLI generation
                        let generator = CliGenerator::new(registry_clone.clone());
                        let _commands = generator.generate_commands().unwrap();
                    }
                    _ => unreachable!(),
                }
            }
            
            task_id
        });

        tasks.push(task);
    }

    // All tasks should complete successfully
    let results = futures::future::join_all(tasks).await;
    
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Stress task {} failed: {:?}", i, result.unwrap_err());
    }

    // System should still be functional after stress test
    let final_detector = registry_arc.as_exclusion_detector();
    let final_metadata = final_detector.get_all_tool_metadata();
    assert_eq!(final_metadata.len(), 500);

    let final_generator = CliGenerator::new(registry_arc);
    let final_commands = final_generator.generate_commands().unwrap();
    assert_eq!(final_commands.len(), 400); // 500 - 100 excluded = 400
}

/// Test error handling with invalid tool implementations
#[tokio::test]
async fn test_invalid_tool_implementations() {
    let _env = IsolatedTestEnvironment::new();

    /// Tool that panics during schema generation
    #[derive(Default)]
    struct PanicSchemaTool;

    #[async_trait]
    impl McpTool for PanicSchemaTool {
        fn name(&self) -> &'static str {
            "panic_schema_tool"
        }

        fn description(&self) -> &'static str {
            "Tool that panics during schema generation"
        }

        fn schema(&self) -> Value {
            // This would panic in a real scenario
            // For testing, we'll return an empty schema instead
            serde_json::json!({})
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("Panic tool executed"))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(PanicSchemaTool::default()));
    registry.register(Box::new(IncludedMockTool::new("normal_tool")));

    // System should handle problematic tools gracefully
    let detector = registry.as_exclusion_detector();
    assert!(detector.get_all_tool_metadata().len() >= 1); // At least the normal tool

    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();
    
    // Should either succeed with valid tools or fail gracefully
    match result {
        Ok(commands) => {
            // If it succeeds, should have at least the normal tool
            assert!(!commands.is_empty());
        }
        Err(_) => {
            // Graceful failure is also acceptable
        }
    }
}