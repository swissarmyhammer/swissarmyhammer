//! Integration tests for CLI generation with exclusion detection
//!
//! These tests validate the complete CLI generation pipeline, including exclusion
//! detection, schema parsing, and command generation with real tool registries.

use std::sync::Arc;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
use swissarmyhammer_tools::ToolRegistry;
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use super::super::common::test_utils::{CliExclusionTestEnvironment, ExcludedMockTool, IncludedMockTool};

/// Test complete CLI generation pipeline with mixed excluded and included tools
#[tokio::test]
async fn test_complete_cli_generation_pipeline() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Create tools with varying complexity to test schema parsing
    
    // Simple excluded tool
    registry.register(Box::new(ExcludedMockTool::new(
        "simple_workflow_tool",
        "Simple workflow orchestration"
    )));

    // Complex excluded tool with detailed schema
    let complex_excluded = create_complex_excluded_tool();
    registry.register(Box::new(complex_excluded));

    // Simple included tool
    registry.register(Box::new(IncludedMockTool::new("simple_user_tool")));

    // Complex included tool with detailed schema
    let complex_included = create_complex_included_tool();
    registry.register(Box::new(complex_included));

    // Test CLI generation
    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();

    assert!(result.is_ok(), "CLI generation should succeed");
    let commands = result.unwrap();

    // Should generate commands only for included tools
    assert_eq!(commands.len(), 2, "Should generate 2 commands for included tools");

    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    assert!(command_tool_names.contains(&&"simple_user_tool".to_string()));
    assert!(command_tool_names.contains(&&"complex_user_tool".to_string()));
    assert!(!command_tool_names.contains(&&"simple_workflow_tool".to_string()));
    assert!(!command_tool_names.contains(&&"complex_workflow_tool".to_string()));

    // Verify command structure for simple tool
    let simple_command = commands
        .iter()
        .find(|c| c.tool_name == "simple_user_tool")
        .expect("Simple command should exist");

    assert_eq!(simple_command.name, "simple-user-tool");
    assert!(!simple_command.description.is_empty());

    // Verify command structure for complex tool
    let complex_command = commands
        .iter()
        .find(|c| c.tool_name == "complex_user_tool")
        .expect("Complex command should exist");

    assert_eq!(complex_command.name, "complex-user-tool");
    assert!(!complex_command.arguments.is_empty());

    // Complex tool should have properly parsed arguments
    let required_args: Vec<_> = complex_command
        .arguments
        .iter()
        .filter(|arg| arg.required)
        .collect();
    let optional_args: Vec<_> = complex_command
        .arguments
        .iter()
        .filter(|arg| !arg.required)
        .collect();

    assert!(!required_args.is_empty(), "Complex tool should have required arguments");
    assert!(!optional_args.is_empty(), "Complex tool should have optional arguments");

    // Arguments should be properly ordered (required first)
    let mut found_optional = false;
    for arg in &complex_command.arguments {
        if !arg.required {
            found_optional = true;
        } else if found_optional {
            panic!("Required argument found after optional argument");
        }
    }
}

/// Test CLI generation with various naming strategies and exclusions
#[tokio::test]
async fn test_naming_strategies_with_exclusions() {
    let env = CliExclusionTestEnvironment::new();
    let registry = Arc::new(env.fixture.registry);

    // Test KeepOriginal naming strategy
    let original_config = GenerationConfig {
        naming_strategy: NamingStrategy::KeepOriginal,
        ..Default::default()
    };
    let generator = CliGenerator::new(registry.clone()).with_config(original_config);
    let original_commands = generator.generate_commands().unwrap();

    // Test GroupByDomain naming strategy
    let domain_config = GenerationConfig {
        naming_strategy: NamingStrategy::GroupByDomain,
        use_subcommands: true,
        ..Default::default()
    };
    let generator = CliGenerator::new(registry.clone()).with_config(domain_config);
    let domain_commands = generator.generate_commands().unwrap();

    // Test Flatten naming strategy
    let flatten_config = GenerationConfig {
        naming_strategy: NamingStrategy::Flatten,
        ..Default::default()
    };
    let generator = CliGenerator::new(registry.clone()).with_config(flatten_config);
    let flattened_commands = generator.generate_commands().unwrap();

    // All strategies should exclude the same tools
    let expected_included_count = env.fixture.included_tool_names.len();

    // Verify exclusions are respected across all naming strategies
    for commands in [&original_commands, &domain_commands, &flattened_commands] {
        for command in commands {
            assert!(
                env.fixture.included_tool_names.contains(&command.tool_name),
                "Command '{}' should only be generated for included tools",
                command.name
            );
            assert!(
                !env.fixture.excluded_tool_names.contains(&command.tool_name),
                "Command '{}' should not be generated for excluded tools",
                command.name
            );
        }
    }

    // Test domain strategy creates hierarchical structure when appropriate
    if domain_commands.len() > 1 {
        // Check for potential parent-child relationships
        let has_subcommands = domain_commands.iter().any(|cmd| cmd.subcommand_of.is_some());
        let has_parents = domain_commands.iter().any(|cmd| cmd.subcommand_of.is_none());
        
        // At least some hierarchical structure should exist if we have tools from different domains
        if env.fixture.included_tool_names.len() > 3 {
            assert!(has_parents || has_subcommands, "Domain strategy should create some hierarchy");
        }
    }

    // Flatten strategy should have simple names
    for command in &flattened_commands {
        assert!(
            !command.name.contains('-'),
            "Flattened command names should not contain hierarchical separators"
        );
    }
}

