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