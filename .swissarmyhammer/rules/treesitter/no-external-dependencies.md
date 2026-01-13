---
severity: error
tags:
- architecture
- dependencies
---

# Tree-sitter: No External Dependencies

## Acceptance Criterion
**AC-4**: No external dependencies required (works offline, no subprocesses)

## What to Check
The tree-sitter tool must:
- NOT require external language server processes
- NOT make network requests
- NOT spawn subprocesses for parsing
- Work completely offline
- Have all parsers compiled into the binary

## Success Criteria
- No code spawns external processes for parsing
- No network calls in parsing logic
- All parsers linked statically at compile time
- Tool works without internet connection
- Tool works without external language server binaries

## Reference
See specification/treesitter.md - key benefit is "Zero external dependencies"