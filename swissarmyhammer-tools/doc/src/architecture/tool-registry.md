# Tool Registry

The Tool Registry provides a modular, extensible system for managing MCP tools. It replaces the traditional large match statement approach with a pluggable registry pattern that enables dynamic tool registration, validation, and execution.

## Architecture

### Core Components

```rust
pub trait McpTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, arguments: Map, context: &ToolContext) -> Result;
    
    // CLI integration (optional)
    fn cli_category(&self) -> Option<&'static str>;
    fn cli_name(&self) -> &'static str;
    fn cli_about(&self) -> Option<&'static str>;
    fn hidden_from_cli(&self) -> bool;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn McpTool>>,
}
```

### Design Principles

1. **Modularity**: Each tool is self-contained in its own module
2. **Extensibility**: New tools can be added without modifying existing code
3. **Type Safety**: Tools are trait objects with compile-time guarantees
4. **Performance**: HashMap-based lookup provides O(1) tool resolution
5. **Testability**: Tools can be unit tested independently

## McpTool Trait

All MCP tools implement the `McpTool` trait, which defines the interface for tool registration, discovery, and execution.

### Required Methods

#### name()
Returns the tool's unique identifier:
```rust
fn name(&self) -> &'static str {
    "memo_create"
}
```

Tool names follow the `{category}_{action}` pattern:
- `memo_create`, `memo_get`, `memo_list`
- `issue_create`, `issue_show`, `issue_list`
- `files_read`, `files_write`, `files_edit`

#### description()
Returns human-readable documentation:
```rust
fn description(&self) -> &'static str {
    include_str!("description.md")
}
```

Best practice: Load from separate markdown file for maintainability.

#### schema()
Defines JSON Schema for parameter validation:
```rust
fn schema(&self) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "The memo title",
                "minLength": 1
            },
            "content": {
                "type": "string",
                "description": "The memo content in markdown format"
            }
        },
        "required": ["title", "content"]
    })
}
```

Supported types:
- `string`, `integer`, `number`, `boolean`, `array`
- **Not supported**: `object`, `null` (for CLI compatibility)

#### execute()
Implements the tool's business logic:
```rust
async fn execute(
    &self,
    arguments: Map<String, Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    // Parse arguments
    let request: CreateMemoRequest = BaseToolImpl::parse_arguments(arguments)?;
    
    // Execute business logic
    let memo_storage = context.memo_storage.write().await;
    let memo = memo_storage.create_memo(request.title, request.content).await?;
    
    // Return success response
    Ok(BaseToolImpl::create_success_response(
        format!("Created memo: {}", memo.id())
    ))
}
```

### CLI Integration Methods

These optional methods enable dynamic CLI generation:

#### cli_category()
Returns the CLI category for grouping commands:
```rust
fn cli_category(&self) -> Option<&'static str> {
    Some("memo")  // From tool name "memo_create"
}
```

Default implementation extracts category from tool name pattern.

#### cli_name()
Returns the command name within the category:
```rust
fn cli_name(&self) -> &'static str {
    "create"  // From tool name "memo_create"
}
```

#### cli_about()
Returns brief help text for CLI:
```rust
fn cli_about(&self) -> Option<&'static str> {
    Some("Create a new memo with title and content")
}
```

#### hidden_from_cli()
Indicates if tool should be excluded from CLI:
```rust
fn hidden_from_cli(&self) -> bool {
    false  // Show in CLI by default
}
```

## Tool Registry Operations

### Registration

Tools are registered at server startup:

```rust
let mut registry = ToolRegistry::new();

// Register individual tools
registry.register(MemoCreateTool::default());
registry.register(MemoGetTool::default());

// Or use category registration functions
register_memo_tools(&mut registry);
register_issue_tools(&mut registry);
```

Category registration functions keep related tools organized:

```rust
pub fn register_memo_tools(registry: &mut ToolRegistry) {
    registry.register(MemoCreateTool::default());
    registry.register(MemoGetTool::default());
    registry.register(MemoListTool::default());
    registry.register(MemoGetAllContextTool::default());
}
```

