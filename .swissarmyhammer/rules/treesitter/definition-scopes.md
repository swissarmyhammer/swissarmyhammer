---
severity: error
tags:
- operations
- definition
- scopes
---

# Tree-sitter: Definition Search Scopes

## Acceptance Criterion
**AC-6**: Supports 3 scopes: `file` (current file), `directory` (same dir), `project` (CWD recursively)

## What to Check
The definition operation must support exactly 3 scope values:
- `file`: Search only in the specified file
- `directory`: Search in same directory as the file
- `project`: Search from CWD recursively (default)

## Success Criteria
- Schema defines scope enum with exactly these 3 values
- `file` scope searches only current file
- `directory` scope globs same directory for parseable files
- `project` scope globs from CWD recursively
- Default scope is `project`

## Reference
See specification/treesitter.md - MCP Tool Design section