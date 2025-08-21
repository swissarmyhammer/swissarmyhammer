# Implement Dynamic CLI Builder

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Create the CliBuilder that generates Clap commands dynamically from MCP tool registry, replacing static CLI command definitions.

## Implementation Tasks

### 1. Create CliBuilder Structure

Create `swissarmyhammer-cli/src/cli_builder.rs`:

```rust
use clap::{Command, Subcommand};
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use crate::schema_conversion::SchemaConverter;
use anyhow::Result;

pub struct CliBuilder {
    tool_registry: Arc<ToolRegistry>,
}

impl CliBuilder {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }
    
    /// Build the complete CLI with static and dynamic commands
    pub fn build_cli(&self) -> Result<Command> {
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("An MCP server for managing prompts, workflows, and development tasks")
            .long_about(Self::get_long_about())
            .subcommand_required(false)
            .arg_required_else_help(true);
            
        // Add static CLI-only commands (unchanged)
        cli = self.add_static_commands(cli);
        
        // Add dynamic MCP-based commands
        cli = self.add_dynamic_commands(cli)?;
        
        Ok(cli)
    }
    
    /// Add static commands that have no MCP equivalent
    fn add_static_commands(&self, mut cli: Command) -> Command {
        cli = cli
            .subcommand(Command::new("serve")
                .about("Run as MCP server"))
            .subcommand(Command::new("doctor")
                .about("Diagnose configuration and setup issues"))
            .subcommand(Command::new("prompt")
                .subcommand_required(true)
                // Keep existing prompt subcommands as they are
                .subcommand(Command::new("list")
                    .about("List available prompts"))
                .subcommand(Command::new("test")
                    .about("Test prompt rendering")
                    .arg(clap::Arg::new("name")
                        .required(true)
                        .help("Prompt name"))))
            .subcommand(Command::new("flow") 
                .subcommand_required(true)
                // Keep existing flow subcommands
                .subcommand(Command::new("run")
                    .about("Execute workflow")
                    .arg(clap::Arg::new("workflow")
                        .required(true)
                        .help("Workflow name"))))
            .subcommand(Command::new("completion")
                .about("Generate shell completions")
                .arg(clap::Arg::new("shell")
                    .required(true)
                    .value_parser(clap::value_parser!(clap_complete::Shell))))
            .subcommand(Command::new("validate")
                .about("Validate prompt files and workflows"))
            .subcommand(Command::new("plan")
                .about("Plan a specific specification file")
                .arg(clap::Arg::new("plan_filename")
                    .required(true)
                    .help("Path to plan file")))
            .subcommand(Command::new("implement")
                .about("Execute implement workflow"));
            
        cli
    }
    
    /// Add dynamic commands generated from MCP tools
    fn add_dynamic_commands(&self, mut cli: Command) -> Result<Command> {
        let categories = self.tool_registry.get_cli_categories();
        
        for category in categories {
            let category_cmd = self.build_category_command(&category)?;
            cli = cli.subcommand(category_cmd);
        }
        
        // Add root-level tools (tools without category)
        let root_tools = self.tool_registry.get_root_cli_tools();
        for tool in root_tools {
            let tool_cmd = self.build_tool_command(tool)?;
            cli = cli.subcommand(tool_cmd);
        }
        
        Ok(cli)
    }
    
    /// Build command for a specific category of tools
    fn build_category_command(&self, category: &str) -> Result<Command> {
        let mut cmd = Command::new(category)
            .about(&format!("{} management commands", 
                Self::capitalize_first(category)))
            .subcommand_required(true);
            
        let tools = self.tool_registry.get_tools_for_category(category);
        
        for tool in tools {
            if tool.hidden_from_cli() {
                continue;
            }
            
            let tool_cmd = self.build_tool_command(tool)?;
            cmd = cmd.subcommand(tool_cmd);
        }
        
        Ok(cmd)
    }
    
    /// Build command for individual MCP tool
    fn build_tool_command(&self, tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool) -> Result<Command> {
        let mut cmd = Command::new(tool.cli_name());
        
        // Use CLI-specific about text or fall back to description
        let about = tool.cli_about()
            .unwrap_or_else(|| tool.description())
            .lines()
            .next()  // Use first line for short about
            .unwrap_or(tool.cli_name());
            
        cmd = cmd.about(about);
        
        // Add long description if available
        if let Some(long_about) = tool.cli_about().or_else(|| Some(tool.description())) {
            cmd = cmd.long_about(long_about);
        }
        
        // Convert tool schema to clap arguments
        let schema = tool.schema();
        let args = SchemaConverter::schema_to_clap_args(&schema)?;
        
        for arg in args {
            cmd = cmd.arg(arg);
        }
        
        Ok(cmd)
    }
    
    /// Get application long about text
    fn get_long_about() -> &'static str {
        "swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts, workflows, issues, memos, and development tools. It supports file watching, 
template substitution, and seamless integration with Claude Code.

Example usage:
  swissarmyhammer serve     # Run as MCP server
  swissarmyhammer doctor    # Check configuration and setup
  swissarmyhammer issue create \"Bug fix\"    # Create new issue
  swissarmyhammer memo list                   # List all memos"
    }
    
    /// Capitalize first letter of string
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}
```