### Tool Lookup

Tools can be retrieved by name or CLI path:

```rust
// By tool name
if let Some(tool) = registry.get_tool("memo_create") {
    let result = tool.execute(args, &context).await?;
}

// By CLI category and name
if let Some(tool) = registry.get_tool_by_cli_name("memo", "create") {
    let result = tool.execute(args, &context).await?;
}
```

### Tool Discovery

The registry supports various discovery operations:

```rust
// List all tool names
let names: Vec<String> = registry.list_tool_names();

// Get all tools as MCP Tool objects
let tools: Vec<Tool> = registry.list_tools();

// Get CLI categories
let categories: Vec<String> = registry.get_cli_categories();

// Get tools in a category
let memo_tools: Vec<&dyn McpTool> = registry.get_tools_for_category("memo");
```

## Tool Validation

The registry includes comprehensive validation to ensure CLI compatibility and catch schema errors early.

### Validation Framework

```rust
// Validate all CLI tools
match registry.validate_cli_tools() {
    Ok(()) => println!("All tools are valid"),
    Err(errors) => {
        for error in errors {
            eprintln!("Validation error: {}", error);
        }
    }
}

// Get detailed validation report
let report = registry.validate_all_tools();
println!("{}", report.summary());
```

### Validation Checks

1. **Schema Structure**
   - Must be a valid JSON object
   - Must have `properties` field
   - Parameter types must be supported

2. **CLI Compatibility**
   - Tool must have valid category (if not hidden)
   - CLI name must not be empty
   - Parameter names must be valid identifiers

3. **Schema Parameters**
   - No nested objects (not CLI-compatible)
   - Required fields must exist in properties
   - Types must be: string, integer, number, boolean, array

### Validation Errors

```rust
pub enum ToolValidationError {
    SchemaValidation { tool_name: String, error: ValidationError },
    MissingCliCategory { tool_name: String },
    InvalidCliName { tool_name: String, cli_name: String, reason: String },
    InvalidDescription { tool_name: String, reason: String },
    NameConflict { tool_name: String, conflicting_tool: String },
}
```

### Graceful Degradation

Invalid tools are reported but don't crash the application:

```rust
// Get warnings instead of errors
let warnings = registry.get_tool_validation_warnings();
for warning in warnings {
    tracing::warn!("Tool validation issue: {}", warning);
}

// Build CLI, skipping invalid tools
let cli_builder = CliBuilder::new(registry);
let cli = cli_builder.build_cli_with_warnings();
```

## BaseToolImpl Utilities

The `BaseToolImpl` struct provides common utilities for tool implementations:

### parse_arguments()
Deserialize arguments into typed struct:
```rust
#[derive(Deserialize)]
struct CreateMemoRequest {
    title: String,
    content: String,
}

let request: CreateMemoRequest = BaseToolImpl::parse_arguments(arguments)?;
```

### create_success_response()
Create standardized success response:
```rust
Ok(BaseToolImpl::create_success_response("Operation completed successfully"))
```

### create_error_response()
Create standardized error response:
```rust
Ok(BaseToolImpl::create_error_response(
    "Operation failed",
    Some("Additional details".to_string())
))
```

## Tool Context

Tools receive a `ToolContext` providing access to shared resources:

```rust
pub struct ToolContext {
    pub tool_handlers: Arc<ToolHandlers>,
    pub issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
    pub git_ops: Arc<Mutex<Option<GitOperations>>>,
    pub memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,
    pub agent_config: Arc<AgentConfig>,
}
```

### Using Tool Context

```rust
async fn execute(&self, args: Map, context: &ToolContext) -> Result {
    // Access memo storage
    let memo_storage = context.memo_storage.write().await;
    let memo = memo_storage.create_memo(title, content).await?;
    
    // Access git operations (optional)
    if let Some(git_ops) = context.git_ops.lock().await.as_ref() {
        let changes = git_ops.get_changes()?;
    }
    
    // Access agent configuration
    let agent = context.agent_config.create_executor()?;
    
    Ok(BaseToolImpl::create_success_response("Success"))
}
```

