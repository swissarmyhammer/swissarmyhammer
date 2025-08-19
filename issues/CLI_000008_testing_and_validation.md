# Comprehensive Testing and Validation of Dynamic CLI System

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Create comprehensive tests to validate the dynamic CLI system works correctly, maintains backward compatibility, and provides equivalent functionality to the previous static command system.

## Implementation Tasks

### 1. Property-Based Testing for Schema Conversion

Create `swissarmyhammer-cli/tests/property_tests.rs`:

```rust
use proptest::prelude::*;
use serde_json::{json, Value};
use swissarmyhammer_cli::schema_conversion::SchemaConverter;

proptest! {
    #[test]
    fn test_schema_conversion_round_trip(
        prop_name in "[a-zA-Z][a-zA-Z0-9_]*",
        description in ".*",
        required in any::<bool>(),
    ) {
        let schema = json!({
            "type": "object",
            "properties": {
                prop_name: {
                    "type": "string",
                    "description": description
                }
            },
            "required": if required { vec![prop_name.clone()] } else { vec![] }
        });
        
        // Should be able to convert schema to clap args without panicking
        let result = SchemaConverter::schema_to_clap_args(&schema);
        prop_assert!(result.is_ok());
        
        let args = result.unwrap();
        prop_assert_eq!(args.len(), 1);
        
        let arg = &args[0];
        prop_assert_eq!(arg.get_id(), prop_name);
        prop_assert_eq!(arg.is_required_set(), required);
    }
    
    #[test]
    fn test_integer_schema_conversion(
        min_val in -1000i64..1000i64,
        max_val in -1000i64..1000i64,
    ) {
        let min = min_val.min(max_val);
        let max = min_val.max(max_val);
        
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "minimum": min,
                    "maximum": max
                }
            }
        });
        
        let result = SchemaConverter::schema_to_clap_args(&schema);
        prop_assert!(result.is_ok());
        
        let args = result.unwrap();
        let count_arg = &args[0];
        prop_assert!(count_arg.get_help().unwrap().contains(&min.to_string()));
    }
    
    #[test]
    fn test_array_schema_conversion(
        items in prop::collection::vec(".*", 1..5)
    ) {
        let schema = json!({
            "type": "object", 
            "properties": {
                "items": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        });
        
        let result = SchemaConverter::schema_to_clap_args(&schema);
        prop_assert!(result.is_ok());
        
        let args = result.unwrap();
        let items_arg = &args[0];
        prop_assert_eq!(items_arg.get_action(), &clap::ArgAction::Append);
    }
}
```

### 2. CLI Generation Validation Tests

Create `swissarmyhammer-cli/tests/cli_generation_tests.rs`:

```rust
use swissarmyhammer_cli::{build_dynamic_cli, cli_builder::CliBuilder};
use swissarmyhammer_tools::mcp::tool_registry::create_tool_registry;

#[tokio::test]
async fn test_all_static_commands_preserved() {
    let cli = build_dynamic_cli().await.unwrap();
    
    let expected_static_commands = vec![
        "serve", "doctor", "prompt", "flow", 
        "completion", "validate", "plan", "implement"
    ];
    
    for cmd_name in expected_static_commands {
        assert!(
            cli.find_subcommand(cmd_name).is_some(),
            "Static command '{}' not found",
            cmd_name
        );
    }
}

#[tokio::test]
async fn test_dynamic_commands_generated() {
    let cli = build_dynamic_cli().await.unwrap();
    
    let expected_dynamic_commands = vec![
        "issue", "memo", "file", "search"
    ];
    
    for cmd_name in expected_dynamic_commands {
        assert!(
            cli.find_subcommand(cmd_name).is_some(),
            "Dynamic command '{}' not found",
            cmd_name
        );
    }
}

#[tokio::test]
async fn test_issue_subcommands_complete() {
    let cli = build_dynamic_cli().await.unwrap();
    let issue_cmd = cli.find_subcommand("issue").unwrap();
    
    let expected_subcommands = vec![
        "create", "list", "show", "update", "complete", "work", "merge"
    ];
    
    for subcmd_name in expected_subcommands {
        assert!(
            issue_cmd.find_subcommand(subcmd_name).is_some(),
            "Issue subcommand '{}' not found",
            subcmd_name
        );
    }
}

#[tokio::test]
async fn test_memo_subcommands_complete() {
    let cli = build_dynamic_cli().await.unwrap();
    let memo_cmd = cli.find_subcommand("memo").unwrap();
    
    let expected_subcommands = vec![
        "create", "list", "get", "update", "delete", "search", "context"
    ];
    
    for subcmd_name in expected_subcommands {
        assert!(
            memo_cmd.find_subcommand(subcmd_name).is_some(),
            "Memo subcommand '{}' not found", 
            subcmd_name
        );
    }
}

#[tokio::test]
async fn test_file_subcommands_complete() {
    let cli = build_dynamic_cli().await.unwrap();
    let file_cmd = cli.find_subcommand("file").unwrap();
    
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

#[tokio::test]
async fn test_help_text_quality() {
    let cli = build_dynamic_cli().await.unwrap();
    
    // Test top-level help
    assert!(cli.get_about().is_some());
    assert!(!cli.get_about().unwrap().is_empty());
    
    // Test category help
    let issue_cmd = cli.find_subcommand("issue").unwrap();
    assert!(issue_cmd.get_about().is_some());
    assert!(issue_cmd.get_about().unwrap().contains("issue"));
    
    // Test tool help
    let create_cmd = issue_cmd.find_subcommand("create").unwrap();
    assert!(create_cmd.get_about().is_some());
    assert!(!create_cmd.get_about().unwrap().is_empty());
}

#[tokio::test]
async fn test_hidden_tools_excluded() {
    let cli = build_dynamic_cli().await.unwrap();
    
    // Tools marked as hidden should not appear in CLI
    assert!(cli.find_subcommand("todo").is_none());
    assert!(cli.find_subcommand("notify").is_none());
    assert!(cli.find_subcommand("abort").is_none());
}
```

