# Implement proper node name extraction in outline parser

## Location
`swissarmyhammer-outline/src/parser.rs:278`

## Current State
```rust
// For now, just return the first line of the node
```

## Description
The outline parser currently just returns the first line of a node as its name. This is a simplistic approach that should be replaced with proper name extraction logic.

## Requirements
- Extract accurate symbol names from tree-sitter nodes
- Handle various node types (functions, classes, methods, etc.)
- Support identifier extraction across all supported languages
- Handle edge cases (anonymous functions, complex declarations)
- Add tests for various symbol naming patterns

## Impact
Inaccurate symbol names in generated outlines.