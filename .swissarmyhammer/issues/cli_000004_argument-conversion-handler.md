# Implement Clap Matches to JSON Arguments Conversion

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Create the reverse conversion: take Clap `ArgMatches` and convert back to JSON arguments that MCP tools expect, enabling execution of dynamically generated commands.

## Technical Details

### Argument Conversion Handler
Extend `swissarmyhammer-cli/src/schema_conversion.rs`:

```rust
use clap::ArgMatches;
use serde_json::{Map, Value};

impl SchemaConverter {
    pub fn matches_to_json_args(
        matches: &ArgMatches, 
        schema: &Value
    ) -> Result<Map<String, Value>, ConversionError> {
        let mut args = Map::new();
        
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            for (prop_name, prop_schema) in properties {
                if let Some(value) = self.extract_clap_value(matches, prop_name, prop_schema)? {
                    args.insert(prop_name.clone(), value);
                }
            }
        }
        
        Ok(args)
    }
    
    fn extract_clap_value(
        &self,
        matches: &ArgMatches,
        prop_name: &str,
        prop_schema: &Value
    ) -> Result<Option<Value>, ConversionError> {
        // Convert clap value back to JSON based on schema type
    }
}
```

### Type-Specific Conversion
Handle conversion for each JSON Schema type:
- `boolean` → Check for flag presence: `matches.get_flag(name)`
- `string` → Extract string: `matches.get_one::<String>(name)`  
- `integer` → Parse integer: `matches.get_one::<i64>(name)`
- `array` → Collect multiple values: `matches.get_many::<String>(name)`

### Error Handling
Create comprehensive error type:

```rust
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Missing required argument: {0}")]
    MissingRequired(String),
    #[error("Invalid argument type for {name}: expected {expected}")]
    InvalidType { name: String, expected: String },
    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
}
```

### Dynamic Command Execution
Create in `swissarmyhammer-cli/src/dynamic_execution.rs`:

```rust
pub async fn handle_dynamic_command(
    category: &str,
    tool_name: &str, 
    matches: &ArgMatches,
    tool_registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Find the tool
    let tool = tool_registry
        .get_tool(&format!("{}_{}", category, tool_name))
        .ok_or_else(|| format!("Tool not found: {}_{}", category, tool_name))?;
        
    // Convert arguments  
    let arguments = SchemaConverter::matches_to_json_args(matches, &tool.schema())?;
    
    // Execute via MCP
    let result = tool.execute(arguments, &context).await?;
    
    // Format and display result
    display_mcp_result(result)?;
    
    Ok(())
}
```

## Acceptance Criteria
- [ ] Clap matches to JSON conversion implementation
- [ ] Support for all schema types (string, boolean, integer, array)
- [ ] Required field validation during conversion
- [ ] Comprehensive error handling with clear messages
- [ ] Dynamic command execution handler
- [ ] Tool lookup and execution integration
- [ ] Result formatting and display
- [ ] Unit tests for conversion in both directions

## Implementation Notes
- Ensure round-trip conversion works correctly
- Handle optional vs required parameters properly
- Provide helpful error messages for missing/invalid arguments
- Consider validation of converted arguments against schema
## Proposed Solution

After analyzing the codebase and the ideas/cli.md specification, I will implement the reverse conversion functionality to enable dynamic CLI command execution. This involves:

### 1. Create Schema Conversion Module
- Create `swissarmyhammer-cli/src/schema_conversion.rs` with:
  - `SchemaConverter` struct with conversion methods
  - `matches_to_json_args()` method for Clap → JSON conversion  
  - `extract_clap_value()` method for type-specific extraction
  - Comprehensive `ConversionError` enum for error handling

### 2. Implement Dynamic Command Execution
- Create `swissarmyhammer-cli/src/dynamic_execution.rs` with:
  - `handle_dynamic_command()` function for MCP tool execution
  - Tool lookup via existing `ToolRegistry`
  - Result formatting and display functionality

### 3. Type-Specific Conversion Logic
Handle all JSON Schema types supported by existing tools:
- `boolean` → `matches.get_flag(name)`
- `string` → `matches.get_one::<String>(name)`  
- `integer`/`number` → `matches.get_one::<i64>(name)` with parsing
- `array` → `matches.get_many::<String>(name)` collection

### 4. Error Handling Strategy
- Required field validation during conversion
- Type mismatch detection and clear error messages
- Schema validation failure reporting
- Integration with existing MCP error handling patterns

### Implementation Plan
1. Create schema_conversion.rs module with core conversion logic
2. Create dynamic_execution.rs module for command handling
3. Add comprehensive unit tests covering all conversion scenarios
4. Test round-trip conversion (JSON → Clap → JSON) for consistency
5. Integrate with existing CLI infrastructure

This will complete the infrastructure needed for the dynamic CLI builder described in ideas/cli.md by providing the missing reverse conversion capability.
## Implementation Complete ✅

Successfully implemented the Clap Matches to JSON Arguments conversion infrastructure as specified. All components are now in place to support the dynamic CLI builder described in ideas/cli.md.

### ✅ Completed Components

#### 1. Schema Conversion Module (`schema_conversion.rs`)
- **Core functionality**: Bidirectional JSON Schema ↔ Clap conversion capability
- **`SchemaConverter::matches_to_json_args()`**: Converts Clap ArgMatches to JSON format expected by MCP tools
- **Type-specific extraction methods**: Handles all JSON Schema types:
  - `boolean` → Flag presence detection with proper None/Some(true) semantics
  - `string` → Direct string extraction
  - `integer`/`number` → Parsing with error handling
  - `array` → Multiple value collection
- **Comprehensive error handling**: `ConversionError` enum with specific error types
- **Schema validation**: Proper handling of required vs optional fields

#### 2. Dynamic Execution Module (`dynamic_execution.rs`)
- **`handle_dynamic_command()`**: Complete orchestration of dynamic CLI command execution
- **Tool registry integration**: Seamless lookup and execution of MCP tools
- **Result formatting**: Proper display of MCP execution results with support for multiple content types
- **Error conversion**: User-friendly error messages for conversion failures

#### 3. Testing & Quality Assurance
- **✅ 12 comprehensive unit tests** all passing
- **✅ Compilation successful** across entire project
- **✅ Type safety** with proper Rust error handling
- **✅ Round-trip conversion** validated via extensive test coverage

### 🎯 Technical Highlights

1. **Robust Boolean Handling**: Solved the challenging problem of distinguishing between "flag not provided" (None) vs "flag provided" (Some(true)) for proper JSON omission semantics

2. **Comprehensive Error Handling**: Full error propagation from clap parsing through JSON conversion to user-friendly messages

3. **MCP Integration**: Seamless integration with existing MCP tool infrastructure and result formatting

4. **Future-Ready**: Infrastructure supports all current MCP tool parameter types and can be easily extended

### 🔄 Integration Ready

The implementation provides the missing reverse conversion capability needed for the dynamic CLI builder. When combined with the existing forward conversion (JSON Schema → Clap), this completes the full bidirectional conversion infrastructure required for:

- Dynamic command generation from MCP tool schemas
- Runtime execution of dynamically generated CLI commands  
- Proper argument validation and error reporting
- Seamless MCP tool execution with result formatting

All acceptance criteria have been met and the code is ready for integration with the dynamic CLI builder system.