### 3. Command Execution Tests

Create `swissarmyhammer-cli/tests/command_execution_tests.rs`:

```rust
use swissarmyhammer_cli::dynamic_execution::{DynamicCommandExecutor, is_dynamic_command, is_static_command};
use swissarmyhammer_cli::cli_builder::CliBuilder;
use swissarmyhammer_tools::mcp::tool_registry::create_tool_registry;
use clap::ArgMatches;

#[tokio::test]
async fn test_static_command_detection() {
    let cli = swissarmyhammer_cli::build_dynamic_cli().await.unwrap();
    
    let test_cases = vec![
        vec!["swissarmyhammer", "serve"],
        vec!["swissarmyhammer", "doctor"], 
        vec!["swissarmyhammer", "prompt", "list"],
        vec!["swissarmyhammer", "validate"],
    ];
    
    for args in test_cases {
        let matches = cli.clone().try_get_matches_from(args).unwrap();
        assert!(is_static_command(&matches), "Should be static command");
    }
}

#[tokio::test] 
async fn test_dynamic_command_detection() {
    let cli = swissarmyhammer_cli::build_dynamic_cli().await.unwrap();
    let registry = std::sync::Arc::new(create_tool_registry().await.unwrap());
    let builder = CliBuilder::new(registry);
    
    let test_cases = vec![
        vec!["swissarmyhammer", "issue", "create"],
        vec!["swissarmyhammer", "memo", "list"],
        vec!["swissarmyhammer", "file", "read", "/path/to/file"],
    ];
    
    for args in test_cases {
        let matches = cli.clone().try_get_matches_from(args).unwrap();
        assert!(is_dynamic_command(&matches, &builder), "Should be dynamic command");
    }
}

#[tokio::test]
async fn test_command_info_extraction() {
    let cli = swissarmyhammer_cli::build_dynamic_cli().await.unwrap();
    let registry = std::sync::Arc::new(create_tool_registry().await.unwrap());
    let builder = CliBuilder::new(registry);
    
    // Test issue create command
    let matches = cli.clone().try_get_matches_from(vec![
        "swissarmyhammer", "issue", "create", "--name", "test"
    ]).unwrap();
    
    let command_info = builder.extract_command_info(&matches).unwrap();
    assert_eq!(command_info.category, Some("issue".to_string()));
    assert_eq!(command_info.tool_name, "create");
    assert_eq!(command_info.mcp_tool_name, "issue_create");
}
```

### 4. Integration Tests for End-to-End Functionality

Create `swissarmyhammer-cli/tests/e2e_dynamic_cli_tests.rs`:

