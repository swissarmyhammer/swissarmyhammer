# MCP Tools Architecture Decision Record

## Status
**Adopted** - Implemented in issue tools migration (REFACTOR_000205)

## Context

The original MCP tool implementation used a large match statement in `tool_registry.rs` to handle tool dispatch. This approach had several limitations:

- **Monolithic Structure**: All tool implementations were in a single file
- **Poor Separation of Concerns**: Tool logic mixed with registry logic
- **Difficult Maintenance**: Adding new tools required modifying the central match statement
- **Inconsistent Documentation**: Tool descriptions were scattered and inconsistent
- **Limited Reusability**: No clear pattern for tool implementation

## Decision

We adopt a **modular tool registry pattern** with the following characteristics:

### 1. Modular Tool Structure
```
src/mcp/tools/
├── mod.rs                    # Main module with architecture docs
├── issues/                   # Issue management tools
│   ├── mod.rs               # Category registration
│   ├── create/
│   │   ├── mod.rs           # Tool implementation
│   │   └── description.md   # Tool documentation
│   └── ...                  # Other issue tools
└── memoranda/               # Memo management tools
    ├── mod.rs               # Category registration
    ├── create/
    │   ├── mod.rs           # Tool implementation
    │   └── description.md   # Tool documentation
    └── ...                  # Other memo tools
```

### 2. Standard Tool Implementation Pattern
```rust
#[derive(Default)]
pub struct ExampleTool;

impl ExampleTool {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl McpTool for ExampleTool {
    fn name(&self) -> &'static str { "tool_name" }
    fn description(&self) -> &'static str { include_str!("description.md") }
    fn schema(&self) -> serde_json::Value { /* JSON schema */ }
    async fn execute(&self, args: Map<String, Value>, ctx: &ToolContext) -> Result<CallToolResult, McpError> {
        let request: RequestType = BaseToolImpl::parse_arguments(args)?;
        ctx.tool_handlers.handle_operation(request).await
    }
}
```

### 3. Category-Based Registration
```rust
pub fn register_category_tools(registry: &mut ToolRegistry) {
    registry.register(tool1::Tool1::new());
    registry.register(tool2::Tool2::new());
    // ...
}
```

### 4. Co-Located Documentation
Each tool includes a `description.md` file providing:
- Tool purpose and functionality
- Parameter specifications with examples
- Return value descriptions
- Usage examples in JSON format

## Benefits

### Modularity
- Each tool is self-contained in its own module
- Clear separation between tool categories
- Easy to add, modify, or remove individual tools

### Consistency
- All tools follow the same implementation pattern
- Standardized documentation format
- Uniform error handling and type safety

### Maintainability
- Tool implementations are isolated from registry logic
- Documentation co-located with implementation
- Clear dependency relationships

### Extensibility
- New tool categories can be added without modifying existing code
- Tools can be easily moved between categories
- Plugin-like architecture supports external tool development

### Developer Experience
- Clear patterns for implementing new tools
- Comprehensive documentation for each tool
- Type-safe parameter handling through schema validation

## Implementation

### Phase 1: Issue Tools Migration
- Migrated all 8 issue tools to new pattern
- Implemented `BaseToolImpl` utilities for common operations
- Created comprehensive `description.md` files for each tool
- Updated tool registry to use modular registration

### Phase 2: Memo Tools Migration  
- Migrated all 7 memo tools following same pattern
- Fixed import paths to use correct type modules
- Maintained backward compatibility throughout migration
- Achieved architectural consistency across all tool categories

## Consequences

### Positive
- **Reduced Complexity**: Central registry is now just a dispatcher
- **Improved Organization**: Tools are logically grouped by functionality
- **Better Documentation**: Each tool has comprehensive, up-to-date docs
- **Easier Testing**: Individual tools can be tested in isolation
- **Clear Patterns**: New contributors can easily follow established patterns

### Neutral
- **More Files**: Each tool now has its own directory structure
- **Learning Curve**: Developers need to understand the new pattern

### Negative
- **Migration Effort**: Required significant refactoring of existing tools
- **Directory Complexity**: More nested directory structure

## Alternatives Considered

### 1. Macro-Based Tool Registration
Using macros to reduce boilerplate in tool definitions.
**Rejected**: Added complexity without significant benefits, harder to debug.

### 2. Trait Object Registry
Using dynamic dispatch with trait objects instead of generics.
**Rejected**: Runtime overhead and loss of type safety.

### 3. Plugin System with Dynamic Loading
Loading tools at runtime from external libraries.
**Rejected**: Unnecessary complexity for current requirements, security concerns.

## Future Considerations

- **Tool Versioning**: Consider adding version support for tool schema evolution
- **Tool Categories**: May need more granular categorization as tools grow
- **Cross-Tool Dependencies**: Need patterns for tools that depend on other tools
- **Performance**: Monitor registry performance as tool count increases

## References

- Issue #REFACTOR_000205: Migrate issue tools to new MCP structure
- MCP Protocol Specification: Tool registration and execution patterns
- Rust API Guidelines: Module organization and naming conventions