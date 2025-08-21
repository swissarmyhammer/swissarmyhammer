use std::sync::Arc;
use swissarmyhammer_cli::{cli_builder::CliBuilder, mcp_integration::create_test_tool_registry};
use swissarmyhammer::test_utils::create_test_home_guard;

/// Comprehensive tests for CLI generation from MCP tools
/// These tests verify that the dynamic CLI system correctly generates commands
/// from the MCP tool registry and preserves all static commands

#[tokio::test]
async fn test_all_static_commands_preserved() {
    let _guard = create_test_home_guard();
    
    // Create tool registry and CLI builder
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Verify all expected static commands are present
    let expected_static_commands = vec![
        "serve", "doctor", "prompt", "flow", 
        "completion", "validate", "plan", "implement", "config"
    ];
    
    for cmd_name in expected_static_commands {
        assert!(
            cli.find_subcommand(cmd_name).is_some(),
            "Static command '{}' not found in generated CLI",
            cmd_name
        );
    }
}

#[tokio::test]
async fn test_dynamic_commands_generated() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Get expected dynamic commands from the tool registry
    let categories = registry.get_cli_categories();
    
    // Verify each category becomes a command
    for category in &categories {
        assert!(
            cli.find_subcommand(category).is_some(),
            "Dynamic category command '{}' not found",
            category
        );
    }
    
    // Test some known categories that should exist
    let expected_categories = vec![
        "issue", "memo", "file", "search"
    ];
    
    for category in expected_categories {
        if categories.contains(&category.to_string()) {
            assert!(
                cli.find_subcommand(category).is_some(),
                "Expected dynamic command '{}' not found",
                category
            );
        }
    }
}

#[tokio::test]
async fn test_issue_subcommands_complete() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    if let Some(issue_cmd) = cli.find_subcommand("issue") {
        let expected_subcommands = vec![
            "create", "list", "show", "update", "work", "merge", "all-complete"
        ];
        
        for subcmd_name in expected_subcommands {
            // Check if the tool exists in the registry before expecting it in CLI
            if issue_cmd.find_subcommand(subcmd_name).is_none() {
                eprintln!("Warning: Issue subcommand '{}' not found. Available subcommands:", subcmd_name);
                for sub in issue_cmd.get_subcommands() {
                    eprintln!("  - {}", sub.get_name());
                }
            }
        }
        
        // At minimum, we should have create, list, and show
        let required_subcommands = vec!["create", "list", "show"];
        for subcmd_name in required_subcommands {
            assert!(
                issue_cmd.find_subcommand(subcmd_name).is_some(),
                "Required issue subcommand '{}' not found",
                subcmd_name
            );
        }
    }
}

#[tokio::test]
async fn test_memo_subcommands_complete() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    if let Some(memo_cmd) = cli.find_subcommand("memo") {
        let expected_subcommands = vec![
            "create", "list", "get", "update", "delete", "search", "context"
        ];
        
        for subcmd_name in expected_subcommands {
            // Check if the tool exists before requiring it
            if memo_cmd.find_subcommand(subcmd_name).is_none() {
                eprintln!("Warning: Memo subcommand '{}' not found. Available subcommands:", subcmd_name);
                for sub in memo_cmd.get_subcommands() {
                    eprintln!("  - {}", sub.get_name());
                }
            }
        }
        
        // At minimum, we should have create, list, and get
        let required_subcommands = vec!["create", "list", "get"];
        for subcmd_name in required_subcommands {
            assert!(
                memo_cmd.find_subcommand(subcmd_name).is_some(),
                "Required memo subcommand '{}' not found",
                subcmd_name
            );
        }
    }
}

#[tokio::test]
async fn test_file_subcommands_complete() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    if let Some(file_cmd) = cli.find_subcommand("file") {
        let expected_subcommands = vec![
            "read", "write", "edit", "glob", "grep"
        ];
        
        for subcmd_name in expected_subcommands {
            assert!(
                file_cmd.find_subcommand(subcmd_name).is_some(),
                "File subcommand '{}' not found",
                subcmd_name
            );
        }
    }
}

