# Implement parameters for domain prompts

## Location
`swissarmyhammer-tools/src/mcp/server.rs:635`

## Current State
```rust
// Domain prompts don't have parameters yet - using empty list for now
```

## Description
Domain prompts currently don't support parameters and use an empty parameter list. This should be implemented to allow configurable domain prompts.

## Requirements
- Design parameter schema for domain prompts
- Implement parameter parsing and validation
- Update prompt loading to handle domain prompt parameters
- Add tests for parameterized domain prompts
- Update documentation

## Use Cases
- Configurable domain-specific behaviors
- Reusable domain prompts with variations
- Better prompt composition