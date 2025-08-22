# Implement JSON Schema to Clap Argument Conversion

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Create utilities to convert JSON Schema definitions (used by MCP tools) into Clap argument definitions for dynamic CLI generation.

## Technical Details

### Core Conversion Function
Create `swissarmyhammer-cli/src/schema_conversion.rs`:

```rust
use clap::{Arg, ArgAction};
use serde_json::Value;

pub struct SchemaConverter;

impl SchemaConverter {
    pub fn schema_to_clap_args(schema: &Value) -> Vec<Arg> {
        // Convert JSON schema properties to clap arguments
    }
    
    fn json_property_to_clap_arg(name: &str, property_schema: &Value) -> Arg {
        // Convert individual property to clap argument
    }
    
    fn extract_required_fields(schema: &Value) -> Vec<String> {
        // Extract required field names from schema
    }
}
```

### Conversion Logic
Support these JSON Schema types → Clap argument types:
- `"type": "string"` → Text argument
- `"type": "boolean"` → `ArgAction::SetTrue`
- `"type": "integer"` → `value_parser!(i64)`
- `"type": "array"` → `ArgAction::Append`
- Required fields → `.required(true)`
- Description → `.help(description)`

### Schema Examples to Handle
From existing MCP tools:
```json
{
  "type": "object",
  "properties": {
    "title": {"type": "string", "description": "Title of the memo"},
    "content": {"type": "string", "description": "Markdown content"},
    "append": {"type": "boolean", "description": "Append to existing content"}
  },
  "required": ["title", "content"]
}
```

Should convert to:
```rust
vec![
    Arg::new("title").long("title").required(true).help("Title of the memo"),
    Arg::new("content").long("content").required(true).help("Markdown content"),
    Arg::new("append").long("append").action(ArgAction::SetTrue).help("Append to existing content"),
]
```

## Acceptance Criteria
- [ ] SchemaConverter struct with conversion methods
- [ ] Support for string, boolean, integer, array types
- [ ] Required field detection and enforcement
- [ ] Description mapping to help text
- [ ] Comprehensive unit tests for conversion logic
- [ ] Handle edge cases (missing properties, invalid schemas)
- [ ] Clear error messages for unsupported schema features

## Implementation Notes
- Focus on schemas used by existing MCP tools first
- Add validation for unsupported schema features
- Consider custom arg names for better CLI UX
- Plan for future schema extensions