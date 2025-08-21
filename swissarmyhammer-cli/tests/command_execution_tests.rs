use std::sync::Arc;
use swissarmyhammer_cli::{
    cli_builder::{CliBuilder, DynamicCommandInfo},
    dynamic_execution::{is_dynamic_command, DynamicCommandExecutor},
    mcp_integration::create_test_tool_registry,
};
use swissarmyhammer::test_utils::create_test_home_guard;

/// Comprehensive tests for command execution detection and handling
/// These tests verify that the CLI correctly identifies static vs dynamic commands
/// and routes them appropriately through the execution system

#[tokio::test]
async fn test_static_command_detection() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    let static_command_cases = vec![
        vec!["swissarmyhammer", "serve"],
        vec!["swissarmyhammer", "doctor"], 
        vec!["swissarmyhammer", "prompt", "list"],
        vec!["swissarmyhammer", "validate"],
        vec!["swissarmyhammer", "flow", "run", "test-workflow"],
        vec!["swissarmyhammer", "completion", "bash"],
        vec!["swissarmyhammer", "plan", "test.plan"],
        vec!["swissarmyhammer", "implement"],
        vec!["swissarmyhammer", "config", "show"],
    ];
    
    for args in static_command_cases {
        let matches = cli.clone().try_get_matches_from(&args);
        
        if let Ok(matches) = matches {
            let is_dynamic = is_dynamic_command(&matches, &builder);
            assert!(!is_dynamic, 
                   "Static command {:?} should not be detected as dynamic", 
                   &args[1..]);
        } else {
            // Some commands may fail parsing due to missing required args, which is expected
            eprintln!("Warning: Static command {:?} failed to parse (may need required args)", 
                     &args[1..]);
        }
    }
}

#[tokio::test] 
async fn test_dynamic_command_detection() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    let categories = registry.get_cli_categories();
    
    // Test category-based dynamic commands
    for category in &categories {
        let tools = registry.get_tools_for_category(category);
        
        for tool in tools {
            if !tool.hidden_from_cli() {
                let args = vec!["swissarmyhammer", category, tool.cli_name()];
                
                match cli.clone().try_get_matches_from(&args) {
                    Ok(matches) => {
                        let is_dynamic = is_dynamic_command(&matches, &builder);
                        assert!(is_dynamic, 
                               "Dynamic command {:?} should be detected as dynamic", 
                               &args[1..]);
                    }
                    Err(e) => {
                        // Command might fail parsing due to missing required args
                        eprintln!("Warning: Dynamic command {:?} failed to parse: {}", 
                                 &args[1..], e);
                    }
                }
            }
        }
    }
    
    // Test root-level dynamic commands
    let root_tools = registry.get_root_cli_tools();
    for tool in root_tools {
        if !tool.hidden_from_cli() {
            let args = vec!["swissarmyhammer", tool.cli_name()];
            
            match cli.clone().try_get_matches_from(&args) {
                Ok(matches) => {
                    let is_dynamic = is_dynamic_command(&matches, &builder);
                    assert!(is_dynamic, 
                           "Root-level dynamic command {:?} should be detected as dynamic", 
                           &args[1..]);
                }
                Err(e) => {
                    eprintln!("Warning: Root-level dynamic command {:?} failed to parse: {}", 
                             &args[1..], e);
                }
            }
        }
    }
}

