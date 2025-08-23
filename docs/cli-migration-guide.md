# CLI Migration Guide

This guide documents the migration from SwissArmyHammer's static CLI command architecture to the new dynamic CLI system that automatically generates commands from MCP tools.

## Migration Overview

The CLI migration eliminated over 600 lines of redundant command definition code by replacing static Clap command enums with dynamic command generation from MCP tool schemas.

### What Changed

**Before (Static Architecture)**:
- CLI commands defined in static enums (`IssueCommands`, `MemoCommands`, etc.)
- Duplicate parameter definitions between CLI and MCP
- Manual maintenance of CLI command structure
- Separate handlers for CLI and MCP interfaces

**After (Dynamic Architecture)**:
- CLI commands automatically generated from MCP tool schemas
- Single source of truth for command definitions
- Zero maintenance for CLI command structure
- Unified execution path for CLI and MCP

### Benefits of the Migration

1. **Eliminated Redundancy** - Removed ~600 lines of duplicate command definitions
2. **Single Source of Truth** - MCP tool schemas drive both interfaces  
3. **Automatic CLI Generation** - New tools appear in CLI without code changes
4. **Perfect Consistency** - CLI and MCP interfaces never drift apart
5. **Simplified Development** - Adding tools requires no CLI-specific code

## For End Users

### No Breaking Changes

The migration was designed to be completely transparent to end users:

- **All commands work identically** - Same syntax, same behavior
- **Help text improved** - More consistent and detailed help messages
- **New commands available** - Additional tools now accessible via CLI

### Command Structure Unchanged

```bash
# These commands work exactly the same as before
sah issue create --name "feature" --content "Description"
sah memo list --format json
sah search query --query "error handling"
```

### New Commands Available

Some MCP tools that weren't previously available in the CLI are now automatically accessible:

```bash
# File operations
sah files read --absolute-path ./src/main.rs
sah files write --file-path ./output.txt --content "Hello"
sah files grep --pattern "function" --path ./src

# Web tools  
sah web fetch --url https://example.com
sah web search --query "rust async programming"

# Shell integration
sah shell execute --command "cargo test"

# Workflow control
sah abort create --reason "User cancellation"
sah notify create --message "Build complete"
```

## For Contributors

### Tool Development Changes

#### Before: Static Command Development

When adding a new tool category, you had to:

1. **Create CLI enum** in `Commands`:
```rust
pub enum Commands {
    // ... existing commands
    NewCategory { subcommand: NewCategoryCommands },
}
```

2. **Define subcommands enum**:
```rust
#[derive(Subcommand, Debug)]
pub enum NewCategoryCommands {
    Create { 
        title: String,
        content: String,
    },
    List {
        format: Option<String>,
    },
}
```

3. **Add command handler**:
```rust
match commands {
    Commands::NewCategory { subcommand } => {
        handle_new_category_command(subcommand).await?;
    }
}
```

4. **Create MCP tool separately** with duplicate parameter definitions

#### After: Dynamic Tool Development

Now you only need to:

1. **Implement MCP tool with CLI metadata**:
```rust
pub struct CreateTool;

impl McpTool for CreateTool {
    fn name(&self) -> &'static str { "newcategory_create" }
    
    // CLI integration metadata
    fn cli_category(&self) -> Option<&'static str> { Some("newcategory") }
    fn cli_name(&self) -> &'static str { "create" }
    fn cli_about(&self) -> Option<&'static str> { 
        Some("Create a new item with title and content")
    }
    
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the item"
                },
                "content": {
                    "type": "string",
                    "description": "Content of the item"
                }
            },
            "required": ["title", "content"]
        })
    }
    
    async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError> {
        // Implementation
    }
}
```

2. **Register with tool registry** (done automatically via build macros)

That's it! The command `sah newcategory create --title "..." --content "..."` is automatically available.

### Migration Patterns

#### CLI Metadata Methods

All MCP tools should implement these CLI integration methods:

```rust
impl McpTool for YourTool {
    // Required: CLI category (groups related commands)
    fn cli_category(&self) -> Option<&'static str> {
        Some("category")  // e.g., "memo", "issue", "files"
    }
    
    // Required: CLI command name within category
    fn cli_name(&self) -> &'static str {
        "action"  // e.g., "create", "list", "update"
    }
    
    // Optional: Description for CLI help
    fn cli_about(&self) -> Option<&'static str> {
        Some("Brief description of what this command does")
    }
    
    // Optional: Hide from CLI (default: false)
    fn hidden_from_cli(&self) -> bool {
        false
    }
}
```

#### Schema Design for CLI

Design JSON schemas that translate well to CLI arguments:

```rust
fn schema(&self) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            // String arguments become --flag value
            "title": {
                "type": "string",
                "description": "Title of the item"  // Becomes help text
            },
            
            // Boolean arguments become flags
            "verbose": {
                "type": "boolean",
                "default": false,
                "description": "Enable verbose output"
            },
            
            // Integer arguments with validation
            "count": {
                "type": "integer",
                "minimum": 1,
                "maximum": 100,
                "description": "Number of items to process"
            },
            
            // Enum arguments become choice lists
            "format": {
                "type": "string",
                "enum": ["json", "yaml", "table"],
                "default": "table",
                "description": "Output format"
            }
        },
        "required": ["title"]  // Required arguments
    })
}
```

#### Testing Migration

Update tests to use the dynamic execution system:

**Before**:
```rust
#[test]
fn test_memo_create_command() {
    let cli = Cli::try_parse_from(["sah", "memo", "create", "--title", "Test"]);
    // Test static command parsing
}
```

