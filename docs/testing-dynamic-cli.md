# Testing Dynamic CLI Architecture

This document provides comprehensive guidance for testing SwissArmyHammer's dynamic CLI architecture, including unit tests, integration tests, and validation approaches.

## Overview

The dynamic CLI architecture requires testing at multiple levels:

1. **Schema Validation Tests** - Ensure JSON schemas convert properly to CLI arguments
2. **Tool Registration Tests** - Verify tools are properly discovered and registered
3. **CLI Generation Tests** - Test that CLI commands are generated correctly
4. **Dynamic Execution Tests** - Test end-to-end command execution
5. **Integration Tests** - Test real CLI usage scenarios

## Schema Validation Testing

### Basic Schema Conversion

Test that JSON schemas convert to appropriate Clap arguments:

```rust
#[cfg(test)]
mod schema_tests {
    use super::*;
    use crate::schema_conversion::SchemaConverter;
    
    #[test]
    fn test_basic_schema_conversion() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the item"
                },
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "Number of items"
                },
                "active": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether item is active"
                }
            },
            "required": ["title"]
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        
        // Verify all properties converted
        assert_eq!(args.len(), 3);
        
        // Verify title argument
        let title_arg = args.iter().find(|a| a.get_id() == "title").unwrap();
        assert!(title_arg.is_required_set());
        assert_eq!(title_arg.get_help().unwrap(), "Title of the item");
        
        // Verify count argument with validation
        let count_arg = args.iter().find(|a| a.get_id() == "count").unwrap();
        assert!(!count_arg.is_required_set());
        assert_eq!(count_arg.get_help().unwrap(), "Number of items");
        
        // Verify boolean flag
        let active_arg = args.iter().find(|a| a.get_id() == "active").unwrap();
        assert!(!active_arg.is_required_set());
        assert_eq!(active_arg.get_help().unwrap(), "Whether item is active");
    }
    
    #[test]
    fn test_enum_schema_conversion() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["json", "yaml", "table"],
                    "default": "table",
                    "description": "Output format"
                }
            },
            "required": []
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let format_arg = args.iter().find(|a| a.get_id() == "format").unwrap();
        
        // Verify enum becomes possible values
        let possible_values = format_arg.get_possible_values();
        assert!(possible_values.iter().any(|v| v.get_name() == "json"));
        assert!(possible_values.iter().any(|v| v.get_name() == "yaml"));
        assert!(possible_values.iter().any(|v| v.get_name() == "table"));
    }
    
    #[test]
    fn test_array_schema_conversion() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of tags"
                }
            }
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let tags_arg = args.iter().find(|a| a.get_id() == "tags").unwrap();
        
        // Verify array allows multiple values
        assert!(tags_arg.get_action().takes_values());
        assert_eq!(tags_arg.get_help().unwrap(), "List of tags");
    }
}
```

### Schema Validation Edge Cases

```rust
#[cfg(test)]
mod schema_validation_tests {
    use super::*;
    
    #[test]
    fn test_invalid_schema_handling() {
        let invalid_schema = serde_json::json!({
            "type": "object",
            // Missing properties field
            "required": ["nonexistent_field"]
        });
        
        let result = SchemaConverter::schema_to_clap_args(&invalid_schema);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_unsupported_types() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "complex": {
                    "type": "object",
                    "description": "Nested object (unsupported)"
                }
            }
        });
        
        // Should skip unsupported types gracefully
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert!(args.is_empty()); // Complex types are skipped
    }
    
    #[test]
    fn test_schema_with_defaults() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "priority": {
                    "type": "integer",
                    "default": 3,
                    "description": "Priority level"
                }
            }
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let priority_arg = args.iter().find(|a| a.get_id() == "priority").unwrap();
        
        // Default values make arguments optional
        assert!(!priority_arg.is_required_set());
    }
}
```

## Tool Registration Testing

### Tool Discovery