## Creating New Tools

Follow these steps to create a new MCP tool:

### 1. Define the Tool Struct

```rust
#[derive(Default)]
pub struct MyTool;
```

### 2. Implement Request/Response Types

```rust
#[derive(Deserialize)]
struct MyToolRequest {
    parameter: String,
}
```

### 3. Implement McpTool Trait

```rust
#[async_trait]
impl McpTool for MyTool {
    fn name(&self) -> &'static str {
        "my_tool_name"
    }
    
    fn description(&self) -> &'static str {
        include_str!("description.md")
    }
    
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "parameter": {
                    "type": "string",
                    "description": "Description of parameter"
                }
            },
            "required": ["parameter"]
        })
    }
    
    async fn execute(
        &self,
        arguments: Map<String, Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        let request: MyToolRequest = BaseToolImpl::parse_arguments(arguments)?;
        
        // Implement business logic here
        
        Ok(BaseToolImpl::create_success_response("Success"))
    }
}
```

### 4. Create Description File

Create `description.md` with tool documentation:

```markdown
Perform a specific operation with the given parameters.

## Parameters

- `parameter` (required): Description of what this parameter does

## Examples

```json
{
  "parameter": "example value"
}
```

## Returns

Returns confirmation message on success.
```

### 5. Register the Tool

```rust
pub fn register_my_tools(registry: &mut ToolRegistry) {
    registry.register(MyTool::default());
}
```

### 6. Add to Server Registration

In `src/mcp/tool_registry.rs`:

```rust
pub fn register_my_tools(registry: &mut ToolRegistry) {
    use super::tools::my_category;
    my_category::register_my_tools(registry);
}
```

## Testing Tools

### Unit Tests

```rust
#[tokio::test]
async fn test_my_tool_execution() {
    let tool = MyTool::default();
    
    let mut args = Map::new();
    args.insert("parameter".into(), Value::String("test".into()));
    
    let context = create_test_context().await;
    let result = tool.execute(args, &context).await;
    
    assert!(result.is_ok());
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_tool_via_registry() {
    let mut registry = ToolRegistry::new();
    registry.register(MyTool::default());
    
    let tool = registry.get_tool("my_tool_name").unwrap();
    let result = tool.execute(args, &context).await;
    
    assert!(result.is_ok());
}
```

## Best Practices

### Tool Design

1. **Single Responsibility**: Each tool should do one thing well
2. **Clear Naming**: Follow `{category}_{action}` pattern
3. **Comprehensive Schemas**: Define all parameters with descriptions
4. **Error Handling**: Use structured errors with helpful messages
5. **Documentation**: Include examples in description

### Performance

1. **Avoid Blocking**: Use async operations throughout
2. **Efficient Storage Access**: Minimize lock hold time
3. **Resource Cleanup**: Ensure resources are properly released
4. **Caching**: Implement caching at tool level if needed

### Security

1. **Input Validation**: Validate all parameters beyond schema
2. **Path Sanitization**: Prevent directory traversal attacks
3. **Resource Limits**: Prevent unbounded operations
4. **Error Messages**: Don't leak sensitive information

## Migration from Legacy System

The registry pattern replaces the previous delegation-based approach:

### Old Pattern (Delegation)
```rust
match tool_name {
    "memo_create" => handlers.create_memo(args).await,
    "memo_get" => handlers.get_memo(args).await,
    // ... many more matches
}
```

### New Pattern (Registry)
```rust
if let Some(tool) = registry.get_tool(tool_name) {
    tool.execute(args, context).await
}
```

### Benefits

- **No Central Dispatch**: Tools are independently registered
- **Better Testing**: Tools can be tested without full server
- **Easier Extension**: Add tools without modifying core code
- **Type Safety**: Compile-time guarantees for tool interfaces

## Related Documentation

- [MCP Server](./mcp-server.md)
- [Storage Backends](./storage-backends.md)
- [Security Model](./security.md)
