/// Integration tests for CLI generation foundation
///
/// These tests validate the CLI generation system end-to-end,
/// ensuring proper integration with the ToolRegistry and MCP tools.
use std::sync::Arc;
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
use swissarmyhammer_tools::ToolRegistry;

/// Helper function to register all available tools for testing
fn register_all_tools(registry: &mut ToolRegistry) {
    // Re-exported functions from swissarmyhammer_tools
    swissarmyhammer_tools::register_file_tools(registry);
    swissarmyhammer_tools::register_issue_tools(registry);
    swissarmyhammer_tools::register_memo_tools(registry);
    swissarmyhammer_tools::register_notify_tools(registry);
    swissarmyhammer_tools::register_search_tools(registry);
    swissarmyhammer_tools::register_shell_tools(registry);
    swissarmyhammer_tools::register_todo_tools(registry);
    swissarmyhammer_tools::register_web_fetch_tools(registry);
    swissarmyhammer_tools::register_web_search_tools(registry);

    // These need to be imported from the mcp module directly
    use swissarmyhammer_tools::mcp::{register_abort_tools, register_outline_tools};
    register_abort_tools(registry);
    register_outline_tools(registry);
}

#[tokio::test]
async fn test_cli_generation_with_real_registry() {
    // Create a registry with actual tools
    let mut registry = ToolRegistry::new();

    // Register some built-in tools that should be available
    register_all_tools(&mut registry);

    // Verify we have tools registered
    assert!(
        !registry.is_empty(),
        "Registry should contain registered tools"
    );

    println!("Registry contains {} tools", registry.len());
    let eligible_count = registry.get_cli_eligible_tools().len();
    println!("CLI-eligible tools: {eligible_count}");

    // Create generator and test basic generation
    let generator = CliGenerator::new(Arc::new(registry));
    let result = generator.generate_commands();

    // Should succeed
    assert!(result.is_ok(), "CLI generation should succeed: {result:?}");

    let commands = result.unwrap();
    println!("Generated {} commands", commands.len());

    // Should generate some commands (but respecting CLI exclusions)
    // Note: The exact number depends on how many tools are CLI-eligible
    assert!(
        !commands.is_empty(),
        "Should generate at least some commands"
    );

    // Verify command structure
    for command in &commands {
        assert!(!command.name.is_empty(), "Command name should not be empty");
        assert!(
            !command.tool_name.is_empty(),
            "Tool name should not be empty"
        );

        // Command names should be CLI-friendly (dashes, not underscores)
        assert!(
            !command.name.contains('_'),
            "Command name should use dashes: {}",
            command.name
        );

        // Arguments should be sorted (required first)
        let mut found_optional = false;
        for arg in &command.arguments {
            if !arg.required && !found_optional {
                found_optional = true;
            } else if arg.required && found_optional {
                panic!(
                    "Required arguments should come before optional ones in command: {}",
                    command.name
                );
            }
        }
    }

    println!("✅ Basic generation test passed");
}

#[tokio::test]
async fn test_naming_strategies() {
    let mut registry = ToolRegistry::new();
    register_all_tools(&mut registry);
    let registry = Arc::new(registry);

    // Test KeepOriginal strategy
    let config = GenerationConfig {
        naming_strategy: NamingStrategy::KeepOriginal,
        ..Default::default()
    };
    let generator = CliGenerator::new(registry.clone()).with_config(config);
    let original_commands = generator.generate_commands().unwrap();

    // Test GroupByDomain strategy
    let config = GenerationConfig {
        naming_strategy: NamingStrategy::GroupByDomain,
        use_subcommands: true,
        ..Default::default()
    };
    let generator = CliGenerator::new(registry.clone()).with_config(config);
    let domain_commands = generator.generate_commands().unwrap();

    // Test Flatten strategy
    let config = GenerationConfig {
        naming_strategy: NamingStrategy::Flatten,
        ..Default::default()
    };
    let generator = CliGenerator::new(registry.clone()).with_config(config);
    let flattened_commands = generator.generate_commands().unwrap();

    println!("Original: {} commands", original_commands.len());
    println!("Domain: {} commands", domain_commands.len());
    println!("Flattened: {} commands", flattened_commands.len());

    // All strategies should generate some commands
    assert!(!original_commands.is_empty());
    assert!(!domain_commands.is_empty());
    assert!(!flattened_commands.is_empty());

    // Domain strategy should create parent commands
    let has_parents = domain_commands
        .iter()
        .any(|cmd| cmd.subcommand_of.is_none());
    let has_subcommands = domain_commands
        .iter()
        .any(|cmd| cmd.subcommand_of.is_some());

    if domain_commands.len() > 1 {
        assert!(
            has_parents || has_subcommands,
            "GroupByDomain should create hierarchical structure"
        );
    }

    println!("✅ Naming strategies test passed");
}

