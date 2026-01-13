---
severity: error
tags:
- treesitter
- core
- operations
---

# Tree-sitter Tool Core Operations

The tree-sitter MCP tool must support exactly 4 operations:

1. **definition** - Find all name-matching definitions in specified scope
2. **references** - Find all name-matching references in specified scope
3. **hover** - Extract doc comments and signature for symbol at position
4. **parse** - Return syntax tree structure with all top-level symbols

**Validation:**
- Tool accepts `operation` parameter with enum values: ["definition", "references", "hover", "parse"]
- Each operation is properly dispatched to its handler
- Invalid operation values return appropriate error

**Reference:** specification/treesitter.md - Core Functionality section