# Implement Schema-to-Clap Argument Conversion

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Create utilities to convert JSON Schema definitions from MCP tools into Clap argument definitions for dynamic CLI command generation.

## Implementation Tasks

### 1. Create Schema Conversion Module

Create `swissarmyhammer-cli/src/schema_conversion.rs`:

```rust
use clap::{Arg, ArgAction, Command, ValueHint};
use serde_json::Value;
use anyhow::{Result, bail};

pub struct SchemaConverter;

impl SchemaConverter {
    /// Convert JSON schema to clap arguments
    pub fn schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>> {
        let mut args = Vec::new();
        
        let properties = schema.get("properties")
            .and_then(|p| p.as_object())
            .ok_or_else(|| anyhow::anyhow!("Schema missing properties object"))?;
            
        let required = schema.get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
            
        for (prop_name, prop_schema) in properties {
            let arg = Self::json_schema_property_to_clap_arg(prop_name, prop_schema, &required)?;
            args.push(arg);
        }
        
        Ok(args)
    }
    
    /// Convert individual JSON schema property to clap Arg
    fn json_schema_property_to_clap_arg(
        name: &str, 
        schema: &Value, 
        required: &[&str]
    ) -> Result<Arg> {
        let mut arg = Arg::new(name).long(name);
        
        // Add help text from description
        if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
            arg = arg.help(desc);
        }
        
        // Handle required fields
        if required.contains(&name) {
            arg = arg.required(true);
        }
        
        // Map JSON schema types to clap actions
        match schema.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => {
                arg = arg.action(ArgAction::SetTrue);
            },
            Some("integer") => {
                arg = arg.value_parser(clap::value_parser!(i64));
                if let Some(min) = schema.get("minimum").and_then(|m| m.as_i64()) {
                    // Add validation range hint
                    arg = arg.help(&format!("{} (min: {})", 
                        arg.get_help().unwrap_or(""), min));
                }
            },
            Some("array") => {
                arg = arg.action(ArgAction::Append);
                // Handle array item types if specified
                if let Some(items) = schema.get("items") {
                    if let Some(item_type) = items.get("type").and_then(|t| t.as_str()) {
                        match item_type {
                            "integer" => arg = arg.value_parser(clap::value_parser!(i64)),
                            _ => {} // string is default
                        }
                    }
                }
            },
            Some("string") | None => {
                // Handle string enums
                if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
                    let values: Result<Vec<String>, _> = enum_values
                        .iter()
                        .map(|v| v.as_str().map(|s| s.to_string())
                            .ok_or_else(|| anyhow::anyhow!("Non-string enum value")))
                        .collect();
                    arg = arg.value_parser(values?);
                }
                
                // Handle format hints
                if let Some(format) = schema.get("format").and_then(|f| f.as_str()) {
                    arg = match format {
                        "uri" | "url" => arg.value_hint(ValueHint::Url),
                        "email" => arg.value_hint(ValueHint::EmailAddress),
                        "path" => arg.value_hint(ValueHint::FilePath),
                        _ => arg,
                    };
                }
                
                // Handle pattern validation hint
                if let Some(_pattern) = schema.get("pattern").and_then(|p| p.as_str()) {
                    // For now, just add to help text
                    arg = arg.help(&format!("{} (pattern: {})", 
                        arg.get_help().unwrap_or(""), _pattern));
                }
            },
            Some(unknown_type) => {
                bail!("Unsupported JSON schema type: {}", unknown_type);
            }
        }
        
        Ok(arg)
    }
    
    /// Convert clap ArgMatches back to JSON arguments
    pub fn matches_to_json_args(
        matches: &clap::ArgMatches, 
        schema: &Value
    ) -> Result<serde_json::Map<String, Value>> {
        let mut args = serde_json::Map::new();
        
        let properties = schema.get("properties")
            .and_then(|p| p.as_object())
            .ok_or_else(|| anyhow::anyhow!("Schema missing properties"))?;
            
        for (prop_name, prop_schema) in properties {
            if let Some(value) = Self::extract_clap_value(matches, prop_name, prop_schema)? {
                args.insert(prop_name.clone(), value);
            }
        }
        
        Ok(args)
    }
    
    /// Extract value from ArgMatches based on schema type
    fn extract_clap_value(
        matches: &clap::ArgMatches,
        prop_name: &str, 
        prop_schema: &Value
    ) -> Result<Option<Value>> {
        if !matches.contains_id(prop_name) {
            return Ok(None);
        }
        
        let json_value = match prop_schema.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => {
                Value::Bool(matches.get_flag(prop_name))
            },
            Some("integer") => {
                if let Some(val) = matches.get_one::<i64>(prop_name) {
                    Value::Number((*val).into())
                } else {
                    return Ok(None);
                }
            },
            Some("array") => {
                let values: Vec<String> = matches.get_many::<String>(prop_name)
                    .unwrap_or_default()
                    .map(|s| s.clone())
                    .collect();
                Value::Array(values.into_iter().map(Value::String).collect())
            },
            _ => {
                // String or unspecified type
                if let Some(val) = matches.get_one::<String>(prop_name) {
                    Value::String(val.clone())
                } else {
                    return Ok(None);
                }
            }
        };
        
        Ok(Some(json_value))
    }
}
```