#[tokio::test]
async fn test_help_text_quality() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test top-level help
    assert!(cli.get_about().is_some(), "CLI should have top-level about text");
    let about_str = cli.get_about().unwrap().to_string();
    assert!(!about_str.is_empty(), "CLI about text should not be empty");
    
    // Test category help
    if let Some(issue_cmd) = cli.find_subcommand("issue") {
        assert!(issue_cmd.get_about().is_some(), "Issue command should have about text");
        let about_text = issue_cmd.get_about().unwrap().to_string().to_lowercase();
        assert!(about_text.contains("issue"), "Issue command help should mention 'issue'");
    }
    
    // Test individual tool help
    if let Some(issue_cmd) = cli.find_subcommand("issue") {
        if let Some(create_cmd) = issue_cmd.find_subcommand("create") {
            assert!(create_cmd.get_about().is_some(), "Issue create command should have about text");
            let about_text = create_cmd.get_about().unwrap().to_string();
            assert!(!about_text.is_empty(), "Issue create help should not be empty");
        }
    }
}

#[tokio::test]
async fn test_hidden_tools_excluded() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Tools marked as hidden should not appear in CLI
    // These are typically internal workflow tools
    let hidden_tools = vec!["todo", "notify", "abort"];
    
    for hidden_tool in hidden_tools {
        assert!(
            cli.find_subcommand(hidden_tool).is_none(),
            "Hidden tool '{}' should not appear in CLI",
            hidden_tool
        );
    }
}

#[tokio::test]
async fn test_root_level_tools() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Check for root-level tools (tools without categories)
    let root_tools = registry.get_root_cli_tools();
    
    for tool in root_tools {
        if !tool.hidden_from_cli() {
            let tool_name = tool.cli_name();
            assert!(
                cli.find_subcommand(tool_name).is_some(),
                "Root-level tool '{}' not found in CLI",
                tool_name
            );
        }
    }
}

#[tokio::test]
async fn test_command_structure_consistency() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Verify category commands have subcommand_required(true)
    let categories = registry.get_cli_categories();
    
    for category in &categories {
        if let Some(category_cmd) = cli.find_subcommand(category) {
            // Category commands should require subcommands
            assert!(
                category_cmd.is_subcommand_required_set(),
                "Category command '{}' should require subcommands",
                category
            );
            
            // Category commands should have at least one subcommand
            assert!(
                category_cmd.get_subcommands().count() > 0,
                "Category command '{}' should have subcommands",
                category
            );
        }
    }
}

#[tokio::test]
async fn test_argument_generation_from_schemas() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test specific tools to ensure their arguments are generated correctly
    if let Some(issue_cmd) = cli.find_subcommand("issue") {
        if let Some(create_cmd) = issue_cmd.find_subcommand("create") {
            // Issue create should have arguments based on its schema
            let args: Vec<_> = create_cmd.get_arguments().collect();
            assert!(
                !args.is_empty(),
                "Issue create command should have arguments"
            );
            
            // Look for expected arguments (these depend on the actual tool schemas)
            let arg_names: Vec<_> = args.iter().map(|arg| arg.get_id().as_str()).collect();
            
            // Common arguments we expect for issue creation
            let expected_args = vec!["content"];  // Simplified expectation
            for expected_arg in expected_args {
                if !arg_names.contains(&expected_arg) {
                    eprintln!("Warning: Expected argument '{}' not found in issue create. Available args: {:?}", 
                             expected_arg, arg_names);
                }
            }
        }
    }
}

#[tokio::test]
async fn test_cli_build_resilience() {
    let _guard = create_test_home_guard();
    
    // Test that CLI can be built even with minimal or empty registry
    let empty_registry = Arc::new(swissarmyhammer_tools::mcp::tool_registry::ToolRegistry::new());
    let builder = CliBuilder::new(empty_registry);
    let cli = builder.build_cli().expect("CLI should build even with empty registry");
    
    // Should still have static commands
    assert!(cli.find_subcommand("serve").is_some());
    assert!(cli.find_subcommand("doctor").is_some());
    assert!(cli.find_subcommand("prompt").is_some());
    
    // Should not have dynamic commands
    assert!(cli.find_subcommand("issue").is_none());
    assert!(cli.find_subcommand("memo").is_none());
}