### 2. Add Command Execution Context

Create command execution infrastructure:

```rust
#[derive(Debug)]
pub struct DynamicCommandInfo {
    pub category: Option<String>,
    pub tool_name: String,
    pub mcp_tool_name: String,
}

impl CliBuilder {
    /// Extract dynamic command information from matches
    pub fn extract_command_info(&self, matches: &clap::ArgMatches) -> Option<DynamicCommandInfo> {
        // Check each category for matches
        for category in self.tool_registry.get_cli_categories() {
            if let Some((category_name, sub_matches)) = matches.subcommand() {
                if category_name == category {
                    if let Some((tool_name, _)) = sub_matches.subcommand() {
                        // Find the MCP tool name
                        let tools = self.tool_registry.get_tools_for_category(&category);
                        for tool in tools {
                            if tool.cli_name() == tool_name {
                                return Some(DynamicCommandInfo {
                                    category: Some(category),
                                    tool_name: tool_name.to_string(),
                                    mcp_tool_name: tool.name().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        // Check root-level tools
        if let Some((command_name, _)) = matches.subcommand() {
            let root_tools = self.tool_registry.get_root_cli_tools();
            for tool in root_tools {
                if tool.cli_name() == command_name {
                    return Some(DynamicCommandInfo {
                        category: None,
                        tool_name: command_name.to_string(),
                        mcp_tool_name: tool.name().to_string(),
                    });
                }
            }
        }
        
        None
    }
}
```

