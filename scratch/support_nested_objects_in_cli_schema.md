# Add support for nested objects in CLI schema

## Location
`swissarmyhammer-cli/src/schema_conversion.rs:252`

## Current State
```rust
"Nested objects are not supported. Consider flattening the schema or using a string representation.".to_string()
```

## Description
The CLI schema conversion currently does not support nested objects. Parameters with nested object types show a suggestion to flatten or use string representation.

## Requirements
- Design approach for representing nested objects in CLI arguments
- Consider JSON string input as one option
- Implement conversion logic
- Add validation for nested structures
- Update tests and documentation

## Impact
Limits the types of parameters that can be used in CLI workflows and prompts.