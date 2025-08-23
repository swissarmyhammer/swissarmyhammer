# Dynamic CLI Architecture

This document explains SwissArmyHammer's dynamic CLI architecture, which automatically generates command-line interfaces from MCP (Model Context Protocol) tool definitions. This approach eliminates code duplication and ensures consistency between MCP and CLI interfaces.

## Overview

The dynamic CLI system transforms MCP tools into CLI commands automatically, removing the need for manual CLI command maintenance. When you implement an MCP tool with proper CLI metadata, it becomes available as a CLI command without any additional code changes.

## Architecture Components

### 1. MCP Tool Registry

The `ToolRegistry` serves as the central hub for all MCP tools. It provides:

- Tool discovery and registration
- Category-based organization
- CLI metadata access
- Tool lookup by CLI names

```rust
// Tools are automatically registered in the registry
let registry = ToolRegistry::new();
registry.register_all_tools();

// Tools can be looked up by CLI category and name
let tool = registry.get_tool_by_cli_name("memo", "create");
```

### 2. CLI Builder

The `CliBuilder` dynamically constructs the complete CLI structure by:

1. Loading static commands (serve, doctor, etc.)
2. Querying the tool registry for available tools
3. Generating subcommands for each category
4. Creating arguments from tool schemas

```rust
// Build the complete CLI from registry
let cli_builder = CliBuilder::new(tool_registry);
let cli = cli_builder.build_cli();
```

### 3. Schema Conversion

The `SchemaConverter` handles bidirectional conversion between:

- JSON Schema (from MCP tools) → Clap arguments (for CLI)
- Clap ArgMatches → JSON arguments (for tool execution)

```rust
// Convert MCP schema to CLI arguments
let args = SchemaConverter::schema_to_clap_args(&schema)?;

// Convert CLI input back to tool arguments  
let json_args = SchemaConverter::matches_to_json_args(&matches, &schema)?;
```

### 4. Dynamic Execution

The `dynamic_execution` module orchestrates tool execution:

1. Tool lookup in registry
2. Argument conversion
3. MCP tool execution  
4. Result formatting and display

## MCP Tool CLI Integration

### Basic CLI Metadata

To make an MCP tool available in the CLI, implement these methods in your `McpTool` trait:

```rust
impl McpTool for MyTool {
    fn name(&self) -> &'static str {
        "category_action"  // MCP tool name
    }
    
    // CLI Integration Methods
    fn cli_category(&self) -> Option<&'static str> {
        Some("category")  // CLI category (e.g., "memo", "issue")
    }
    
    fn cli_name(&self) -> &'static str {
        "action"  // CLI command name (e.g., "create", "list")
    }
    
    fn cli_about(&self) -> Option<&'static str> {
        Some("Brief description of what this command does")
    }
    
    fn hidden_from_cli(&self) -> bool {
        false  // Set true to hide from CLI
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
                    "description": "Markdown content"
                }
            },
            "required": ["title", "content"]
        })
    }
}
```

### CLI Command Structure

Tools are automatically organized into this structure:

```
sah <category> <action> [arguments]
```

Examples:
- `sah memo create --title "Notes" --content "Content"`
- `sah issue list --format json`
- `sah files read --absolute-path /path/to/file`

### Schema to CLI Argument Mapping

The system automatically converts JSON Schema types to appropriate CLI arguments:

| JSON Schema Type | CLI Argument Type | Example |
|------------------|-------------------|---------|
| `string` | Text argument | `--title "My Title"` |
| `integer` | Numeric argument | `--count 42` |
| `boolean` | Flag argument | `--verbose` |
| `array` | Multiple values | `--tags tag1 --tags tag2` |
| `enum` | Choice argument | `--format json` |

### Advanced Schema Features

#### Optional vs Required Arguments

```rust
serde_json::json!({
    "type": "object",
    "properties": {
        "title": {"type": "string", "description": "Title (required)"},
        "draft": {"type": "boolean", "description": "Mark as draft (optional)"}
    },
    "required": ["title"]  // Only title is required
})
```

#### Argument Validation

```rust
serde_json::json!({
    "type": "object", 
    "properties": {
        "priority": {
            "type": "integer",
            "minimum": 1,
            "maximum": 5,
            "description": "Priority level (1-5)"
        },
        "format": {
            "type": "string",
            "enum": ["json", "yaml", "table"],
            "description": "Output format"
        }
    }
})
```

#### Default Values

```rust
serde_json::json!({
    "type": "object",
    "properties": {
        "limit": {
            "type": "integer",
            "default": 10,
            "description": "Number of results to return"
        }
    }
})
```

## Implementation Examples

### Simple Tool Example

```rust
pub struct CreateMemoTool;

impl McpTool for CreateMemoTool {
    fn name(&self) -> &'static str { "memo_create" }
    fn cli_category(&self) -> Option<&'static str> { Some("memo") }
    fn cli_name(&self) -> &'static str { "create" }
    fn cli_about(&self) -> Option<&'static str> { 
        Some("Create a new memo with title and content")
    }
    
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the memo"
                },
                "content": {
                    "type": "string", 
                    "description": "Markdown content of the memo"
                }
            },
            "required": ["title", "content"]
        })
    }
    
    async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError> {
        // Implementation here
    }
}
```

