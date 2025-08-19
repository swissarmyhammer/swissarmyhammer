//! Comprehensive CLI Exclusion System Tests
//!
//! This test file serves as the main entry point for all CLI exclusion system tests,
//! providing comprehensive validation of the exclusion marker system from end-to-end.

mod cli_exclusion;

use cli_exclusion::common::test_utils::{CliExclusionTestEnvironment, assert_exclusion_detection};
use swissarmyhammer_tools::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig};
use std::sync::Arc;

/// Main integration test validating the complete CLI exclusion system
#[tokio::test]
async fn test_cli_exclusion_system_comprehensive() {
    let _env = IsolatedTestEnvironment::new();
    
    println!("🚀 Starting comprehensive CLI exclusion system test");
    
    // Test with generated test fixtures
    let test_env = CliExclusionTestEnvironment::new();
    println!("📝 Created test environment with {} excluded and {} included tools",
        test_env.fixture.excluded_tool_names.len(),
        test_env.fixture.included_tool_names.len()
    );

    // Step 1: Validate exclusion detection
    println!("🔍 Testing exclusion detection...");
    let detector = test_env.fixture.as_exclusion_detector();
    assert_exclusion_detection(
        &detector,
        &test_env.fixture.excluded_tool_names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &test_env.fixture.included_tool_names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    println!("✅ Exclusion detection working correctly");

    // Step 2: Validate CLI generation respects exclusions
    println!("🏗️  Testing CLI generation...");
    let generator = CliGenerator::new(Arc::new(test_env.fixture.registry));
    let commands = generator.generate_commands().unwrap();
    
    assert_eq!(commands.len(), test_env.fixture.included_tool_names.len(),
        "Should generate commands only for included tools");
    
    // Verify only included tools have commands
    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    for included_tool in &test_env.fixture.included_tool_names {
        assert!(command_tool_names.contains(&included_tool),
            "Included tool '{}' should have a generated command", included_tool);
    }
    for excluded_tool in &test_env.fixture.excluded_tool_names {
        assert!(!command_tool_names.contains(&excluded_tool),
            "Excluded tool '{}' should not have a generated command", excluded_tool);
    }
    println!("✅ CLI generation respecting exclusions correctly");

    // Step 3: Validate command structure
    println!("🔧 Validating command structure...");
    for command in &commands {
        assert!(!command.name.is_empty(), "Command name should not be empty");
        assert!(!command.tool_name.is_empty(), "Tool name should not be empty");
        assert!(!command.description.is_empty(), "Description should not be empty");
        assert!(!command.name.contains('_'), "Command names should use kebab-case");
        
        // Validate argument ordering (required first)
        let mut found_optional = false;
        for arg in &command.arguments {
            if !arg.required {
                found_optional = true;
            } else if found_optional {
                panic!("Required argument '{}' found after optional arguments in command '{}'",
                    arg.name, command.name);
            }
        }
    }
    println!("✅ All generated commands have proper structure");

    // Step 4: Test with different configurations
    println!("⚙️  Testing different CLI generation configurations...");
    let configs = vec![
        ("default", GenerationConfig::default()),
        ("with_prefix", GenerationConfig {
            command_prefix: Some("test".to_string()),
            ..Default::default()
        }),
    ];

    for (config_name, config) in configs {
        let generator = CliGenerator::new(Arc::from(test_env.fixture.registry.clone()))
            .with_config(config);
        let config_commands = generator.generate_commands().unwrap();
        
        assert_eq!(config_commands.len(), test_env.fixture.included_tool_names.len(),
            "Config '{}' should generate same number of commands", config_name);
        
        // All configs should respect exclusions
        for command in &config_commands {
            assert!(test_env.fixture.included_tool_names.contains(&command.tool_name),
                "Config '{}' should only generate commands for included tools", config_name);
        }
    }
    println!("✅ All configurations respect exclusions correctly");

    println!("🎉 Comprehensive CLI exclusion system test completed successfully!");
    println!("   • {} tools tested ({} excluded, {} included)",
        test_env.fixture.total_count(),
        test_env.fixture.excluded_tool_names.len(),
        test_env.fixture.included_tool_names.len()
    );
    println!("   • {} CLI commands generated", commands.len());
    println!("   • All exclusions respected");
    println!("   • All command structures validated");
}

/// Test CLI exclusion system with real SwissArmyHammer tools
#[tokio::test]
async fn test_cli_exclusion_with_real_swissarmyhammer_tools() {
    let _env = IsolatedTestEnvironment::new();
    
    println!("🚀 Testing CLI exclusion with real SwissArmyHammer tools");
    
    let mut registry = swissarmyhammer_tools::ToolRegistry::new();
    
    // Register various tool categories
    swissarmyhammer_tools::register_memo_tools(&registry);
    swissarmyhammer_tools::register_file_tools(&registry);
    swissarmyhammer_tools::register_issue_tools(&registry);
    
    // Register tools that should be excluded
    use swissarmyhammer_tools::mcp::register_abort_tools;
    register_abort_tools(&registry);

    println!("📝 Registered {} real SwissArmyHammer tools", registry.len());

    // Test exclusion detection
    let detector = registry.as_exclusion_detector();
    let excluded_tools = detector.get_excluded_tools();
    let eligible_tools = detector.get_cli_eligible_tools();
    
    println!("🔍 Found {} excluded tools, {} eligible tools", 
        excluded_tools.len(), eligible_tools.len());

    // Verify known exclusions
    if registry.get_tool("abort_create").is_some() {
        assert!(excluded_tools.contains(&"abort_create".to_string()),
            "abort_create should be excluded from CLI");
        println!("✅ abort_create correctly excluded");
    }

    if registry.get_tool("issue_work").is_some() {
        assert!(excluded_tools.contains(&"issue_work".to_string()),
            "issue_work should be excluded from CLI");
        println!("✅ issue_work correctly excluded");
    }

    if registry.get_tool("issue_merge").is_some() {
        assert!(excluded_tools.contains(&"issue_merge".to_string()),
            "issue_merge should be excluded from CLI");
        println!("✅ issue_merge correctly excluded");
    }

    // Test CLI generation
    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();
    
    assert!(result.is_ok(), "CLI generation with real tools should succeed");
    let commands = result.unwrap();
    
    assert!(!commands.is_empty(), "Should generate some CLI commands");
    println!("🏗️  Generated {} CLI commands from real tools", commands.len());

    // Verify no excluded tools made it through
    let command_tool_names: Vec<&String> = commands.iter().map(|c| &c.tool_name).collect();
    for excluded_tool in &excluded_tools {
        assert!(!command_tool_names.contains(&excluded_tool),
            "Excluded tool '{}' should not have a generated command", excluded_tool);
    }

    // Validate command quality with real tools
    for command in &commands {
        assert!(!command.name.is_empty());
        assert!(!command.tool_name.is_empty());
        assert!(!command.description.is_empty());
        assert!(!command.name.contains('_'), "Real tool commands should use kebab-case");
    }

    println!("✅ All real tool CLI commands properly structured");
    println!("🎉 Real SwissArmyHammer tool integration test completed successfully!");
}

/// Performance validation test
#[tokio::test]
async fn test_cli_exclusion_performance_validation() {
    let _env = IsolatedTestEnvironment::new();
    
    println!("🚀 Performance validation test for CLI exclusion system");
    
    // Create a reasonably sized registry for performance testing
    let test_env = CliExclusionTestEnvironment::with_tool_counts(100, 200); // 300 total tools
    println!("📝 Created performance test environment with {} tools", test_env.fixture.total_count());

    // Test exclusion detection performance
    let start = std::time::Instant::now();
    let detector = test_env.fixture.as_exclusion_detector();
    let detection_time = start.elapsed();
    
    println!("🔍 Exclusion detection setup: {:.2}ms", detection_time.as_millis());
    assert!(detection_time.as_millis() < 100, "Detection should be very fast");

    // Test query performance
    let query_start = std::time::Instant::now();
    for i in 0..1000 {
        let tool_name = format!("test_query_{}", i % 300);
        let _ = detector.is_cli_excluded(&tool_name);
    }
    let query_duration = query_start.elapsed();
    let queries_per_sec = 1000.0 / query_duration.as_secs_f64();
    
    println!("🚀 Query performance: {:.0} queries/sec", queries_per_sec);
    assert!(queries_per_sec > 10000.0, "Should handle >10k queries/sec");

    // Test CLI generation performance
    let generation_start = std::time::Instant::now();
    let generator = CliGenerator::new(Arc::new(test_env.fixture.registry));
    let commands = generator.generate_commands().unwrap();
    let generation_time = generation_start.elapsed();
    
    println!("🏗️  CLI generation: {} commands in {:.2}ms", 
        commands.len(), generation_time.as_millis());
    assert!(generation_time.as_millis() < 1000, "Generation should complete within 1 second");

    println!("✅ Performance validation passed!");
    println!("🎉 CLI exclusion system meets performance requirements");
}

/// Error handling validation test
#[tokio::test]
async fn test_cli_exclusion_error_handling_validation() {
    let _env = IsolatedTestEnvironment::new();
    
    println!("🚀 Error handling validation test for CLI exclusion system");

    // Test with empty registry
    let empty_registry = Arc::new(swissarmyhammer_tools::ToolRegistry::new());
    let empty_detector = empty_registry.as_exclusion_detector();
    let empty_generator = CliGenerator::new(empty_registry);
    
    assert!(empty_detector.get_excluded_tools().is_empty());
    assert!(empty_detector.get_cli_eligible_tools().is_empty());
    assert!(!empty_detector.is_cli_excluded("any_tool"));
    
    let empty_commands = empty_generator.generate_commands().unwrap();
    assert!(empty_commands.is_empty());
    println!("✅ Empty registry handled correctly");

    // Test with invalid configuration
    let test_env = CliExclusionTestEnvironment::new();
    let invalid_config = GenerationConfig {
        command_prefix: Some("".to_string()), // Invalid empty prefix
        ..Default::default()
    };
    
    let invalid_generator = CliGenerator::new(Arc::new(test_env.fixture.registry))
        .with_config(invalid_config);
    let invalid_result = invalid_generator.generate_commands();
    
    assert!(invalid_result.is_err(), "Invalid config should cause error");
    println!("✅ Invalid configuration handled correctly");

    // Test nonexistent tool queries
    let detector = test_env.fixture.as_exclusion_detector();
    assert!(!detector.is_cli_excluded("definitely_nonexistent_tool"));
    assert!(!detector.is_cli_excluded(""));
    println!("✅ Nonexistent tool queries handled correctly");

    println!("🎉 Error handling validation completed successfully!");
}

/// System integration smoke test
#[test]
fn test_cli_exclusion_system_smoke_test() {
    println!("🚀 CLI Exclusion System Smoke Test");
    println!("==================================");
    
    // Test that all components can be imported and basic structures created
    use swissarmyhammer_tools::cli::{CliExclusionMarker, ToolCliMetadata, RegistryCliExclusionDetector};
    use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig};
    use std::collections::HashMap;

    // Test metadata creation
    let metadata = ToolCliMetadata::excluded("test_tool", "Test exclusion");
    assert!(metadata.is_cli_excluded);
    assert_eq!(metadata.name, "test_tool");
    
    // Test detector creation
    let mut metadata_map = HashMap::new();
    metadata_map.insert("test_tool".to_string(), metadata);
    let detector = RegistryCliExclusionDetector::new(metadata_map);
    assert!(detector.is_cli_excluded("test_tool"));
    
    // Test config creation
    let config = GenerationConfig::default();
    assert!(config.validate().is_ok());
    
    println!("✅ All components can be imported and instantiated");
    println!("✅ Basic functionality works correctly");
    println!("🎉 Smoke test passed - CLI exclusion system is functional!");
}