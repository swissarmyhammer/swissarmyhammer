# Add Comprehensive Validation and Error Handling

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Implement robust validation and error handling for the dynamic CLI system, ensuring schema conversion edge cases are handled gracefully and users receive helpful error messages.

## Proposed Solution

After analyzing the existing codebase, I will implement validation and error handling through the following approach:

### 1. Schema Validation Framework (`schema_validation.rs`)
- Create a comprehensive `ValidationError` enum with detailed error types
- Implement `SchemaValidator` with static methods for validating schema structures
- Add validation for supported types, required fields, and schema completeness
- Include helpful error messages with suggestions for fixing issues

### 2. Enhanced Schema Converter
- Integrate validation into the existing `SchemaConverter` in `schema_conversion.rs`
- Add detailed error context and user-friendly error messages  
- Provide suggestions for common schema issues
- Maintain backward compatibility with existing conversion logic

### 3. Tool Registry Validation
- Add validation methods to `ToolRegistry` for checking all CLI tools at startup
- Implement graceful degradation for problematic tools (warn but continue)
- Add optional strict validation mode for development/testing

### 4. CLI Builder Integration
- Update `CliBuilder` to validate tools during CLI construction
- Implement fallback behavior that skips invalid tools with warnings
- Add startup validation option that can be enabled/disabled

### 5. Dynamic CLI Error Handling
- Enhanced error handling in the dynamic CLI execution path
- Better error messages for tool resolution failures
- Improved argument conversion error reporting

### Key Design Decisions:
1. **Non-breaking**: All changes maintain backward compatibility
2. **Graceful degradation**: Invalid tools are skipped with warnings, not crashes
3. **User-focused**: Error messages provide actionable guidance
4. **Developer-friendly**: Detailed validation for development scenarios
5. **Performance-aware**: Validation can be cached and optimized

## Technical Details

### Schema Validation Framework
Create `swissarmyhammer-cli/src/schema_validation.rs`:

```rust
use serde_json::Value;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Unsupported schema type: {schema_type} for parameter {parameter}")]
    UnsupportedSchemaType { schema_type: String, parameter: String },
    
    #[error("Invalid schema structure: {message}")]
    InvalidSchema { message: String },
    
    #[error("Missing required schema field: {field}")]
    MissingSchemaField { field: String },
    
    #[error("Schema conversion failed: {details}")]
    ConversionFailed { details: String },
}

pub struct SchemaValidator;

impl SchemaValidator {
    pub fn validate_schema(schema: &Value) -> Result<(), ValidationError> {
        // Validate that schema is convertible to clap arguments
    }
    
    pub fn validate_properties(properties: &serde_json::Map<String, Value>) -> Result<(), ValidationError> {
        // Validate individual property schemas
    }
    
    pub fn check_supported_types(property_schema: &Value) -> Result<(), ValidationError> {
        // Verify all schema types are supported by converter
    }
}
```

### Enhanced Error Messages
Improve error messaging throughout the system:

```rust
impl SchemaConverter {
    pub fn schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>, ValidationError> {
        SchemaValidator::validate_schema(schema)?;
        
        // Convert with detailed error context
        // ...
    }
    
    fn provide_conversion_suggestions(schema_type: &str) -> String {
        match schema_type {
            "object" => "Nested objects are not supported. Consider flattening the schema.".to_string(),
            "null" => "Null type parameters are not supported in CLI.".to_string(),
            unknown => format!("Unknown type '{}'. Supported types: string, boolean, integer, array.", unknown),
        }
    }
}
```

### Tool Registry Validation
Add validation to tool registry operations:

```rust
impl ToolRegistry {
    pub fn validate_cli_tools(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        
        for tool in self.get_cli_tools() {
            if let Err(e) = SchemaValidator::validate_schema(&tool.schema()) {
                errors.push(e);
            }
            
            if tool.cli_category().is_none() && !tool.hidden_from_cli() {
                errors.push(ValidationError::MissingSchemaField { 
                    field: format!("CLI category for tool {}", tool.name()) 
                });
            }
        }
        
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### CLI Error Handling Integration
Integrate validation with CLI error handling:

```rust
// In main.rs or cli.rs
pub fn handle_dynamic_cli_errors(error: Box<dyn std::error::Error>) -> ! {
    match error.downcast::<ValidationError>() {
        Ok(validation_error) => {
            eprintln!("❌ Schema validation failed:");
            eprintln!("   {}", validation_error);
            eprintln!("\n💡 This is likely a bug in the tool definition.");
            eprintln!("   Please report this issue with the tool name and command attempted.");
            std::process::exit(EXIT_ERROR);
        }
        Err(other_error) => {
            eprintln!("❌ Command failed: {}", other_error);
            std::process::exit(EXIT_ERROR);
        }
    }
}
```

### Graceful Degradation
Implement fallback behavior for problematic tools:

```rust
impl CliBuilder {
    fn build_tool_command(&self, tool: &dyn McpTool) -> Option<Command> {
        match SchemaValidator::validate_schema(&tool.schema()) {
            Ok(()) => {
                // Normal conversion
                Some(self.convert_schema_to_command(tool))
            }
            Err(e) => {
                tracing::warn!("Skipping tool {} from CLI due to schema validation error: {}", tool.name(), e);
                None // Skip problematic tools rather than crash
            }
        }
    }
}
```

### Startup Validation
Add CLI startup validation:

```rust
impl CliBuilder {
    pub fn build_cli(&self) -> Result<Command, Vec<ValidationError>> {
        // Validate all tools before building CLI
        self.tool_registry.validate_cli_tools()?;
        
        // Build CLI with validated tools
        Ok(self.build_cli_internal())
    }
    
    pub fn build_cli_with_warnings(&self) -> Command {
        // Build CLI but only warn about validation errors
        match self.tool_registry.validate_cli_tools() {
            Ok(()) => {},
            Err(errors) => {
                for error in errors {
                    tracing::warn!("Tool validation warning: {}", error);
                }
            }
        }
        
        self.build_cli_internal()
    }
}
```

### Testing Edge Cases
Add comprehensive tests for edge cases:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unsupported_schema_types() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",  // Not supported
                    "properties": {}
                }
            }
        });
        
        assert!(SchemaValidator::validate_schema(&schema).is_err());
    }
    
    #[test] 
    fn test_malformed_schema() {
        let schema = serde_json::json!({
            "properties": {
                "param": "invalid_schema"  // Should be object
            }
        });
        
        assert!(SchemaValidator::validate_schema(&schema).is_err());
    }
}
```

## Acceptance Criteria
- [ ] Comprehensive schema validation framework
- [ ] Clear, helpful error messages for validation failures
- [ ] Graceful degradation for problematic tools
- [ ] Startup validation with appropriate warnings
- [ ] Edge case handling for malformed schemas
- [ ] Integration with existing CLI error handling
- [ ] Comprehensive test coverage for error scenarios
- [ ] Tool registry validation methods
- [ ] User-friendly error reporting with suggestions

## Implementation Notes
- Focus on user experience - errors should be actionable
- Don't crash CLI for individual tool validation failures
- Provide clear guidance for resolving schema issues
- Test with intentionally malformed schemas
- Consider adding a --validate-tools CLI flag for debugging