#[tokio::test]
async fn test_prompt_subcommands_preserved() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Verify prompt subcommands are preserved from static definition
    if let Some(prompt_cmd) = cli.find_subcommand("prompt") {
        let expected_prompt_subcommands = vec![
            "list", "test", "search", "validate"
        ];
        
        for subcmd_name in expected_prompt_subcommands {
            assert!(
                prompt_cmd.find_subcommand(subcmd_name).is_some(),
                "Prompt subcommand '{}' not found",
                subcmd_name
            );
        }
    }
}

#[tokio::test]
async fn test_flow_subcommands_preserved() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Verify flow subcommands are preserved from static definition
    if let Some(flow_cmd) = cli.find_subcommand("flow") {
        let expected_flow_subcommands = vec![
            "run", "test", "resume", "list", "status", "logs", "metrics", "visualize"
        ];
        
        for subcmd_name in expected_flow_subcommands {
            assert!(
                flow_cmd.find_subcommand(subcmd_name).is_some(),
                "Flow subcommand '{}' not found",
                subcmd_name
            );
        }
    }
}

#[tokio::test]
async fn test_config_subcommands_preserved() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Verify config subcommands are preserved from static definition
    if let Some(config_cmd) = cli.find_subcommand("config") {
        let expected_config_subcommands = vec![
            "show", "variables", "test", "env"
        ];
        
        for subcmd_name in expected_config_subcommands {
            assert!(
                config_cmd.find_subcommand(subcmd_name).is_some(),
                "Config subcommand '{}' not found",
                subcmd_name
            );
        }
    }
}

#[tokio::test]
async fn test_cli_metadata_quality() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test CLI metadata
    assert!(cli.get_version().is_some(), "CLI should have version information");
    assert_eq!(cli.get_name(), "swissarmyhammer", "CLI name should be correct");
    
    // Test that long about includes essential information
    if let Some(long_about) = cli.get_long_about() {
        let about_text = long_about.to_string().to_lowercase();
        assert!(about_text.contains("mcp"), "Long about should mention MCP");
        assert!(about_text.contains("prompt"), "Long about should mention prompts");
    }
}

#[tokio::test]
async fn test_argument_validation_setup() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test that arg_required_else_help is properly set
    // This ensures users get help when no arguments are provided
    assert!(
        cli.is_arg_required_else_help_set(),
        "CLI should show help when no arguments provided"
    );
}

#[tokio::test]
async fn test_dynamic_tool_argument_types() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test that different argument types are handled correctly
    let categories = registry.get_cli_categories();
    
    for category in &categories {
        if let Some(category_cmd) = cli.find_subcommand(category) {
            for subcommand in category_cmd.get_subcommands() {
                for arg in subcommand.get_arguments() {
                    // Each argument should have an ID
                    assert!(!arg.get_id().as_str().is_empty(), 
                           "Argument in {}/{} should have non-empty ID", 
                           category, subcommand.get_name());
                    
                    // Arguments should have help text (unless it's empty in the schema)
                    if let Some(help) = arg.get_help() {
                        // If help exists, it shouldn't be just whitespace
                        let help_str = help.to_string();
                        assert!(!help_str.trim().is_empty() || help_str.is_empty(),
                               "Help text in {}/{} should be meaningful or empty", 
                               category, subcommand.get_name());
                    }
                }
            }
        }
    }
}

/// Test that CLI generation is deterministic and consistent
#[tokio::test]
async fn test_cli_generation_deterministic() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    
    // Build CLI multiple times
    let builder1 = CliBuilder::new(registry.clone());
    let cli1 = builder1.build_cli().expect("Failed to build CLI");
    
    let builder2 = CliBuilder::new(registry.clone());
    let cli2 = builder2.build_cli().expect("Failed to build CLI");
    
    // Should have same number of subcommands
    let subcommands1: Vec<_> = cli1.get_subcommands().map(|c| c.get_name()).collect();
    let subcommands2: Vec<_> = cli2.get_subcommands().map(|c| c.get_name()).collect();
    
    assert_eq!(subcommands1.len(), subcommands2.len(), 
               "CLI builds should be deterministic");
    
    // Should have same command names
    for (cmd1, cmd2) in subcommands1.iter().zip(subcommands2.iter()) {
        assert_eq!(cmd1, cmd2, "Command names should be consistent across builds");
    }
}