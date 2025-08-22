# Create Dynamic CLI Builder Infrastructure

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Implement `CliBuilder` struct that can dynamically generate Clap commands from MCP tool registry, replacing static command definitions.

## Technical Details

### CliBuilder Structure
Create in `swissarmyhammer-cli/src/dynamic_cli.rs`:

```rust
use clap::Command;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;

pub struct CliBuilder {
    tool_registry: Arc<ToolRegistry>,
}

impl CliBuilder {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }
    
    pub fn build_cli(&self) -> Command {
        // Build base CLI with static commands
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("MCP server for managing prompts and workflows");
            
        // Add static commands (serve, doctor, etc.)
        cli = self.add_static_commands(cli);
        
        // Add dynamic MCP tool commands
        cli = self.add_dynamic_commands(cli);
        
        cli
    }
    
    fn add_static_commands(&self, cli: Command) -> Command {
        // Add non-MCP commands: serve, doctor, prompt, flow, completion, etc.
    }
    
    fn add_dynamic_commands(&self, cli: Command) -> Command {
        // Add MCP tool-based commands dynamically
    }
    
    fn build_category_command(&self, category: &str) -> Command {
        // Build subcommand for a category (memo, issue, file, etc.)
    }
    
    fn build_tool_command(&self, tool: &dyn McpTool) -> Command {
        // Build individual tool command with arguments from schema
    }
}
```

### Tool Category Discovery
Implement methods to discover and organize MCP tools:

```rust
impl ToolRegistry {
    pub fn get_cli_categories(&self) -> Vec<String> {
        // Return unique categories from tools with CLI integration
    }
    
    pub fn get_tools_for_category(&self, category: &str) -> Vec<&dyn McpTool> {
        // Return tools matching the specified category
    }
    
    pub fn get_cli_tools(&self) -> Vec<&dyn McpTool> {
        // Return all tools that should appear in CLI (not hidden)
    }
}
```

### Integration Pattern
- Preserve existing static commands (serve, doctor, prompt, flow, completion, validate, plan, implement)
- Add dynamic categories (memo, issue, file, search, web-search, config, shell) 
- Each category becomes a subcommand with tool-specific subcommands
- Use schema conversion from previous step to generate arguments

## Acceptance Criteria
- [ ] CliBuilder struct with registry integration
- [ ] Dynamic command generation from MCP tools
- [ ] Category-based organization of commands
- [ ] Schema-to-argument conversion integration
- [ ] Static command preservation
- [ ] Tool discovery and filtering methods
- [ ] Unit tests for CLI building logic
- [ ] Generated CLI matches existing structure for MCP tools

## Implementation Notes
- Start with empty dynamic sections, build incrementally
- Ensure static commands remain unchanged
- Focus on correct category mapping
- Plan for command help text generation