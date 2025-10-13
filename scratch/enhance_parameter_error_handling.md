# Enhance parameter validation error handling

## Location
`swissarmyhammer-common/src/parameters.rs:616`

## Current State
```rust
// Add other cases as needed for completeness, for now just return a generic error
```

## Description
Parameter validation currently returns generic errors for unhandled cases. Error handling should be enhanced to provide specific, actionable error messages for all validation scenarios.

## Requirements
- Identify all parameter validation cases
- Create specific error types/variants for each case
- Provide detailed error messages with context
- Include suggestions for fixing validation errors
- Add error recovery hints
- Improve error message formatting
- Add tests for all error cases

## Impact
Generic errors make it hard for users to understand and fix parameter issues.