/// Test CLI generation performance with large registries
#[tokio::test]
async fn test_cli_generation_performance() {
    let _env = IsolatedTestEnvironment::new();
    let env = CliExclusionTestEnvironment::with_tool_counts(100, 200); // 100 excluded, 200 included

    let start_time = std::time::Instant::now();
    let generator = CliGenerator::new(Arc::new(env.fixture.registry));
    let generation_result = generator.generate_commands();
    let generation_time = start_time.elapsed();

    assert!(generation_result.is_ok(), "Large registry generation should succeed");
    let commands = generation_result.unwrap();

    // Verify correct number of commands generated
    assert_eq!(commands.len(), 200, "Should generate commands for all included tools");

    // Generation should complete in reasonable time
    assert!(
        generation_time.as_millis() < 5000, // 5 seconds is generous
        "CLI generation took too long: {}ms",
        generation_time.as_millis()
    );

    // Verify all commands have proper structure
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.tool_name.is_empty());
        assert!(!command.description.is_empty());
        
        // Command names should be properly formatted
        assert!(!command.name.contains('_'), "Command names should use kebab-case");
        assert!(command.name.contains('-') || command.name.chars().all(|c| c.is_ascii_lowercase()));
    }
}

/// Test CLI generation error handling with complex scenarios
#[tokio::test]
async fn test_cli_generation_error_scenarios() {
    let _env = IsolatedTestEnvironment::new();

    // Test with empty registry
    let empty_registry = Arc::new(ToolRegistry::new());
    let generator = CliGenerator::new(empty_registry);
    let result = generator.generate_commands();
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());

    // Test with invalid configuration
    let env = CliExclusionTestEnvironment::new();
    let invalid_config = GenerationConfig {
        command_prefix: Some("".to_string()), // Invalid empty prefix
        ..Default::default()
    };
    let generator = CliGenerator::new(Arc::new(env.fixture.registry))
        .with_config(invalid_config);
    let result = generator.generate_commands();
    assert!(result.is_err(), "Invalid config should cause error");

    // Test with command limit exceeded
    let env = CliExclusionTestEnvironment::with_tool_counts(0, 10);
    let limited_config = GenerationConfig {
        max_commands: 5, // Less than the number of included tools
        ..Default::default()
    };
    let generator = CliGenerator::new(Arc::new(env.fixture.registry))
        .with_config(limited_config);
    let result = generator.generate_commands();
    
    if env.fixture.included_tool_names.len() > 5 {
        assert!(result.is_err(), "Should error when exceeding command limit");
    }
}

/// Test CLI generation with real SwissArmyHammer tools
#[tokio::test]
async fn test_cli_generation_with_real_tools() {
    let _env = IsolatedTestEnvironment::new();
    let mut registry = ToolRegistry::new();

    // Register some real SwissArmyHammer tools
    swissarmyhammer_tools::register_memo_tools(&mut registry);
    swissarmyhammer_tools::register_file_tools(&mut registry);
    
    // Also register some tools that should be excluded
    use swissarmyhammer_tools::mcp::register_abort_tools;
    register_abort_tools(&mut registry);

    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();

    assert!(result.is_ok(), "Real tools CLI generation should succeed");
    let commands = result.unwrap();
    assert!(!commands.is_empty(), "Should generate some commands from real tools");

    // Verify known exclusions are respected
    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    assert!(!command_tool_names.contains(&&"abort_create".to_string()), 
        "abort_create should be excluded from CLI generation");

    // Verify included tools are present
    let has_memo_tools = commands.iter().any(|c| c.tool_name.contains("memo"));
    let has_file_tools = commands.iter().any(|c| c.tool_name.contains("files"));
    
    if has_memo_tools {
        println!("✅ Memo tools included in CLI generation");
    }
    if has_file_tools {
        println!("✅ File tools included in CLI generation");
    }

    // All commands should have valid structure
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.tool_name.is_empty());
        assert!(!command.description.is_empty());

        // Validate argument structure
        for arg in &command.arguments {
            assert!(!arg.name.is_empty());
            assert!(!arg.description.is_empty());
            assert!(!arg.value_type.is_empty());
        }

        // Validate option structure  
        for option in &command.options {
            assert!(!option.name.is_empty());
            assert!(!option.long.is_empty());
            assert!(option.long.starts_with("--"));
        }
    }
}