```rust
#[cfg(test)]
mod tool_registry_tests {
    use super::*;
    use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    
    #[tokio::test]
    async fn test_tool_registry_initialization() {
        let registry = ToolRegistry::new();
        
        // Verify registry is not empty
        let categories = registry.get_cli_categories();
        assert!(!categories.is_empty(), "Registry should contain tool categories");
        
        // Verify common categories exist
        assert!(categories.contains(&"memo"), "Should have memo category");
        assert!(categories.contains(&"issue"), "Should have issue category");
        assert!(categories.contains(&"files"), "Should have files category");
    }
    
    #[test]
    fn test_tool_lookup_by_cli_name() {
        let registry = ToolRegistry::new();
        
        // Test successful lookup
        let tool = registry.get_tool_by_cli_name("memo", "create");
        assert!(tool.is_some(), "Should find memo create tool");
        
        let tool = tool.unwrap();
        assert_eq!(tool.cli_category().unwrap(), "memo");
        assert_eq!(tool.cli_name(), "create");
        
        // Test failed lookup
        let missing_tool = registry.get_tool_by_cli_name("nonexistent", "action");
        assert!(missing_tool.is_none(), "Should not find nonexistent tool");
    }
    
    #[test]
    fn test_category_tool_listing() {
        let registry = ToolRegistry::new();
        
        let memo_tools = registry.get_tools_for_category("memo");
        assert!(!memo_tools.is_empty(), "Memo category should have tools");
        
        // Verify tools have correct CLI metadata
        for tool in memo_tools {
            assert!(tool.cli_category().is_some(), "Tool should have CLI category");
            assert!(!tool.cli_name().is_empty(), "Tool should have CLI name");
            if !tool.hidden_from_cli() {
                assert!(tool.cli_about().is_some(), "Visible tools should have description");
            }
        }
    }
}
```

### CLI Metadata Validation

```rust
#[cfg(test)]
mod cli_metadata_tests {
    use super::*;
    
    #[test]
    fn test_tool_cli_metadata_completeness() {
        let registry = ToolRegistry::new();
        
        for category in registry.get_cli_categories() {
            let tools = registry.get_tools_for_category(&category);
            
            for tool in tools {
                if !tool.hidden_from_cli() {
                    // Visible tools should have complete CLI metadata
                    assert!(tool.cli_category().is_some(), 
                        "Tool {} should have CLI category", tool.name());
                    assert!(!tool.cli_name().is_empty(),
                        "Tool {} should have CLI name", tool.name());
                    
                    // Verify naming conventions
                    let category = tool.cli_category().unwrap();
                    assert!(category.chars().all(|c| c.is_lowercase() || c == '_'),
                        "Category should be lowercase: {}", category);
                    
                    let cli_name = tool.cli_name();
                    assert!(cli_name.chars().all(|c| c.is_lowercase() || c == '_'),
                        "CLI name should be lowercase: {}", cli_name);
                }
            }
        }
    }
    
    #[test]
    fn test_no_duplicate_cli_commands() {
        let registry = ToolRegistry::new();
        let mut seen_commands = std::collections::HashSet::new();
        
        for category in registry.get_cli_categories() {
            let tools = registry.get_tools_for_category(&category);
            
            for tool in tools {
                if !tool.hidden_from_cli() {
                    let command_path = format!("{} {}", 
                        tool.cli_category().unwrap(), 
                        tool.cli_name()
                    );
                    
                    assert!(!seen_commands.contains(&command_path),
                        "Duplicate CLI command: {}", command_path);
                    seen_commands.insert(command_path);
                }
            }
        }
    }
}
```

## CLI Generation Testing

### Dynamic CLI Building