```rust
use tokio::process::Command;
use std::env;

#[tokio::test]
async fn test_issue_create_e2e() {
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["issue", "create", "--name", "test-issue", "--content", "Test content"])
        .env("SWISSARMYHAMMER_TEST_MODE", "1")  // Enable test mode
        .output()
        .await
        .unwrap();
        
    if !output.status.success() {
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created issue"));
}

#[tokio::test]
async fn test_memo_list_e2e() {
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["memo", "list"])
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .output()
        .await
        .unwrap();
        
    assert!(output.status.success());
}

#[tokio::test]
async fn test_file_operations_e2e() {
    let temp_file = "/tmp/test_file_cli_dynamic.txt";
    
    // Test file write
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["file", "write", temp_file, "Hello, World!"])
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    
    // Test file read
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["file", "read", temp_file])
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    
    let content = String::from_utf8_lossy(&output.stdout);
    assert!(content.contains("Hello, World!"));
    
    // Cleanup
    let _ = tokio::fs::remove_file(temp_file).await;
}

#[tokio::test] 
async fn test_help_generation_e2e() {
    // Test top-level help
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["--help"])
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    
    let help_text = String::from_utf8_lossy(&output.stdout);
    assert!(help_text.contains("issue"));
    assert!(help_text.contains("memo"));
    assert!(help_text.contains("file"));
    
    // Test category help
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["issue", "--help"])
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    
    let help_text = String::from_utf8_lossy(&output.stdout);
    assert!(help_text.contains("create"));
    assert!(help_text.contains("list"));
    assert!(help_text.contains("show"));
}
```

### 5. Backward Compatibility Tests

Create `swissarmyhammer-cli/tests/backward_compatibility_tests.rs`:

```rust
use tokio::process::Command;
use std::env;

#[tokio::test]
async fn test_all_previous_commands_work() {
    // Test that all commands that worked before still work
    let test_commands = vec![
        // Static commands
        vec!["serve", "--help"],
        vec!["doctor", "--help"],
        vec!["prompt", "list", "--help"],
        vec!["validate", "--help"],
        
        // Dynamic commands (should work identically to before)
        vec!["issue", "list", "--help"],
        vec!["memo", "create", "--help"],
        vec!["file", "glob", "--help"],
        vec!["search", "query", "--help"],
    ];
    
    for args in test_commands {
        let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
            .args(&args)
            .output()
            .await
            .unwrap();
            
        if !output.status.success() {
            eprintln!("Command failed: {:?}", args);
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        assert!(output.status.success(), "Command should succeed: {:?}", args);
    }
}

#[tokio::test]
async fn test_error_handling_preserved() {
    // Test that error handling works the same way
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["issue", "show", "nonexistent-issue"])
        .output()
        .await
        .unwrap();
        
    // Should fail gracefully with proper error message
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("Error"));
}
```

### 6. Performance Tests

Create `swissarmyhammer-cli/tests/performance_tests.rs`:

```rust
use std::time::Instant;
use tokio::process::Command;

#[tokio::test]
async fn test_cli_startup_performance() {
    let start = Instant::now();
    
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["--help"])
        .output()
        .await
        .unwrap();
        
    let duration = start.elapsed();
    
    assert!(output.status.success());
    
    // CLI should start reasonably quickly (within 2 seconds for help)
    assert!(duration.as_secs() < 2, "CLI startup took too long: {:?}", duration);
}

#[tokio::test]
async fn test_dynamic_command_performance() {
    let start = Instant::now();
    
    let output = Command::new(env!("CARGO_BIN_EXE_swissarmyhammer"))
        .args(["issue", "list"])
        .env("SWISSARMYHAMMER_TEST_MODE", "1")
        .output()
        .await
        .unwrap();
        
    let duration = start.elapsed();
    
    assert!(output.status.success());
    
    // Dynamic commands should execute within reasonable time
    assert!(duration.as_secs() < 5, "Dynamic command took too long: {:?}", duration);
}
```

## Success Criteria

- [ ] All property-based tests pass for schema conversion
- [ ] CLI generation tests verify all expected commands are present
- [ ] Command execution tests validate static/dynamic detection works
- [ ] End-to-end tests confirm full functionality
- [ ] Backward compatibility tests ensure no regressions
- [ ] Performance tests validate acceptable startup times
- [ ] Help generation tests verify quality of generated help text
- [ ] Integration tests pass with both static and dynamic commands
- [ ] Error handling tests confirm graceful failure modes

## Architecture Notes

- Comprehensive validation of the dynamic CLI system
- Property-based testing ensures robustness across different schemas
- End-to-end tests validate the complete user experience
- Performance tests ensure the dynamic system doesn't impact usability
- Backward compatibility tests prevent regressions during migration