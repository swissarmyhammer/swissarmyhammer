# CLI Architecture Specification: Eliminating MCP Tool Redundancy

## Problem Statement

The current CLI architecture suffers from significant code duplication between CLI-specific commands and MCP tools. This manifests as:

1. **Redundant Enums**: CLI commands replicate MCP tool parameters in enums like `IssueCommands`, `MemoCommands`, `FileCommands`, etc.
2. **Duplicate Schema Definitions**: Parameter validation and help text are duplicated between CLI parsing and MCP tool schemas
3. **Maintenance Overhead**: Adding new MCP tools requires updating both MCP tool definitions AND CLI-specific enums
4. **Inconsistent Interfaces**: CLI and MCP interfaces can drift apart due to independent maintenance

## Current Architecture Analysis

### CLI Command Structure
```rust
pub enum Commands {
    // CLI-specific commands (keep as-is)
    Serve,
    Doctor,
    Prompt { subcommand: PromptSubcommand },
    Flow { subcommand: FlowSubcommand },
    Completion { shell: Shell },
    Validate { /* ... */ },
    Plan { plan_filename: String },
    Implement,
    
    // MCP tool pass-throughs (problematic)
    Issue { subcommand: IssueCommands },
    Memo { subcommand: MemoCommands }, 
    File { subcommand: FileCommands },
    Search { subcommand: SearchCommands },
    WebSearch { subcommand: WebSearchCommands },
    Config { subcommand: ConfigCommands },
    Shell { subcommand: ShellCommands },
}
```

### MCP Tool Structure
```rust
#[async_trait]
impl McpTool for CreateMemoTool {
    fn name(&self) -> &'static str { "memo_create" }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Title of the memo"},
                "content": {"type": "string", "description": "Markdown content of the memo"}
            },
            "required": ["title", "content"]
        })
    }
}
```

## Proposed Solution

### 1. Dynamic MCP Command Generation

Replace static CLI enums with dynamic command generation from MCP tool definitions:

```rust
pub enum Commands {
    // Pure CLI commands (no MCP equivalent)
    Serve,
    Doctor, 
    Prompt { subcommand: PromptSubcommand },
    Flow { subcommand: FlowSubcommand },
    Completion { shell: Shell },
    Validate { /* ... */ },
    Plan { plan_filename: String },
    Implement,
    
    // Dynamic MCP pass-through to be called with `sah mcp ...`
    Mcp {
        tool_name: String,
        #[command(flatten)]
        args: Vec<String>, // Raw args parsed dynamically
    },
}
```

### 2. MCP Tool Registry Enhancement

Extend the existing `ToolRegistry` to provide CLI metadata:

```rust
pub trait McpTool {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    
    // New CLI integration methods
    fn cli_category(&self) -> Option<&'static str> { None }
    fn cli_name(&self) -> &'static str { self.name() }
    fn cli_about(&self) -> Option<&'static str> { None }
    fn hidden_from_cli(&self) -> bool { false }
    
    async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError>;
}
```

### 3. Dynamic Command Registration

```rust
pub struct CliBuilder {
    tool_registry: Arc<ToolRegistry>,
}

impl CliBuilder {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }
    
    pub fn build_cli(&self) -> Command {
        let mut cli = Command::new("swissarmyhammer")
            // Add static commands
            .subcommand(Command::new("serve"))
            .subcommand(Command::new("doctor"))
            // etc.
            ;
            
        // Add MCP tool commands dynamically
        let tool_categories = self.tool_registry.get_cli_categories();
        for category in tool_categories {
            cli = cli.subcommand(self.build_category_command(category));
        }
        
        cli
    }
    
    fn build_category_command(&self, category: &str) -> Command {
        let mut cmd = Command::new(category);
        
        let tools = self.tool_registry.get_tools_for_category(category);
        for tool in tools {
            if tool.hidden_from_cli() { continue; }
            
            cmd = cmd.subcommand(self.build_tool_command(tool));
        }
        
        cmd
    }
    
    fn build_tool_command(&self, tool: &dyn McpTool) -> Command {
        let schema = tool.schema();
        let mut cmd = Command::new(tool.cli_name());
        
        if let Some(about) = tool.cli_about() {
            cmd = cmd.about(about);
        }
        
        // Convert JSON schema to clap arguments
        cmd = self.schema_to_clap_args(cmd, &schema);
        
        cmd
    }
    
    fn schema_to_clap_args(&self, mut cmd: Command, schema: &serde_json::Value) -> Command {
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            for (prop_name, prop_schema) in properties {
                cmd = cmd.arg(self.json_schema_to_clap_arg(prop_name, prop_schema));
            }
        }
        cmd
    }
    
    fn json_schema_to_clap_arg(&self, name: &str, schema: &serde_json::Value) -> Arg {
        let mut arg = Arg::new(name).long(name);
        
        if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
            arg = arg.help(desc);
        }
        
        match schema.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => arg.action(ArgAction::SetTrue),
            Some("integer") => arg.value_parser(value_parser!(i64)),
            Some("array") => arg.action(ArgAction::Append),
            _ => arg, // string by default
        }
    }
}
```