```rust
#[cfg(test)]
mod cli_generation_tests {
    use super::*;
    use crate::dynamic_cli::CliBuilder;
    
    #[test]
    fn test_cli_generation() {
        let registry = Arc::new(ToolRegistry::new());
        let cli_builder = CliBuilder::new(registry);
        
        let cli = cli_builder.build_cli();
        
        // Verify CLI structure
        assert_eq!(cli.get_name(), "swissarmyhammer");
        assert!(cli.get_version().is_some());
        assert!(cli.get_about().is_some());
        
        // Verify subcommands exist
        let subcommands: Vec<_> = cli.get_subcommands().map(|s| s.get_name()).collect();
        assert!(subcommands.contains(&"serve"), "Should have serve command");
        
        // Verify dynamic commands
        assert!(subcommands.contains(&"memo"), "Should have memo commands");
        assert!(subcommands.contains(&"issue"), "Should have issue commands");
    }
    
    #[test]
    fn test_category_command_structure() {
        let registry = Arc::new(ToolRegistry::new());
        let cli_builder = CliBuilder::new(registry);
        let cli = cli_builder.build_cli();
        
        // Get memo category command
        let memo_cmd = cli.find_subcommand("memo").unwrap();
        assert!(memo_cmd.get_about().is_some());
        
        // Verify memo has tool subcommands
        let memo_subcommands: Vec<_> = memo_cmd.get_subcommands()
            .map(|s| s.get_name()).collect();
        assert!(memo_subcommands.contains(&"create"), "Should have create command");
        assert!(memo_subcommands.contains(&"list"), "Should have list command");
    }
    
    #[test]
    fn test_tool_command_arguments() {
        let registry = Arc::new(ToolRegistry::new());
        let cli_builder = CliBuilder::new(registry);
        let cli = cli_builder.build_cli();
        
        // Get memo create command
        let memo_cmd = cli.find_subcommand("memo").unwrap();
        let create_cmd = memo_cmd.find_subcommand("create").unwrap();
        
        // Verify arguments from schema
        let args: Vec<_> = create_cmd.get_arguments().map(|a| a.get_id()).collect();
        assert!(args.contains(&"title"), "Should have title argument");
        assert!(args.contains(&"content"), "Should have content argument");
        
        // Verify required arguments
        let title_arg = create_cmd.get_arguments()
            .find(|a| a.get_id() == "title").unwrap();
        assert!(title_arg.is_required_set(), "Title should be required");
    }
}
```

### Help Generation Testing

```rust
#[cfg(test)]
mod help_generation_tests {
    use super::*;
    
    #[test]
    fn test_help_text_generation() {
        let registry = Arc::new(ToolRegistry::new());
        let cli_builder = CliBuilder::new(registry);
        let cli = cli_builder.build_cli();
        
        // Test main help
        let help_output = cli.render_help();
        let help_str = help_output.to_string();
        assert!(help_str.contains("swissarmyhammer"));
        assert!(help_str.contains("MCP server"));
        
        // Test category help
        let memo_cmd = cli.find_subcommand("memo").unwrap();
        let memo_help = memo_cmd.render_help().to_string();
        assert!(memo_help.contains("memo"));
        assert!(memo_help.contains("create"));
        assert!(memo_help.contains("list"));
    }
    
    #[test]
    fn test_argument_help() {
        let registry = Arc::new(ToolRegistry::new());
        let cli_builder = CliBuilder::new(registry);
        let cli = cli_builder.build_cli();
        
        let memo_cmd = cli.find_subcommand("memo").unwrap();
        let create_cmd = memo_cmd.find_subcommand("create").unwrap();
        let help_str = create_cmd.render_help().to_string();
        
        // Verify argument help from schema descriptions
        assert!(help_str.contains("--title"));
        assert!(help_str.contains("--content"));
        
        // Verify descriptions appear
        assert!(help_str.contains("Title of"));
        assert!(help_str.contains("Content of") || help_str.contains("content"));
    }
}
```

## Dynamic Execution Testing

### End-to-End Command Execution

