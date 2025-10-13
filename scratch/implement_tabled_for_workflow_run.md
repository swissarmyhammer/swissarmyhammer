# Implement Tabled trait for WorkflowRun

## Location
`swissarmyhammer-cli/src/commands/flow/shared.rs:86`

## Current State
```rust
// For now, use simple serialization since WorkflowRun doesn't implement Tabled
```

## Description
WorkflowRun currently uses simple serialization for display because it doesn't implement the Tabled trait. Implementing Tabled would provide better formatted table output.

## Requirements
- Implement Tabled trait for WorkflowRun struct
- Define appropriate column layout for workflow information
- Handle nested data structures appropriately
- Add tests for table formatting
- Ensure consistent with other table output in CLI

## Impact
Inconsistent and less readable workflow output formatting.