### 4. Execution Handler

```rust
pub async fn handle_mcp_command(
    category: &str, 
    tool_name: &str, 
    matches: &ArgMatches,
    tool_registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
) -> Result<()> {
    let tool = tool_registry
        .get_tool(&format!("{}_{}", category, tool_name))
        .ok_or_else(|| anyhow!("Tool not found: {}_{}", category, tool_name))?;
        
    // Convert clap matches to JSON arguments
    let arguments = matches_to_json_args(matches, &tool.schema())?;
    
    // Execute via MCP
    let result = tool.execute(arguments, &context).await?;
    
    // Format and display result
    display_mcp_result(result)?;
    
    Ok(())
}

fn matches_to_json_args(matches: &ArgMatches, schema: &serde_json::Value) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut args = serde_json::Map::new();
    
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties {
            if let Some(value) = extract_clap_value(matches, prop_name, prop_schema) {
                args.insert(prop_name.clone(), value);
            }
        }
    }
    
    Ok(args)
}
```

## Migration Strategy

### Phase 1: Infrastructure Setup
1. Extend `McpTool` trait with CLI metadata methods
2. Implement `CliBuilder` with dynamic command generation
3. Add schema-to-clap conversion utilities

### Phase 2: Gradual Migration
1. Start with one category (e.g., `memo`) 
2. Remove `MemoCommands` enum
3. Update existing memo tools with CLI metadata
4. Verify functionality and help text generation

### Phase 3: Full Migration
1. Migrate remaining categories (`issue`, `file`, `search`, etc.) -- one category at a time per issue
2. Remove all redundant command enums
3. Update integration tests

### Phase 4: Enhancement
1. Add validation for schema-to-clap conversion
2. Implement custom help formatting for better UX
3. Add shell completion generation from schemas

## Benefits

### Eliminated Redundancy
- **Single Source of Truth**: MCP tool schemas drive both MCP and CLI interfaces
- **Automatic CLI Generation**: New MCP tools appear in CLI without code changes
- **Consistent Help Text**: Tool descriptions used for both MCP and CLI help

### Improved Maintainability  
- **Reduced Code Duplication**: ~80% reduction in CLI command definitions
- **Simplified Tool Addition**: Add MCP tool → CLI command appears automatically
- **Centralized Validation**: Parameter validation logic in one place

### Enhanced Consistency
- **Unified Interface**: CLI and MCP interfaces stay in sync automatically
- **Consistent Naming**: Tool names, parameters, and descriptions unified
- **Better Documentation**: Schema-driven help generation

## Backward Compatibility

None

## Implementation Considerations

### Schema Validation
- Ensure JSON schemas are compatible with clap argument types
- Add validation for unsupported schema features
- Provide clear error messages for schema conversion failures

### Performance Impact
- Dynamic command building happens once at startup
- Runtime performance identical to static commands
- Memory usage slightly higher due to dynamic structures

### Testing Strategy
- Property-based tests for schema-to-clap conversion
- Integration tests comparing old vs new CLI behavior
- Schema validation tests for all MCP tools

## Success Criteria

1. **Code Reduction**: Remove 8+ redundant command enums (600+ lines)
2. **Functional Parity**: All existing non MCP CLI commands work identically
3. **Help Generation**: Auto-generated help matches or exceeds current quality
4. **Tool Addition**: New MCP tools appear in CLI without CLI code changes

This specification transforms the CLI from a static, duplicative system to a dynamic, schema-driven architecture that eliminates redundancy while maintaining full backward compatibility and improving maintainability.