**After**:
```rust
#[tokio::test]
async fn test_memo_create_dynamic() {
    let registry = create_test_registry();
    let matches = create_test_matches();
    
    let result = handle_dynamic_command(
        "memo", "create", &matches, registry, context
    ).await;
    
    assert!(result.is_ok());
}
```

### Removed Code Patterns

These patterns are no longer needed and should be removed:

#### Static Command Enums
```rust
// ❌ Remove these patterns
#[derive(Subcommand, Debug)]
pub enum IssueCommands {
    Create { name: String, content: String },
    List { format: Option<String> },
    // ... more commands
}
```

#### Command Handlers
```rust
// ❌ Remove these patterns
async fn handle_issue_command(subcommand: IssueCommands) -> Result<()> {
    match subcommand {
        IssueCommands::Create { name, content } => {
            // Handler logic
        }
        // ... more handlers
    }
}
```

#### Duplicate Parameter Definitions
```rust
// ❌ No longer needed - parameters come from schema
struct CreateIssueArgs {
    name: String,
    content: String,
}
```

## Integration Testing

### CLI Integration Tests

Test that MCP tools work correctly through the CLI:

```rust
#[tokio::test]
async fn test_cli_mcp_integration() {
    let registry = ToolRegistry::new();
    let context = Arc::new(create_test_context());
    
    // Test tool discovery
    let categories = registry.get_cli_categories();
    assert!(categories.contains(&"memo"));
    
    // Test tool lookup
    let tool = registry.get_tool_by_cli_name("memo", "create");
    assert!(tool.is_some());
    
    // Test argument conversion
    let schema = tool.unwrap().schema();
    let test_args = create_test_args();
    let json_args = SchemaConverter::matches_to_json_args(&test_args, &schema);
    assert!(json_args.is_ok());
    
    // Test execution
    let result = handle_dynamic_command(
        "memo", "create", &test_args, Arc::new(registry), context
    ).await;
    assert!(result.is_ok());
}
```

### Schema Validation Tests

Ensure schemas convert properly to CLI arguments:

```rust
#[test]
fn test_schema_to_cli_conversion() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string", "description": "Item title"},
            "count": {"type": "integer", "minimum": 1},
            "active": {"type": "boolean", "default": false}
        },
        "required": ["title"]
    });
    
    let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
    
    // Verify argument generation
    assert!(args.iter().any(|a| a.get_id() == "title"));
    assert!(args.iter().any(|a| a.get_id() == "count"));
    assert!(args.iter().any(|a| a.get_id() == "active"));
    
    // Verify required argument is marked as required
    let title_arg = args.iter().find(|a| a.get_id() == "title").unwrap();
    assert!(title_arg.is_required_set());
}
```

## Troubleshooting Migration Issues

### Tool Not Found in CLI

**Problem**: MCP tool exists but doesn't appear in CLI

**Solutions**:
1. Verify `cli_category()` returns `Some("category")`  
2. Ensure tool is not marked `hidden_from_cli() = true`
3. Check tool is properly registered in the registry
4. Verify tool name follows naming conventions

### Argument Conversion Errors

**Problem**: CLI arguments don't convert to JSON properly

**Solutions**:
1. Validate JSON schema syntax
2. Ensure schema types are supported (string, integer, boolean, array, enum)
3. Check required fields are properly specified
4. Verify schema descriptions are provided

### Help Text Issues

**Problem**: CLI help text is missing or unhelpful

**Solutions**:
1. Add `cli_about()` method returning descriptive text
2. Include `description` fields in schema properties
3. Use clear, action-oriented descriptions
4. Test help generation: `sah category command --help`

### Performance Issues

**Problem**: CLI startup seems slow

**Solutions**:
1. Check number of registered tools (should be <100)
2. Verify schemas are reasonably sized
3. Profile tool registry initialization
4. Consider lazy loading for large tool sets

## Best Practices

### Tool Naming Conventions

- **Categories**: Use singular nouns (`memo`, `issue`, `file`)
- **Commands**: Use action verbs (`create`, `list`, `update`, `delete`)
- **Consistency**: Follow existing patterns in the codebase

### Schema Design

- **Descriptions**: Always include helpful descriptions
- **Types**: Use appropriate JSON Schema types
- **Validation**: Add constraints where meaningful  
- **Defaults**: Provide sensible defaults for optional parameters

### CLI Integration

- **About Text**: Write clear, concise command descriptions
- **Categories**: Group related commands logically
- **Visibility**: Only hide tools that shouldn't be user-facing

### Testing

- **Integration**: Test both MCP and CLI interfaces
- **Schema**: Validate schema conversion
- **Error Handling**: Test error scenarios
- **Help**: Verify help text generation

## Future Considerations

### Planned Enhancements

- **Shell Completion**: Generate completions from schemas
- **Interactive Mode**: Prompt for missing arguments
- **Configuration**: Support for tool-specific configuration
- **Validation**: Enhanced validation beyond JSON Schema

### Backward Compatibility

The dynamic system is designed to be forward-compatible:

- New schema features can be added without breaking existing tools
- CLI generation can be enhanced without tool changes
- Tool registration system supports plugin architectures

## Migration Timeline

This migration was completed in phases:

1. **Phase 1** - Extended MCP tool trait with CLI metadata
2. **Phase 2** - Implemented schema-to-clap conversion
3. **Phase 3** - Created dynamic CLI builder 
4. **Phase 4** - Added dynamic command execution handler
5. **Phase 5** - Migrated tool categories one by one
6. **Phase 6** - Removed static command infrastructure
7. **Phase 7** - Enhanced validation and error handling
8. **Phase 8** - Updated documentation and examples

The migration maintained 100% backward compatibility for end users while providing a much more maintainable architecture for developers.