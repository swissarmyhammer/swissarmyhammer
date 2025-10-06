# Implement Mermaid diagram serialization for workflows

## Location
`swissarmyhammer-workflow/src/storage.rs:427`

## Current State
```rust
// For now, store as JSON since we don't have mermaid serialization
```

## Description
Workflows are currently stored as JSON even when Mermaid format might be more appropriate for visualization. Mermaid serialization should be implemented to support visual workflow representation.

## Requirements
- Implement Mermaid diagram generation from workflow definitions
- Support all workflow state types and transitions
- Generate readable and properly formatted Mermaid syntax
- Add deserialization if bidirectional conversion is needed
- Add tests with various workflow structures

## Use Cases
- Workflow visualization in documentation
- Debugging complex workflows
- User-friendly workflow representation