### 3. Create Testing Infrastructure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
    
    fn create_test_registry() -> ToolRegistry {
        // Create a registry with some test tools for validation
        let mut registry = ToolRegistry::new();
        
        // This would need actual test tools - for now just basic structure
        registry
    }
    
    #[test]
    fn test_static_commands_preserved() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        
        let cli = builder.build_cli().unwrap();
        
        // Verify static commands are present
        assert!(cli.find_subcommand("serve").is_some());
        assert!(cli.find_subcommand("doctor").is_some());
        assert!(cli.find_subcommand("prompt").is_some());
        assert!(cli.find_subcommand("flow").is_some());
        assert!(cli.find_subcommand("completion").is_some());
        assert!(cli.find_subcommand("validate").is_some());
        assert!(cli.find_subcommand("plan").is_some());
        assert!(cli.find_subcommand("implement").is_some());
    }
    
    #[test] 
    fn test_category_commands_generated() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        
        let cli = builder.build_cli().unwrap();
        
        // This will need actual tools in the test registry
        // For now, just verify the build succeeds
        assert!(!cli.get_subcommands().collect::<Vec<_>>().is_empty());
    }
    
    #[test]
    fn test_command_info_extraction() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        
        // This would test the command parsing logic
        // Need actual ArgMatches for full testing
    }
}
```

### 4. Integration with Main CLI Module

Update `swissarmyhammer-cli/src/lib.rs`:

```rust
pub mod cli_builder;
pub use cli_builder::CliBuilder;
```

## Success Criteria

- [ ] CliBuilder generates CLI with static commands preserved
- [ ] Dynamic commands created from MCP tool registry
- [ ] Tool categories become CLI subcommands (issue, memo, file, etc.)
- [ ] Individual tools become nested subcommands
- [ ] Schema-to-clap conversion integrated correctly
- [ ] Help text generated from tool descriptions
- [ ] Command extraction works for dynamic commands
- [ ] Tests verify static commands remain unchanged
- [ ] Tests validate dynamic command generation

## Architecture Notes

- Preserves all existing static CLI commands
- Generates dynamic commands from MCP tool registry
- Uses schema conversion from previous step
- Creates foundation for unified command execution
- Maintains backward compatibility with existing CLI usage
## Proposed Solution

After examining the codebase, I will implement the dynamic CLI builder following the existing patterns and architecture:

### Implementation Approach

1. **CliBuilder Structure**: Create `swissarmyhammer-cli/src/cli_builder.rs` with:
   - Uses existing `ToolRegistry` from `swissarmyhammer-tools`
   - Leverages existing `SchemaConverter` for JSON Schema to Clap conversion  
   - Preserves all static CLI commands exactly as they are
   - Dynamically generates commands from MCP tool categories and individual tools

2. **Key Design Decisions**:
   - Reuse existing schema conversion logic from `schema_conversion.rs`
   - Utilize the MCP tool registry's CLI integration methods (`cli_category()`, `cli_name()`, etc.)
   - Maintain backward compatibility with all existing CLI commands
   - Create structured command info extraction for execution routing

3. **Architecture Integration**:
   - The CliBuilder will integrate with the existing MCP tool context system
   - Uses the same argument parsing and validation patterns
   - Maintains the established error handling patterns
   - Follows the testing patterns established in the codebase

4. **Testing Strategy**:
   - Unit tests for command generation logic
   - Integration tests with real MCP tools
   - Verification that static commands remain unchanged
   - Schema-to-clap conversion validation

### Implementation Steps

1. Create the CliBuilder struct with dynamic command generation
2. Implement command extraction logic for routing dynamic commands
3. Add comprehensive unit and integration tests
4. Integrate with main CLI module
5. Verify all existing functionality preserved

This approach leverages the excellent foundation already established in the codebase, particularly the tool registry pattern and schema conversion utilities.
## Implementation Complete

✅ **Status: COMPLETED**

### What was implemented:

1. **CliBuilder Structure** (`swissarmyhammer-cli/src/cli_builder.rs`):
   - Created complete `CliBuilder` struct with dynamic command generation
   - Integrates with existing `ToolRegistry` and `SchemaConverter`
   - Preserves all static CLI commands exactly as they were
   - Dynamically generates commands from MCP tool categories and individual tools
   - Uses `Box::leak` pattern for 'static string requirements in clap

2. **Command Execution Context**:
   - Implemented `DynamicCommandInfo` struct to capture command routing information
   - Added `extract_command_info()` method to parse CLI matches and identify MCP tools
   - Added `get_tool_matches()` method to extract tool-specific arguments
   - Supports both categorized tools (e.g., `sah issue create`) and root-level tools (e.g., `sah search`)

3. **Schema Integration**:
   - Leverages existing `SchemaConverter` to convert JSON schemas to clap arguments
   - Maintains all existing argument validation and help text generation
   - Uses tool descriptions for CLI help text with fallbacks

4. **Testing Infrastructure**:
   - Created comprehensive test suite with 14 passing tests
   - Tests cover command generation, argument parsing, help text, and edge cases
   - Mock tools created to test various CLI integration scenarios
   - Tests verify static command preservation and dynamic command generation

5. **Module Integration**:
   - Added `cli_builder` module to `swissarmyhammer-cli/src/lib.rs`
   - Added `async-trait` dependency for test compilation
   - Exported `CliBuilder` for easy access
   - Verified compilation in release mode

### Key Features:

- **Backward Compatibility**: All existing static CLI commands preserved exactly
- **Dynamic Generation**: MCP tools automatically become CLI commands
- **Category Organization**: Tools grouped by category become subcommands (e.g., `issue`, `memo`)
- **Schema-Driven**: JSON schemas automatically generate CLI arguments
- **Help Generation**: Tool descriptions become CLI help text
- **Type Safety**: Full type safety with comprehensive error handling
- **Testing**: Extensive test coverage with mock tools and integration tests

### Technical Architecture:

```
CLI Structure Generated:
sah
├── serve                    # Static command (preserved)
├── doctor                   # Static command (preserved)  
├── prompt                   # Static command (preserved)
├── flow                     # Static command (preserved)
├── issue                    # Dynamic category from MCP tools
│   ├── create              # Generated from issue_create MCP tool
│   ├── list                # Generated from issue_list MCP tool
│   └── ...
├── memo                     # Dynamic category from MCP tools
│   ├── create              # Generated from memo_create MCP tool
│   └── ...
└── search                   # Dynamic root-level tool (no category)
```

The implementation successfully eliminates the CLI redundancy problem described in `/ideas/cli.md` by making the CLI structure fully dynamic while preserving all existing functionality.

All tests pass, code compiles successfully, and the implementation follows the established patterns in the codebase.