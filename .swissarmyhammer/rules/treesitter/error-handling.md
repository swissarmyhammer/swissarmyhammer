---
severity: error
tags:
- treesitter
- error-handling
- robustness
---

# Tree-sitter Error Handling

The tree-sitter implementation must handle errors gracefully:

- Unsupported language errors must be returned with clear error messages
- Parse timeout handling must prevent hanging operations
- Symbol not found errors must return empty results, not crash
- Parse errors must be detected and reported but allow partial results
- All operations must degrade gracefully when encountering errors

Reference: specification/treesitter.md Phase 5.1