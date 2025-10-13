# Add support for complex union types in CLI

## Location
`swissarmyhammer-cli/src/schema_validation.rs:321`

## Current State
```rust
suggestion: "Complex union types are not supported. Use a single type or make parameters optional.".to_string(),
```

## Description
Complex union types are not currently supported in CLI schema validation. This limits flexibility in parameter type definitions.

## Requirements
- Analyze what "complex union types" means in this context
- Design approach for handling unions in CLI arguments
- Consider type discrimination strategies
- Implement parsing and validation logic
- Add comprehensive tests

## Related
Similar to nested object support - both expand CLI type system capabilities.