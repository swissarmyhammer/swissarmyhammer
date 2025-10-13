# Create proper ValidationError type in common module

## Location
`swissarmyhammer-tools/src/mcp/tool_registry.rs:1124`

## Current State
```rust
// For now, let's define a simple ValidationError type here
```

## Description
A simple ValidationError type is currently defined locally in the tool registry. This should be moved to a common module and properly designed to handle all validation error scenarios.

## Requirements
- Design comprehensive ValidationError type
- Move to swissarmyhammer-common for reuse
- Include error codes, messages, and context
- Support error composition and chaining
- Add conversion from various error types
- Update all validation code to use the new type

## Related
Part of broader error handling standardization effort.