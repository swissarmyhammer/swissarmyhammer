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