```rust
#[cfg(test)]
mod execution_tests {
    use super::*;
    use crate::dynamic_execution::handle_dynamic_command;
    use clap::ArgMatches;
    
    async fn create_test_context() -> Arc<ToolContext> {
        // Create test context with appropriate settings
        Arc::new(ToolContext::new_for_testing())
    }
    
    #[tokio::test]
    async fn test_dynamic_command_execution() {
        let registry = Arc::new(ToolRegistry::new());
        let context = create_test_context().await;
        
        // Create test argument matches
        let matches = create_test_matches("memo", "create", vec![
            ("title", "Test Memo"),
            ("content", "Test content"),
        ]);
        
        let result = handle_dynamic_command(
            "memo", "create", &matches, registry, context
        ).await;
        
        assert!(result.is_ok(), "Command execution should succeed");
    }
    
    #[tokio::test]
    async fn test_missing_required_arguments() {
        let registry = Arc::new(ToolRegistry::new());
        let context = create_test_context().await;
        
        // Missing required title argument
        let matches = create_test_matches("memo", "create", vec![
            ("content", "Test content"),
        ]);
        
        let result = handle_dynamic_command(
            "memo", "create", &matches, registry, context
        ).await;
        
        assert!(result.is_err(), "Should fail with missing required argument");
        let error_str = result.err().unwrap().to_string();
        assert!(error_str.contains("title"), "Error should mention missing title");
    }
    
    #[tokio::test]
    async fn test_nonexistent_tool() {
        let registry = Arc::new(ToolRegistry::new());
        let context = create_test_context().await;
        
        let matches = create_empty_matches();
        
        let result = handle_dynamic_command(
            "nonexistent", "action", &matches, registry, context
        ).await;
        
        assert!(result.is_err(), "Should fail for nonexistent tool");
        let error_str = result.err().unwrap().to_string();
        assert!(error_str.contains("not found"), "Error should indicate tool not found");
    }
    
    fn create_test_matches(category: &str, action: &str, args: Vec<(&str, &str)>) -> ArgMatches {
        // Helper to create ArgMatches for testing
        // Implementation depends on your test setup
        unimplemented!("Create ArgMatches with specified arguments")
    }
    
    fn create_empty_matches() -> ArgMatches {
        // Helper to create empty ArgMatches
        unimplemented!("Create empty ArgMatches")
    }
}
```

### Argument Conversion Testing

```rust
#[cfg(test)]
mod argument_conversion_tests {
    use super::*;
    use crate::schema_conversion::SchemaConverter;
    
    #[test]
    fn test_argument_extraction() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "count": {"type": "integer"},
                "active": {"type": "boolean"}
            },
            "required": ["title"]
        });
        
        let matches = create_test_matches("test", "action", vec![
            ("title", "Test Title"),
            ("count", "42"),
            ("active", "true"),
        ]);
        
        let json_args = SchemaConverter::matches_to_json_args(&matches, &schema).unwrap();
        
        assert_eq!(json_args.get("title").unwrap().as_str().unwrap(), "Test Title");
        assert_eq!(json_args.get("count").unwrap().as_i64().unwrap(), 42);
        assert_eq!(json_args.get("active").unwrap().as_bool().unwrap(), true);
    }
    
    #[test]
    fn test_type_conversion_errors() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer"}
            }
        });
        
        let matches = create_test_matches("test", "action", vec![
            ("count", "not_a_number"),
        ]);
        
        let result = SchemaConverter::matches_to_json_args(&matches, &schema);
        assert!(result.is_err(), "Should fail with type conversion error");
        
        let error_str = result.err().unwrap().to_string();
        assert!(error_str.contains("integer"), "Error should mention integer type");
    }
}
```

## Integration Testing

