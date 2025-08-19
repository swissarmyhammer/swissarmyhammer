//! Cross-system integration tests for CLI exclusion system
//!
//! These tests validate the complete integration between tool registration,
//! exclusion detection, CLI generation, and actual tool execution.

use std::sync::Arc;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig};
use swissarmyhammer_tools::ToolRegistry;
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use super::super::common::test_utils::{ExcludedMockTool, IncludedMockTool};

/// Test complete flow from tool registration to CLI generation
#[tokio::test]
async fn test_complete_tool_lifecycle() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Step 1: Register tools with mixed exclusion status
    registry.register(Box::new(ExcludedMockTool::new(
        "workflow_orchestrator",
        "Designed for MCP workflow orchestration only"
    )));
    registry.register(Box::new(IncludedMockTool::new("user_memo_tool")));
    registry.register(Box::new(IncludedMockTool::new("file_processor")));
    registry.register(Box::new(ExcludedMockTool::new(
        "internal_state_manager", 
        "Internal tool for state management"
    )));

    // Step 2: Verify tools are properly registered and functional in MCP
    assert_eq!(registry.len(), 4);
    
    let context = registry.create_test_context().await;
    
    // All tools should be executable in MCP context (exclusion doesn't affect MCP)
    for tool_name in ["workflow_orchestrator", "user_memo_tool", "file_processor", "internal_state_manager"] {
        let tool = registry.get_tool(tool_name).expect("Tool should exist");
        let result = tool.execute(serde_json::Map::new(), &context).await;
        assert!(result.is_ok(), "Tool '{}' should execute successfully in MCP", tool_name);
    }

    // Step 3: Test CLI exclusion detection
    let detector = registry.as_exclusion_detector();
    
    assert!(detector.is_cli_excluded("workflow_orchestrator"));
    assert!(detector.is_cli_excluded("internal_state_manager"));
    assert!(!detector.is_cli_excluded("user_memo_tool"));
    assert!(!detector.is_cli_excluded("file_processor"));

    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();
    
    assert_eq!(excluded_tools.len(), 2);
    assert_eq!(eligible_tools.len(), 2);

    // Step 4: Test CLI generation respects exclusions
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();

    assert_eq!(commands.len(), 2, "Should generate commands only for eligible tools");

    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    assert!(command_tool_names.contains(&&"user_memo_tool".to_string()));
    assert!(command_tool_names.contains(&&"file_processor".to_string()));
    assert!(!command_tool_names.contains(&&"workflow_orchestrator".to_string()));
    assert!(!command_tool_names.contains(&&"internal_state_manager".to_string()));

    // Step 5: Verify generated commands have proper structure
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.description.is_empty());
        assert!(command.name.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
    }
}

/// Test integration with real SwissArmyHammer tools and known exclusions
#[tokio::test] 
async fn test_integration_with_real_swissarmyhammer_tools() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register various SwissArmyHammer tool categories
    swissarmyhammer_tools::register_memo_tools(&mut registry);
    swissarmyhammer_tools::register_file_tools(&mut registry);
    swissarmyhammer_tools::register_issue_tools(&mut registry);
    
    // Register tools that should be excluded
    use swissarmyhammer_tools::mcp::register_abort_tools;
    register_abort_tools(&mut registry);

    println!("Total registered tools: {}", registry.len());

    // Test CLI exclusion detection with real tools
    let detector = registry.as_exclusion_detector();
    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();

    println!("Excluded tools: {:?}", excluded_tools);
    println!("Eligible tools count: {}", eligible_tools.len());

    // Known exclusions should be detected
    if registry.get_tool("abort_create").is_some() {
        assert!(detector.is_cli_excluded("abort_create"), 
            "abort_create should be excluded from CLI");
    }

    if registry.get_tool("issue_work").is_some() {
        assert!(detector.is_cli_excluded("issue_work"), 
            "issue_work should be excluded from CLI");
    }

    if registry.get_tool("issue_merge").is_some() {
        assert!(detector.is_cli_excluded("issue_merge"), 
            "issue_merge should be excluded from CLI");
    }

    // Test CLI generation
    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();

    assert!(result.is_ok(), "CLI generation with real tools should succeed");
    let commands = result.unwrap();
    
    assert!(!commands.is_empty(), "Should generate some commands");
    println!("Generated {} CLI commands", commands.len());

    // Verify no excluded tools made it through
    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    for excluded_tool in &excluded_tools {
        assert!(!command_tool_names.contains(&excluded_tool),
            "Excluded tool '{}' should not have a generated command", excluded_tool);
    }

    // Test that commands have realistic structure for CLI usage
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.tool_name.is_empty());
        assert!(!command.description.is_empty());
        
        // Command should have CLI-friendly naming
        assert!(!command.name.contains('_'), "Command '{}' should use kebab-case", command.name);
        
        // Arguments should be properly structured
        for arg in &command.arguments {
            assert!(!arg.name.is_empty());
            assert!(!arg.name.contains('_'), "Arg '{}' should use kebab-case", arg.name);
        }
    }
}

