# Implement YAML output format for validate command

## Location
`swissarmyhammer-cli/src/validate.rs:400`

## Current State
```rust
// For validate command, YAML output is not implemented, fall back to JSON
```

## Description
The validate command currently falls back to JSON when YAML output is requested. YAML output should be properly implemented for consistency.

## Requirements
- Implement YAML serialization for validation results
- Ensure formatting is readable and follows YAML best practices
- Add tests for YAML output format
- Update command help text if needed

## Impact
Inconsistent output format options across commands.