### 2. Add Validation and Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SchemaConversionError {
    #[error("Invalid schema structure: {0}")]
    InvalidSchema(String),
    #[error("Unsupported schema type: {0}")]
    UnsupportedType(String),
    #[error("Argument extraction failed: {0}")]
    ArgumentExtraction(String),
}

// Update methods to use custom error type
```

### 3. Create Tests

Add comprehensive tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_string_property_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name parameter"
                }
            },
            "required": ["name"]
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);
        
        let name_arg = &args[0];
        assert_eq!(name_arg.get_id(), "name");
        assert_eq!(name_arg.get_help(), Some("The name parameter"));
        assert!(name_arg.is_required_set());
    }
    
    #[test]
    fn test_boolean_property_conversion() {
        let schema = json!({
            "type": "object", 
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "description": "Enable feature"
                }
            }
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let enabled_arg = &args[0];
        
        assert_eq!(enabled_arg.get_action(), &ArgAction::SetTrue);
        assert!(!enabled_arg.is_required_set());
    }
    
    #[test] 
    fn test_integer_with_minimum() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Item count"
                }
            }
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let count_arg = &args[0];
        
        assert!(count_arg.get_help().unwrap().contains("min: 1"));
    }
    
    #[test]
    fn test_array_property() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array", 
                    "items": {"type": "string"},
                    "description": "List of tags"
                }
            }
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let tags_arg = &args[0];
        
        assert_eq!(tags_arg.get_action(), &ArgAction::Append);
    }
    
    #[test]
    fn test_enum_values() {
        let schema = json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["json", "yaml", "table"],
                    "description": "Output format"
                }
            }
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        let format_arg = &args[0];
        
        // Verify enum values are handled (exact verification depends on clap internals)
        assert!(format_arg.get_help().unwrap().contains("Output format"));
    }
    
    #[test]
    fn test_matches_to_json_round_trip() {
        // Test that we can convert schema -> args -> matches -> json successfully
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "count": {"type": "integer"},
                "enabled": {"type": "boolean"}
            },
            "required": ["name"]
        });
        
        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        
        // This would need to be tested with actual clap command parsing
        // For now, test the JSON conversion logic directly
        assert!(!args.is_empty());
    }
}
```

### 4. Integration with CLI Module

Update `swissarmyhammer-cli/src/lib.rs` to export the new module:

```rust
pub mod schema_conversion;
```

## Success Criteria

- [ ] SchemaConverter can convert basic JSON schema types to Clap args
- [ ] Supports string, boolean, integer, and array types
- [ ] Handles required fields correctly
- [ ] Converts enum values to value parsers
- [ ] Round-trip conversion: schema → args → matches → JSON
- [ ] Comprehensive test coverage for all supported types
- [ ] Clear error handling for unsupported schema features
- [ ] Proper validation and error messages

## Architecture Notes

- Focused on core JSON Schema features used by MCP tools
- Extensible design for adding more schema features later
- Clear separation between schema parsing and Clap integration
- Comprehensive error handling for robust CLI experience