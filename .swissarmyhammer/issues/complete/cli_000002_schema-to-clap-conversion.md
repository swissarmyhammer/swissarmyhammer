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

## Proposed Solution

I will implement the JSON Schema to Clap conversion functionality as outlined in the issue requirements. My approach will be:

### Implementation Steps

1. **Create `swissarmyhammer-cli/src/schema_conversion.rs` module** with:
   - `SchemaConverter` struct containing the conversion logic
   - `schema_to_clap_args()` - main conversion function taking JSON schema and returning Vec<Arg>
   - `json_property_to_clap_arg()` - converts individual schema properties to clap arguments
   - `extract_required_fields()` - extracts required field names from schema

2. **Core conversion mappings**:
   - `"type": "string"` → Text argument with `value_parser(clap::value_parser!(String))`
   - `"type": "boolean"` → `ArgAction::SetTrue` flag argument
   - `"type": "integer"` → `value_parser(clap::value_parser!(i64))`
   - `"type": "array"` → `ArgAction::Append` for collecting multiple values
   - Required fields → `.required(true)`
   - Schema description → `.help(description)`

3. **Error handling and validation**:
   - Handle missing or invalid schema properties gracefully
   - Provide clear error messages for unsupported schema features
   - Validate argument names and ensure they're valid CLI argument names

4. **Testing strategy**:
   - Unit tests for each conversion function
   - Test with real MCP tool schemas from the codebase
   - Edge case testing (empty schemas, missing properties, invalid types)
   - Integration tests to ensure generated clap args work correctly

### Schema Examples to Handle

Based on existing MCP tools in the codebase:

**Simple memo creation schema**:
```json
{
  "type": "object", 
  "properties": {
    "title": {"type": "string", "description": "Title of the memo"},
    "content": {"type": "string", "description": "Markdown content"}
  },
  "required": ["title", "content"]
}
```

**File edit schema with optional boolean**:
```json
{
  "type": "object",
  "properties": {
    "file_path": {"type": "string", "description": "Absolute path to the file"},
    "old_string": {"type": "string", "description": "Exact text to replace"}, 
    "new_string": {"type": "string", "description": "Replacement text"},
    "replace_all": {"type": "boolean", "description": "Replace all occurrences", "default": false}
  },
  "required": ["file_path", "old_string", "new_string"]
}
```

This approach ensures compatibility with existing MCP tool schemas while providing a solid foundation for dynamic CLI generation.
## Implementation Completed ✅

The JSON Schema to Clap argument conversion functionality has been successfully implemented with the following features:

### Core Implementation

**File**: `swissarmyhammer-cli/src/schema_conversion.rs`

- `SchemaConverter` struct with static conversion methods
- `schema_to_clap_args()` - Converts complete JSON schema to `Vec<Arg>`
- `json_property_to_clap_arg()` - Converts individual properties to clap arguments
- `extract_required_fields()` - Extracts required field names from schema

### Supported Type Mappings

✅ **String properties** → Text arguments with `value_parser!(String)`
✅ **Boolean properties** → `ArgAction::SetTrue` flag arguments  
✅ **Integer properties** → Integer arguments with `value_parser!(i64)`
✅ **Array properties** → `ArgAction::Append` for multiple values
✅ **Required field detection** → `.required(true)` enforcement
✅ **Schema descriptions** → `.help(description)` mapping
✅ **Snake_case to kebab-case** → Automatic conversion (`file_path` → `--file-path`)

### Error Handling

Comprehensive error types via `SchemaConversionError`:
- Invalid schema structure validation
- Missing or invalid property types
- Unsupported schema features
- Clear error messages for debugging

### Technical Implementation Details

**Memory Management**: Uses `Box::leak()` to provide 'static lifetime strings required by clap's API. This is necessary because clap requires string references to live for the entire program duration for dynamic CLI generation.

**Testing**: 13 comprehensive unit tests covering:
- All supported property types
- Required field detection
- Error conditions and edge cases
- Real MCP tool schema examples (memo creation, file editing)
- Snake case to kebab case conversion

### Integration

- Added to `swissarmyhammer-cli/src/lib.rs` as public module
- Added `thiserror` dependency for proper error handling
- All tests pass ✅
- Code formatted and linted ✅
- Builds successfully ✅

### Usage Example

```rust
use swissarmyhammer_cli::schema_conversion::SchemaConverter;
use serde_json::json;

let schema = json!({
    "type": "object",
    "properties": {
        "title": {"type": "string", "description": "Title of the item"},
        "force": {"type": "boolean", "description": "Force the operation"}
    },
    "required": ["title"]
});

let args = SchemaConverter::schema_to_clap_args(&schema)?;
// Creates clap arguments: --title (required) and --force (flag)
```

The implementation is ready for integration with dynamic CLI generation systems that need to convert MCP tool schemas into command-line interfaces.