---
severity: error
tags:
- performance
- treesitter
---

## Speed Performance Requirements

Definition lookup must complete in:
- < 50ms for single file searches
- < 500ms for project-wide searches

This ensures the tree-sitter tool provides fast, responsive code navigation without the overhead of external language servers.

Reference: specification/treesitter.md - Success Criteria section