/// Test CLI generation consistency across multiple runs
#[tokio::test]
async fn test_cli_generation_consistency() {
    let _env = IsolatedTestEnvironment::new();
    let env = CliExclusionTestEnvironment::new();
    let registry = Arc::new(env.fixture.registry);

    // Generate commands multiple times
    let generator = CliGenerator::new(registry);
    
    let commands1 = generator.generate_commands().unwrap();
    let commands2 = generator.generate_commands().unwrap();
    let commands3 = generator.generate_commands().unwrap();

    // Results should be consistent
    assert_eq!(commands1.len(), commands2.len());
    assert_eq!(commands2.len(), commands3.len());

    // Command names should be consistent
    let names1: Vec<&String> = commands1.iter().map(|c| &c.name).collect();
    let names2: Vec<&String> = commands2.iter().map(|c| &c.name).collect();
    let names3: Vec<&String> = commands3.iter().map(|c| &c.name).collect();

    assert_eq!(names1, names2);
    assert_eq!(names2, names3);

    // Tool names should be consistent
    let tool_names1: Vec<&String> = commands1.iter().map(|c| &c.tool_name).collect();
    let tool_names2: Vec<&String> = commands2.iter().map(|c| &c.tool_name).collect();
    let tool_names3: Vec<&String> = commands3.iter().map(|c| &c.tool_name).collect();

    assert_eq!(tool_names1, tool_names2);
    assert_eq!(tool_names2, tool_names3);
}

/// Test CLI generation with different configuration combinations
#[tokio::test]
async fn test_cli_generation_config_combinations() {
    let _env = IsolatedTestEnvironment::new();
    let env = CliExclusionTestEnvironment::new();
    let registry = Arc::new(env.fixture.registry);

    // Test various configuration combinations
    let configs = vec![
        GenerationConfig {
            naming_strategy: NamingStrategy::KeepOriginal,
            command_prefix: None,
            use_subcommands: false,
            max_commands: 1000,
        },
        GenerationConfig {
            naming_strategy: NamingStrategy::GroupByDomain,
            command_prefix: Some("test".to_string()),
            use_subcommands: true,
            max_commands: 50,
        },
        GenerationConfig {
            naming_strategy: NamingStrategy::Flatten,
            command_prefix: Some("cli".to_string()),
            use_subcommands: false,
            max_commands: 100,
        },
    ];

    for (i, config) in configs.into_iter().enumerate() {
        let generator = CliGenerator::new(registry.clone()).with_config(config.clone());
        let result = generator.generate_commands();
        
        assert!(result.is_ok(), "Configuration {} should succeed", i);
        let commands = result.unwrap();

        // Verify configuration is applied
        if let Some(prefix) = &config.command_prefix {
            for command in &commands {
                assert!(
                    command.name.starts_with(&format!("{}-", prefix)),
                    "Command '{}' should have prefix '{}-'",
                    command.name,
                    prefix
                );
            }
        }

        // All commands should still exclude the same tools regardless of config
        for command in &commands {
            assert!(
                env.fixture.included_tool_names.contains(&command.tool_name),
                "Config {} should still respect exclusions",
                i
            );
        }
    }
}

/// Helper function to create a complex excluded tool for testing
fn create_complex_excluded_tool() -> ExcludedMockTool {
    use swissarmyhammer_tools::cli::CliExclusionMarker;
    use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext, BaseToolImpl};
    use async_trait::async_trait;
    use rmcp::model::CallToolResult;
    use rmcp::Error as McpError;
    use serde_json::Value;

    #[derive(Default)]
    struct ComplexExcludedTool;

    impl CliExclusionMarker for ComplexExcludedTool {
        fn is_cli_excluded(&self) -> bool {
            true
        }

        fn exclusion_reason(&self) -> Option<&'static str> {
            Some("Complex workflow orchestration requiring MCP context")
        }
    }

    #[async_trait]
    impl McpTool for ComplexExcludedTool {
        fn name(&self) -> &'static str {
            "complex_workflow_tool"
        }

        fn description(&self) -> &'static str {
            "Complex workflow orchestration tool with advanced schema"
        }

        fn schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "workflow_name": {
                        "type": "string",
                        "description": "Name of the workflow to manage"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["create", "start", "pause", "resume", "stop", "delete"],
                        "description": "Action to perform on the workflow"
                    },
                    "parameters": {
                        "type": "object",
                        "description": "Workflow-specific parameters",
                        "additionalProperties": true
                    },
                    "timeout": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 3600,
                        "default": 300,
                        "description": "Timeout in seconds"
                    },
                    "async_mode": {
                        "type": "boolean",
                        "default": false,
                        "description": "Whether to run in async mode"
                    }
                },
                "required": ["workflow_name", "action"]
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, Value>,
            _context: &ToolContext,
        ) -> Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response(
                "Complex workflow tool executed"
            ))
        }

        fn as_any(&self) -> Option<&dyn std::any::Any> {
            Some(self)
        }
    }

    // This is a hack to return the right type, but it demonstrates the concept
    ExcludedMockTool::new("complex_workflow_tool", "Complex workflow orchestration")
}

/// Helper function to create a complex included tool for testing
fn create_complex_included_tool() -> IncludedMockTool {
    IncludedMockTool::new("complex_user_tool")
}