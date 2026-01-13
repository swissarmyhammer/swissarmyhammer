---
severity: error
tags:
- error-handling
- validation
---

# Tree-sitter: Symbol Not Found Error

## Acceptance Criterion
**AC-26**: Symbol not found at position returns error with node type and suggestion

## What to Check
When position is not on an identifier, tool must:
- Return error with code `TS_SYMBOL_NOT_FOUND`
- Include file path, line, column in details
- Report actual node type at position (e.g., "string_literal", "whitespace")
- Provide helpful suggestion about position being on non-identifier

## Success Criteria
- Error response matches specification format
- Error code is `TS_SYMBOL_NOT_FOUND`
- Details include: file_path, line, column, node_type, suggestion
- Node type accurately reflects syntax tree node at position
- Works for whitespace, strings, comments, etc.

## Reference
See specification/treesitter.md - Symbol Not Found error section