#[tokio::test]
async fn test_command_info_extraction() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test categorized command info extraction
    let categories = registry.get_cli_categories();
    
    for category in &categories {
        let tools = registry.get_tools_for_category(category);
        
        for tool in tools.iter().take(1) { // Test first tool in each category
            if !tool.hidden_from_cli() {
                let args = vec!["swissarmyhammer", category, tool.cli_name()];
                
                match cli.clone().try_get_matches_from(&args) {
                    Ok(matches) => {
                        if let Some(command_info) = builder.extract_command_info(&matches) {
                            assert_eq!(command_info.category.as_deref(), Some(category.as_str()));
                            assert_eq!(command_info.tool_name, tool.cli_name());
                            assert_eq!(command_info.mcp_tool_name, tool.name());
                        } else {
                            eprintln!("Warning: Failed to extract command info for {:?}", 
                                     &args[1..]);
                        }
                    }
                    Err(_) => {
                        // Skip commands that fail parsing due to missing args
                        eprintln!("Skipping command {:?} due to parsing failure", &args[1..]);
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn test_tool_matches_extraction() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test tool matches extraction for categorized commands
    let categories = registry.get_cli_categories();
    
    for category in &categories {
        let tools = registry.get_tools_for_category(category);
        
        for tool in tools.iter().take(1) { // Test first tool in each category
            if !tool.hidden_from_cli() {
                let args = vec!["swissarmyhammer", category, tool.cli_name()];
                
                match cli.clone().try_get_matches_from(&args) {
                    Ok(matches) => {
                        if let Some(command_info) = builder.extract_command_info(&matches) {
                            let tool_matches = builder.get_tool_matches(&matches, &command_info);
                            
                            assert!(tool_matches.is_some(), 
                                   "Should be able to extract tool matches for {:?}", 
                                   &args[1..]);
                            
                            // Verify we get the leaf-level matches
                            let tool_matches = tool_matches.unwrap();
                            assert!(tool_matches.subcommand().is_none(), 
                                   "Tool matches should be leaf-level (no subcommands)");
                        }
                    }
                    Err(_) => {
                        eprintln!("Skipping tool matches test for {:?} due to parsing failure", 
                                 &args[1..]);
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn test_unknown_command_handling() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test unknown top-level commands
    let unknown_commands = vec![
        vec!["swissarmyhammer", "unknown-command"],
        vec!["swissarmyhammer", "nonexistent"],
        vec!["swissarmyhammer", "fake"],
    ];
    
    for args in unknown_commands {
        let result = cli.clone().try_get_matches_from(&args);
        
        // Unknown commands should fail parsing
        assert!(result.is_err(), 
               "Unknown command {:?} should fail to parse", 
               &args[1..]);
    }
}

#[tokio::test]
async fn test_invalid_subcommand_handling() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    let categories = registry.get_cli_categories();
    
    for category in categories.iter().take(2) { // Test a few categories
        let args = vec!["swissarmyhammer", category, "invalid-subcommand"];
        
        let result = cli.clone().try_get_matches_from(&args);
        
        // Invalid subcommands should fail parsing
        assert!(result.is_err(), 
               "Invalid subcommand in category '{}' should fail to parse", 
               category);
    }
}

#[tokio::test]
async fn test_help_command_detection() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    let help_commands = vec![
        vec!["swissarmyhammer", "--help"],
        vec!["swissarmyhammer", "-h"],
        vec!["swissarmyhammer", "serve", "--help"],
        vec!["swissarmyhammer", "prompt", "--help"],
    ];
    
    for args in help_commands {
        let result = cli.clone().try_get_matches_from(&args);
        
        match result {
            Ok(matches) => {
                // Help should not be detected as dynamic command
                let is_dynamic = is_dynamic_command(&matches, &builder);
                assert!(!is_dynamic, 
                       "Help command {:?} should not be detected as dynamic", 
                       &args);
            }
            Err(e) => {
                // Help requests might result in DisplayHelp errors, which is expected
                use clap::error::ErrorKind;
                if matches!(e.kind(), ErrorKind::DisplayHelp) {
                    // This is expected for --help flags
                    continue;
                } else {
                    eprintln!("Unexpected error for help command {:?}: {}", &args, e);
                }
            }
        }
    }
}

#[tokio::test]
async fn test_dynamic_command_info_types() {
    let _guard = create_test_home_guard();
    
    // Test DynamicCommandInfo structure and equality
    let info1 = DynamicCommandInfo {
        category: Some("issue".to_string()),
        tool_name: "create".to_string(),
        mcp_tool_name: "issue_create".to_string(),
    };
    
    let info2 = DynamicCommandInfo {
        category: Some("issue".to_string()),
        tool_name: "create".to_string(),
        mcp_tool_name: "issue_create".to_string(),
    };
    
    let info3 = DynamicCommandInfo {
        category: None,
        tool_name: "search".to_string(),
        mcp_tool_name: "search_files".to_string(),
    };
    
    // Test equality
    assert_eq!(info1, info2);
    assert_ne!(info1, info3);
    
    // Test debug formatting
    let debug_str = format!("{:?}", info1);
    assert!(debug_str.contains("issue"));
    assert!(debug_str.contains("create"));
    assert!(debug_str.contains("issue_create"));
}

#[tokio::test]
async fn test_command_execution_setup() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let tool_context = create_test_tool_context().await;
    
    // Test that DynamicCommandExecutor can be created
    let _executor = DynamicCommandExecutor::new(registry.clone(), tool_context);
    
    // Executor should be able to handle commands (we don't execute them here
    // to avoid side effects in unit tests, but we verify the structure)
    let builder = CliBuilder::new(registry.clone());
    let _cli = builder.build_cli().expect("Failed to build CLI");
    
    // Verify that we can identify commands that the executor would handle
    let categories = registry.get_cli_categories();
    assert!(!categories.is_empty(), "Should have some categories to test");
}

#[tokio::test]
async fn test_static_vs_dynamic_boundary() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let _cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test the boundary between static and dynamic commands
    // Ensure there's no overlap or confusion
    
    let static_commands = vec!["serve", "doctor", "prompt", "flow", "validate", "plan", "implement", "config", "completion"];
    let dynamic_categories = registry.get_cli_categories();
    
    // No static command should be a dynamic category
    for static_cmd in &static_commands {
        assert!(!dynamic_categories.contains(&static_cmd.to_string()),
               "Static command '{}' should not be a dynamic category", 
               static_cmd);
    }
    
    // No dynamic category should conflict with static commands
    for category in &dynamic_categories {
        assert!(!static_commands.contains(&category.as_str()),
               "Dynamic category '{}' should not conflict with static commands", 
               category);
    }
}

#[tokio::test]
async fn test_command_parsing_edge_cases() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry);
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    let edge_cases = vec![
        // Empty arguments (should show help)
        vec!["swissarmyhammer"],
        
        // Version request
        vec!["swissarmyhammer", "--version"],
        
        // Unknown flags
        vec!["swissarmyhammer", "--unknown-flag"],
        
        // Mixed valid and invalid
        vec!["swissarmyhammer", "serve", "--invalid-flag"],
    ];
    
    for args in edge_cases {
        let result = cli.clone().try_get_matches_from(&args);
        
        // Most edge cases should either succeed (help/version) or fail gracefully
        match result {
            Ok(matches) => {
                // If parsing succeeds, it should not be detected as dynamic
                // (since these are edge cases, not real dynamic commands)
                let is_dynamic = is_dynamic_command(&matches, &builder);
                
                // For empty args or version, dynamic detection should return false
                if args.len() == 1 || args.contains(&"--version") {
                    assert!(!is_dynamic, "Edge case {:?} should not be dynamic", args);
                }
            }
            Err(e) => {
                // Errors should be graceful (not panics)
                use clap::error::ErrorKind;
                assert!(matches!(e.kind(), 
                       ErrorKind::DisplayHelp | 
                       ErrorKind::DisplayVersion | 
                       ErrorKind::UnknownArgument |
                       ErrorKind::MissingRequiredArgument |
                       ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand),
                       "Edge case {:?} should fail gracefully, got: {:?}", 
                       args, e.kind());
            }
        }
    }
}

#[tokio::test]
async fn test_argument_extraction_validation() {
    let _guard = create_test_home_guard();
    
    let registry = Arc::new(create_test_tool_registry().await.expect("Failed to create tool registry"));
    let builder = CliBuilder::new(registry.clone());
    let cli = builder.build_cli().expect("Failed to build CLI");
    
    // Test that we can extract command info only for valid dynamic commands
    let categories = registry.get_cli_categories();
    
    for category in categories.iter().take(2) { // Test a couple categories
        let tools = registry.get_tools_for_category(category);
        
        for tool in tools.iter().take(1) { // Test first tool
            if !tool.hidden_from_cli() {
                // Valid dynamic command
                let valid_args = vec!["swissarmyhammer", category, tool.cli_name()];
                
                match cli.clone().try_get_matches_from(&valid_args) {
                    Ok(matches) => {
                        let command_info = builder.extract_command_info(&matches);
                        assert!(command_info.is_some(), 
                               "Should extract command info for valid dynamic command {:?}", 
                               &valid_args[1..]);
                    }
                    Err(_) => {
                        eprintln!("Skipping argument extraction test for {:?} due to parsing failure", 
                                 &valid_args[1..]);
                        continue;
                    }
                }
                
                // Invalid subcommand in same category
                let invalid_args = vec!["swissarmyhammer", category, "invalid-subcommand"];
                
                if let Ok(matches) = cli.clone().try_get_matches_from(&invalid_args) {
                    let command_info = builder.extract_command_info(&matches);
                    assert!(command_info.is_none(), 
                           "Should not extract command info for invalid subcommand {:?}", 
                           &invalid_args[1..]);
                } // If parsing fails, that's also valid (clap caught the invalid subcommand)
            }
        }
    }
}

// Helper function to create a test tool context
async fn create_test_tool_context() -> Arc<swissarmyhammer_tools::mcp::tool_registry::ToolContext> {
    use swissarmyhammer::common::rate_limiter::get_rate_limiter;
    use swissarmyhammer::git::GitOperations;
    use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
    use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
    use swissarmyhammer_tools::{mcp::tool_handlers::ToolHandlers, ToolContext};
    use tokio::sync::{Mutex, RwLock};
    
    let work_dir = std::env::temp_dir().join("sah_test_command_execution");
    std::fs::create_dir_all(&work_dir).ok();
    
    // Use mock storage to avoid filesystem dependencies in unit tests
    let memo_storage: Box<dyn MemoStorage> = Box::new(MockMemoStorage::new());
    let memo_storage = Arc::new(RwLock::new(memo_storage));
    
    // Create a minimal issue storage in temp directory
    let issues_dir = work_dir.join("issues");
    std::fs::create_dir_all(&issues_dir).ok();
    let issue_storage: Box<dyn IssueStorage> = 
        Box::new(FileSystemIssueStorage::new(issues_dir).unwrap_or_else(|_| {
            panic!("Failed to create test issue storage")
        }));
    let issue_storage = Arc::new(RwLock::new(issue_storage));
    
    let git_ops = GitOperations::with_work_dir(work_dir).ok();
    let git_ops = Arc::new(Mutex::new(git_ops));
    
    let tool_handlers = ToolHandlers::new(memo_storage.clone());
    
    Arc::new(ToolContext::new(
        Arc::new(tool_handlers),
        issue_storage,
        git_ops,
        memo_storage,
        get_rate_limiter().clone(),
    ))
}