### CLI Usage Scenarios

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;
    
    #[test]
    fn test_help_commands() {
        // Test main help
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", "--help"])
            .output()
            .expect("Failed to execute command");
        
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("swissarmyhammer"));
        assert!(stdout.contains("COMMANDS"));
        
        // Test category help
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", "memo", "--help"])
            .output()
            .expect("Failed to execute command");
        
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("memo"));
    }
    
    #[test]
    fn test_command_discovery() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", "--help"])
            .output()
            .expect("Failed to execute command");
        
        let stdout = String::from_utf8(output.stdout).unwrap();
        
        // Verify dynamic commands appear in help
        assert!(stdout.contains("memo"), "Should list memo commands");
        assert!(stdout.contains("issue"), "Should list issue commands");
        assert!(stdout.contains("files"), "Should list file commands");
    }
    
    #[tokio::test]
    async fn test_full_command_execution() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let original_dir = std::env::current_dir().unwrap();
        
        // Change to temp directory
        std::env::set_current_dir(temp_dir.path()).unwrap();
        
        // Initialize git repo
        Command::new("git")
            .args(&["init"])
            .output()
            .expect("Failed to init git repo");
        
        // Test a simple command
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", "memo", "list"])
            .output()
            .expect("Failed to execute command");
        
        // Restore directory
        std::env::set_current_dir(original_dir).unwrap();
        
        // Should execute successfully (even if no memos found)
        assert!(output.status.success() || output.status.code() == Some(0));
    }
}
```

### Error Handling Testing

```rust
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_invalid_command_handling() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", "nonexistent", "command"])
            .output()
            .expect("Failed to execute command");
        
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("unrecognized subcommand") || stderr.contains("not found"));
    }
    
    #[test]
    fn test_missing_arguments() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", "memo", "create"])
            .output()
            .expect("Failed to execute command");
        
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("required") || stderr.contains("missing"));
    }
    
    #[test]
    fn test_invalid_argument_types() {
        let output = Command::new("cargo")
            .args(&["run", "--bin", "swissarmyhammer-cli", "--", 
                   "search", "query", "--limit", "not_a_number"])
            .output()
            .expect("Failed to execute command");
        
        assert!(!output.status.success());
        // Should provide clear error about invalid number format
    }
}
```

## Test Utilities

### Mock Tool Registry

```rust
#[cfg(test)]
pub fn create_test_registry() -> Arc<ToolRegistry> {
    let registry = ToolRegistry::new();
    
    // Register test tools if needed
    // This would be used for isolated testing
    
    Arc::new(registry)
}

#[cfg(test)]
pub fn create_test_tool(name: &str, category: &str, cli_name: &str) -> Box<dyn McpTool> {
    struct TestTool {
        name: String,
        category: String,
        cli_name: String,
    }
    
    impl McpTool for TestTool {
        fn name(&self) -> &'static str { &self.name }
        fn cli_category(&self) -> Option<&'static str> { Some(&self.category) }
        fn cli_name(&self) -> &'static str { &self.cli_name }
        fn cli_about(&self) -> Option<&'static str> { Some("Test tool") }
        
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "test_arg": {
                        "type": "string",
                        "description": "Test argument"
                    }
                },
                "required": ["test_arg"]
            })
        }
        
        async fn execute(&self, _: serde_json::Map<String, serde_json::Value>, _: &ToolContext) -> Result<CallToolResult, McpError> {
            Ok(CallToolResult::success("Test result"))
        }
    }
    
    Box::new(TestTool {
        name: name.to_string(),
        category: category.to_string(),
        cli_name: cli_name.to_string(),
    })
}
```

### Test Helpers

```rust
#[cfg(test)]
pub mod test_helpers {
    use super::*;
    
    pub fn validate_cli_structure(cli: &Command) {
        // Validate CLI has proper structure
        assert!(!cli.get_name().is_empty());
        assert!(cli.get_version().is_some());
        assert!(cli.get_about().is_some());
        
        // Validate all subcommands
        for subcommand in cli.get_subcommands() {
            assert!(!subcommand.get_name().is_empty());
            assert!(subcommand.get_about().is_some() || subcommand.get_long_about().is_some());
        }
    }
    