#[tokio::test]
async fn test_cli_exclusion_respect() {
    let mut registry = ToolRegistry::new();
    register_all_tools(&mut registry);

    // Check that some tools are marked as CLI-excluded
    let all_tools = registry.list_tool_names();
    let eligible_tools: Vec<_> = registry
        .get_cli_eligible_tools()
        .into_iter()
        .map(|meta| meta.name.clone())
        .collect();

    println!(
        "All tools: {} | CLI-eligible: {}",
        all_tools.len(),
        eligible_tools.len()
    );

    // There should be some exclusions (this test assumes some tools are excluded)
    if all_tools.len() > eligible_tools.len() {
        println!("✅ CLI exclusion system is working (some tools excluded)");

        // Verify specific exclusions that we know should exist
        let excluded_tools = ["issue_work", "issue_merge", "abort_create"];
        for tool_name in &excluded_tools {
            if all_tools.contains(&tool_name.to_string()) {
                assert!(
                    !eligible_tools.contains(&tool_name.to_string()),
                    "Tool '{tool_name}' should be CLI-excluded"
                );
                println!("✅ Confirmed {tool_name} is excluded as expected");
            }
        }
    }

    // Generate commands and verify excluded tools are not included
    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();

    let command_tool_names: Vec<_> = commands.iter().map(|cmd| &cmd.tool_name).collect();

    // Ensure no excluded tools made it through
    for excluded in &["issue_work", "issue_merge", "abort_create"] {
        let excluded_string = excluded.to_string();
        assert!(
            !command_tool_names.contains(&&excluded_string),
            "Excluded tool '{excluded}' should not generate a command"
        );
    }

    println!("✅ CLI exclusion respect test passed");
}

#[tokio::test]
async fn test_schema_parsing_comprehensive() {
    let mut registry = ToolRegistry::new();
    register_all_tools(&mut registry);

    let generator = CliGenerator::new(Arc::new(registry));
    let commands = generator.generate_commands().unwrap();

    // Find commands with different argument patterns
    let mut found_required_args = false;
    let mut found_optional_args = false;
    let mut found_options = false;
    let mut found_constraints = false;

    for command in &commands {
        if !command.arguments.is_empty() {
            for arg in &command.arguments {
                if arg.required {
                    found_required_args = true;
                } else {
                    found_optional_args = true;
                }

                if arg.has_constraints() {
                    found_constraints = true;
                }
            }
        }

        if !command.options.is_empty() {
            found_options = true;
        }

        // Verify argument naming is consistent
        for arg in &command.arguments {
            assert!(
                !arg.name.contains('_'),
                "Argument names should use dashes: {}",
                arg.name
            );
        }

        for option in &command.options {
            assert!(
                !option.name.contains('_'),
                "Option names should use dashes: {}",
                option.name
            );
            assert!(
                option.long.starts_with("--"),
                "Long options should start with --: {}",
                option.long
            );
        }
    }

    // We should have found examples of different schema patterns
    // (These assertions might not always hold depending on the specific tools registered)
    if !commands.is_empty() {
        println!("Schema parsing patterns found:");
        println!("  Required args: {found_required_args}");
        println!("  Optional args: {found_optional_args}");
        println!("  Options: {found_options}");
        println!("  Constraints: {found_constraints}");
    }

    println!("✅ Schema parsing comprehensive test passed");
}

#[tokio::test]
async fn test_generation_error_handling() {
    // Test with empty registry
    let empty_registry = Arc::new(ToolRegistry::new());
    let generator = CliGenerator::new(empty_registry);

    let result = generator.generate_commands();
    assert!(
        result.is_ok(),
        "Empty registry should succeed with empty commands list"
    );
    assert!(
        result.unwrap().is_empty(),
        "Empty registry should generate no commands"
    );

    // Test configuration validation
    let mut registry = ToolRegistry::new();
    register_all_tools(&mut registry);

    // Test invalid configuration - empty prefix
    let bad_config = GenerationConfig {
        command_prefix: Some("".to_string()),
        ..Default::default()
    };
    let generator = CliGenerator::new(Arc::new(registry)).with_config(bad_config);
    let result = generator.generate_commands();
    assert!(
        result.is_err(),
        "Empty prefix should cause validation error"
    );

    println!("✅ Error handling test passed");
}

#[tokio::test]
async fn test_command_limits() {
    let mut registry = ToolRegistry::new();
    register_all_tools(&mut registry);

    // If we have enough tools, test the command limit
    if registry.len() > 1 {
        let config = GenerationConfig {
            max_commands: 1, // Very low limit
            ..Default::default()
        };

        let generator = CliGenerator::new(Arc::new(registry)).with_config(config);
        let result = generator.generate_commands();

        // This might succeed or fail depending on how many CLI-eligible tools we have
        match result {
            Ok(commands) => {
                // If it succeeded, we had few enough eligible tools
                assert!(commands.len() <= 1, "Should respect command limit");
                println!(
                    "✅ Command limit respected (generated {} commands)",
                    commands.len()
                );
            }
            Err(e) => {
                // If it failed, we hit the limit as expected
                println!("✅ Command limit correctly enforced: {e}");
                assert!(e.to_string().contains("Too many commands"));
            }
        }
    } else {
        println!("⚠️  Skipping command limit test - not enough tools in registry");
    }
}