Results in CLI command:
```bash
sah memo create --title "My Memo" --content "# Content\n\nSome text"
```

### Complex Tool Example

```rust
pub struct SearchTool;

impl McpTool for SearchTool {
    fn name(&self) -> &'static str { "search_query" }
    fn cli_category(&self) -> Option<&'static str> { Some("search") }
    fn cli_name(&self) -> &'static str { "query" }
    fn cli_about(&self) -> Option<&'static str> {
        Some("Search files with semantic similarity")
    }
    
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "integer",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 100,
                    "description": "Number of results to return"
                },
                "format": {
                    "type": "string",
                    "enum": ["table", "json", "yaml"],
                    "default": "table",
                    "description": "Output format"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "default": false,
                    "description": "Case sensitive search"
                }
            },
            "required": ["query"]
        })
    }
}
```

Results in CLI command with rich options:
```bash
sah search query "error handling" --limit 5 --format json --case-sensitive
```

## Tool Registration

Tools are automatically discovered and registered through the build process:

```rust
// In tool registry initialization
impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        
        // Tools are registered automatically via build macros
        registry.register_all_tools();
        registry
    }
}
```

## Error Handling

The system provides comprehensive error handling with user-friendly messages:

### Conversion Errors
- Missing required arguments
- Invalid argument types
- Schema validation failures
- Unsupported schema features

### Execution Errors  
- Tool not found
- MCP execution failures
- Result formatting errors

### Example Error Messages

```
Error: Missing required argument '--title' for tool 'create'.
Use '--help' to see all required arguments.

Error: Invalid type for argument '--count': expected integer, got string.
Please check the argument format.

Error: Tool 'invalid' not found in category 'memo'. 
Available tools: [create, list, update, delete]
```

## Testing

### Schema Validation Tests
```rust
#[test]
fn test_schema_conversion() {
    let schema = create_test_schema();
    let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
    // Verify argument generation
}
```

### CLI Integration Tests
```rust
#[tokio::test]
async fn test_dynamic_command_execution() {
    let registry = create_test_registry();
    let matches = create_test_matches();
    
    let result = handle_dynamic_command(
        "memo", "create", &matches, registry, context
    ).await;
    
    assert!(result.is_ok());
}
```

## Performance Characteristics

### Startup Performance
- CLI structure built once at startup
- Tool registry initialization: ~10ms
- Schema conversion: ~1ms per tool
- Total CLI generation: <50ms for 50+ tools

### Runtime Performance  
- Tool lookup: O(1) hash map access
- Argument conversion: O(n) where n = number of arguments
- Execution identical to direct MCP calls

### Memory Usage
- Dynamic structures add ~10MB over static CLI
- Tool registry: ~1KB per registered tool
- Schema cache: ~500B per tool schema

## Best Practices

### Tool Development
1. **Clear CLI Names**: Use descriptive, action-oriented names
2. **Comprehensive Schemas**: Include descriptions for all parameters
3. **Validation Rules**: Add appropriate constraints and defaults
4. **Error Handling**: Provide clear error messages
5. **Testing**: Test both MCP and CLI interfaces

### Schema Design
1. **Descriptive Properties**: Good descriptions become help text
2. **Appropriate Types**: Use correct JSON Schema types
3. **Validation**: Add constraints where appropriate
4. **Defaults**: Provide sensible defaults for optional parameters
5. **Required Fields**: Mark truly required fields only

### Category Organization
1. **Logical Grouping**: Group related tools in categories
2. **Consistent Naming**: Use consistent action names across categories
3. **Category Descriptions**: Provide helpful category-level documentation

## Migration from Static Commands

When migrating static commands to dynamic tools:

1. **Identify Command Structure**: Map existing commands to categories/actions
2. **Create MCP Tools**: Implement tools with proper CLI metadata  
3. **Remove Static Code**: Delete old command enums and handlers
4. **Update Tests**: Modify tests to use dynamic execution
5. **Validate Functionality**: Ensure all features work identically

## Troubleshooting

### Common Issues

**Tool Not Found**
- Verify tool is registered in the registry
- Check `cli_category()` and `cli_name()` return correct values
- Ensure tool is not marked as `hidden_from_cli()`

**Argument Conversion Failures**
- Validate JSON schema syntax
- Ensure schema types are supported
- Check required field specifications

**Help Text Issues**
- Verify schema descriptions are provided
- Check `cli_about()` returns descriptive text
- Ensure parameter descriptions are clear

### Debug Commands

```bash
# List all available dynamic commands
sah --help

# View tool-specific help
sah memo create --help  

# Enable debug logging
RUST_LOG=debug sah memo create --title "Test"
```

## Future Enhancements

- **Shell Completion**: Generate completions from schemas
- **Interactive Mode**: Prompt for missing required arguments
- **Configuration Files**: Support for tool-specific config files
- **Plugin System**: Dynamic tool loading at runtime
- **Advanced Validation**: Custom validation rules beyond JSON Schema