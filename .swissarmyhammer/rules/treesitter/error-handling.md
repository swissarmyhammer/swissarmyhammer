---
severity: error
tags:
- treesitter
- error-handling
---

# Tree-sitter Error Handling

The tree-sitter implementation must handle errors gracefully:

- Unsupported language errors must be returned with clear error messages
- Parse timeout handling must prevent hanging operations
- Symbol not found errors must return empty results rather than errors
- Parse errors must not prevent partial results from being returned
- All error cases must be documented and tested

Reference: specification/complete/treesitter.md Phase 5.1