---
severity: error
tags:
- api
- operations
---

# Tree-sitter: Operations Supported

## Acceptance Criterion
**AC-1**: Tool accepts 4 operations: `definition`, `references`, `hover`, `parse`

## What to Check
The tree-sitter MCP tool must accept exactly these four operations via the `operation` parameter:
- `definition` - Find definitions of symbol at position
- `references` - Find references to symbol at position  
- `hover` - Get documentation for symbol at position
- `parse` - Parse file and return syntax tree

## Success Criteria
- Tool schema defines `operation` as enum with these 4 values
- Tool dispatches each operation to appropriate handler
- Invalid operation values are rejected with error

## Reference
See specification/treesitter.md for operation details