    pub fn validate_tool_schema(tool: &dyn McpTool) {
        let schema = tool.schema();
        
        // Basic schema validation
        assert_eq!(schema.get("type").unwrap().as_str().unwrap(), "object");
        assert!(schema.get("properties").is_some());
        
        // Validate CLI integration
        if !tool.hidden_from_cli() {
            assert!(tool.cli_category().is_some());
            assert!(!tool.cli_name().is_empty());
        }
    }
}
```

## Performance Testing

### Load Testing

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn test_cli_generation_performance() {
        let registry = Arc::new(ToolRegistry::new());
        
        let start = Instant::now();
        let cli_builder = CliBuilder::new(registry);
        let construction_time = start.elapsed();
        
        let start = Instant::now();
        let _cli = cli_builder.build_cli();
        let generation_time = start.elapsed();
        
        // CLI generation should be fast
        assert!(construction_time.as_millis() < 100, 
            "CLI builder construction took too long: {:?}", construction_time);
        assert!(generation_time.as_millis() < 50,
            "CLI generation took too long: {:?}", generation_time);
    }
    
    #[test]
    fn test_tool_lookup_performance() {
        let registry = ToolRegistry::new();
        
        let start = Instant::now();
        for _ in 0..1000 {
            let _tool = registry.get_tool_by_cli_name("memo", "create");
        }
        let lookup_time = start.elapsed();
        
        // Tool lookups should be O(1)
        assert!(lookup_time.as_micros() < 1000,
            "Tool lookups too slow: {:?}", lookup_time);
    }
}
```

## Continuous Integration

### Test Organization

Organize tests into categories for CI efficiency:

```toml
# In Cargo.toml
[package.metadata.scripts]
test-schema = "cargo test schema_tests"
test-registry = "cargo test tool_registry_tests" 
test-cli = "cargo test cli_generation_tests"
test-integration = "cargo test integration_tests"
test-performance = "cargo test performance_tests"
```

### CI Pipeline

```yaml
# .github/workflows/test-dynamic-cli.yml
name: Dynamic CLI Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Schema Tests
        run: cargo test schema_tests --verbose
      
      - name: Registry Tests
        run: cargo test tool_registry_tests --verbose
      
      - name: CLI Generation Tests
        run: cargo test cli_generation_tests --verbose
      
      - name: Integration Tests
        run: cargo test integration_tests --verbose
        
      - name: Performance Tests
        run: cargo test performance_tests --verbose --release
```

## Debugging Test Failures

### Common Issues

1. **Schema Conversion Failures**
   - Check JSON schema syntax
   - Verify all required properties exist
   - Ensure types are supported

2. **Tool Registration Issues**
   - Verify tool implements all CLI metadata methods
   - Check tool is not marked as hidden
   - Ensure tool name follows conventions

3. **CLI Generation Problems**
   - Validate tool schemas before generation
   - Check for duplicate command names
   - Verify help text generation

4. **Execution Failures**
   - Test argument conversion separately
   - Verify tool context is properly initialized
   - Check error handling paths

### Debug Utilities

```rust
#[cfg(test)]
pub fn debug_cli_structure(cli: &Command) {
    eprintln!("CLI: {}", cli.get_name());
    eprintln!("Subcommands:");
    for cmd in cli.get_subcommands() {
        eprintln!("  {}: {}", cmd.get_name(), 
            cmd.get_about().unwrap_or("No description"));
        
        for subcmd in cmd.get_subcommands() {
            eprintln!("    {}: {}", subcmd.get_name(),
                subcmd.get_about().unwrap_or("No description"));
        }
    }
}

#[cfg(test)]
pub fn debug_tool_registry(registry: &ToolRegistry) {
    eprintln!("Tool Registry Categories:");
    for category in registry.get_cli_categories() {
        eprintln!("  {}:", category);
        for tool in registry.get_tools_for_category(&category) {
            eprintln!("    {} ({})", tool.cli_name(), tool.name());
            if tool.hidden_from_cli() {
                eprintln!("      [HIDDEN]");
            }
        }
    }
}
```

This comprehensive testing approach ensures the dynamic CLI architecture works correctly and maintains high quality standards while providing clear debugging capabilities for development and CI environments.