/// Test CLI generation configuration affects exclusion behavior correctly
#[tokio::test]
async fn test_configuration_integration_with_exclusions() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register diverse set of tools
    registry.register(Box::new(ExcludedMockTool::new("workflow_tool", "Workflow")));
    registry.register(Box::new(ExcludedMockTool::new("orchestration_tool", "Orchestration")));
    registry.register(Box::new(IncludedMockTool::new("user_tool")));
    registry.register(Box::new(IncludedMockTool::new("api_tool")));
    registry.register(Box::new(IncludedMockTool::new("data_tool")));

    let registry_arc = Arc::new(registry);

    // Test various configurations all respect exclusions
    let configurations = vec![
        ("default", GenerationConfig::default()),
        ("with_prefix", GenerationConfig {
            command_prefix: Some("app".to_string()),
            ..Default::default()
        }),
        ("domain_grouped", GenerationConfig {
            naming_strategy: swissarmyhammer_cli::generation::NamingStrategy::GroupByDomain,
            use_subcommands: true,
            ..Default::default()
        }),
        ("limited", GenerationConfig {
            max_commands: 2,
            ..Default::default()
        }),
    ];

    for (config_name, config) in configurations {
        let generator = CliGenerator::new(registry_arc.clone()).with_config(config);
        let result = generator.generate_commands();

        match config_name {
            "limited" => {
                // May succeed or fail depending on number of eligible tools vs limit
                if let Ok(commands) = result {
                    assert!(commands.len() <= 2, "Should respect command limit");
                    // Still should respect exclusions
                    for command in &commands {
                        assert!(!command.tool_name.starts_with("workflow"));
                        assert!(!command.tool_name.starts_with("orchestration"));
                    }
                }
            }
            _ => {
                assert!(result.is_ok(), "Configuration '{}' should succeed", config_name);
                let commands = result.unwrap();
                
                // All configs should respect exclusions
                assert_eq!(commands.len(), 3, "All configs should generate 3 commands for included tools");
                
                for command in &commands {
                    assert!(
                        ["user_tool", "api_tool", "data_tool"].contains(&command.tool_name.as_str()),
                        "Config '{}' generated command for excluded tool: {}",
                        config_name,
                        command.tool_name
                    );
                    
                    // Verify configuration-specific formatting
                    match config_name {
                        "with_prefix" => {
                            assert!(command.name.starts_with("app-"), 
                                "Command '{}' should have 'app-' prefix", command.name);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Test error propagation across the complete system
#[tokio::test]
async fn test_cross_system_error_handling() {
    let _env = IsolatedTestEnvironment::new();

    // Test 1: Empty registry handling across all systems
    let empty_registry = Arc::new(ToolRegistry::new());
    
    // CLI exclusion detection should work with empty registry
    let detector = empty_registry.as_exclusion_detector();
    assert!(detector.get_excluded_tools().is_empty());
    assert!(detector.get_cli_eligible_tools().is_empty());
    assert!(!detector.is_cli_excluded("any_tool"));

    // CLI generation should succeed with empty result
    let generator = CliGenerator::new(empty_registry);
    let result = generator.generate_commands();
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());

    // Test 2: Invalid configuration propagation
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(IncludedMockTool::new("test_tool")));

    let invalid_config = GenerationConfig {
        command_prefix: Some("".to_string()), // Invalid empty prefix
        ..Default::default()
    };

    let generator = CliGenerator::new(Arc::new(registry)).with_config(invalid_config);
    let result = generator.generate_commands();
    assert!(result.is_err(), "Invalid config should propagate as error");

    // Error should be a configuration validation error
    match result.unwrap_err() {
        swissarmyhammer_cli::generation::GenerationError::ConfigValidation(_) => {},
        other => panic!("Expected ConfigValidation error, got {:?}", other),
    }
}

/// Test concurrent access across all system components
#[tokio::test]
async fn test_concurrent_cross_system_access() {
    use std::sync::Arc;
    use tokio::task;

    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register tools
    for i in 0..20 {
        if i % 3 == 0 {
            registry.register(Box::new(ExcludedMockTool::new(
                format!("excluded_{}", i),
                "Concurrent test exclusion"
            )));
        } else {
            registry.register(Box::new(IncludedMockTool::new(format!("included_{}", i))));
        }
    }

    let registry_arc = Arc::new(registry);

    // Spawn multiple tasks accessing different parts of the system
    let mut tasks = Vec::new();

    for task_id in 0..10 {
        let registry_clone = registry_arc.clone();
        
        let task = task::spawn(async move {
            match task_id % 4 {
                0 => {
                    // Test MCP tool access
                    let context = registry_clone.create_test_context().await;
                    for i in 0..5 {
                        let tool_name = format!("included_{}", i * 3 + 1);
                        if let Some(tool) = registry_clone.get_tool(&tool_name) {
                            let result = tool.execute(serde_json::Map::new(), &context).await;
                            assert!(result.is_ok());
                        }
                    }
                }
                1 => {
                    // Test CLI exclusion detection
                    let detector = registry_clone.as_exclusion_detector();
                    for i in 0..20 {
                        let tool_name = if i % 3 == 0 {
                            format!("excluded_{}", i)
                        } else {
                            format!("included_{}", i)
                        };
                        let is_excluded = detector.is_cli_excluded(&tool_name);
                        let should_be_excluded = i % 3 == 0;
                        assert_eq!(is_excluded, should_be_excluded);
                    }
                }
                2 => {
                    // Test bulk exclusion queries
                    let detector = registry_clone.as_exclusion_detector();
                    let excluded = detector.get_excluded_tools();
                    let eligible = detector.get_cli_eligible_tools();
                    assert_eq!(excluded.len() + eligible.len(), 20);
                }
                3 => {
                    // Test CLI generation
                    let generator = CliGenerator::new(registry_clone);
                    let result = generator.generate_commands();
                    assert!(result.is_ok());
                    let commands = result.unwrap();
                    
                    // Should be roughly 2/3 of the tools (non-excluded)
                    let expected_count = (20 * 2) / 3; // Roughly 13-14 commands
                    assert!(commands.len() >= expected_count - 2 && commands.len() <= expected_count + 2);
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
        assert!(result.is_ok(), "Concurrent task {} failed: {:?}", i, result.unwrap_err());
    }
}

/// Test system behavior with registry modifications after detector creation
#[tokio::test]
async fn test_registry_modification_isolation() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register initial tools
    registry.register(Box::new(ExcludedMockTool::new("initial_excluded", "Initial")));
    registry.register(Box::new(IncludedMockTool::new("initial_included")));

    // Create detector from initial state
    let detector = registry.as_exclusion_detector();
    assert_eq!(detector.get_all_tool_metadata().len(), 2);

    // Modify registry after detector creation
    registry.register(Box::new(IncludedMockTool::new("added_later")));

    // Original detector should still reflect the state when it was created
    let original_metadata = detector.get_all_tool_metadata();
    assert_eq!(original_metadata.len(), 2);
    assert!(!detector.is_cli_excluded("added_later")); // Returns false for unknown tools

    // New detector should see the updated state
    let new_detector = registry.as_exclusion_detector();
    let new_metadata = new_detector.get_all_tool_metadata();
    assert_eq!(new_metadata.len(), 3);

    // CLI generation should use the current registry state
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();
    assert_eq!(commands.len(), 2); // 2 included tools
    
    let command_tools: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    assert!(command_tools.contains(&&"initial_included".to_string()));
    assert!(command_tools.contains(&&"added_later".to_string()));
}

/// Test integration with SwissArmyHammer test utilities
#[tokio::test]
async fn test_integration_with_test_utilities() {
    let _env = IsolatedTestEnvironment::new();
    
    // Test that our CLI exclusion system works with SwissArmyHammer's test infrastructure
    use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment as SAHTestEnv;
    
    let _sah_env = SAHTestEnv::new();
    let mut registry = ToolRegistry::new();

    // Register tools
    registry.register(Box::new(ExcludedMockTool::new("test_excluded", "Test")));
    registry.register(Box::new(IncludedMockTool::new("test_included")));

    // Test MCP execution context creation
    let context = registry.create_test_context().await;
    
    // Both tools should execute in MCP context
    let excluded_result = registry
        .get_tool("test_excluded")
        .unwrap()
        .execute(serde_json::Map::new(), &context)
        .await;
    assert!(excluded_result.is_ok());

    let included_result = registry
        .get_tool("test_included")
        .unwrap()
        .execute(serde_json::Map::new(), &context)
        .await;
    assert!(included_result.is_ok());

    // CLI generation should respect exclusions
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();
    
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].tool_name, "test_included");
}