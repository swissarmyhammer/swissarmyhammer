---
severity: warning
tags:
- performance
- memory
---

# Tree-sitter: Memory Usage

## Acceptance Criterion
**AC-19**: Additional memory usage < 50MB for typical usage

## What to Check
Memory overhead of tree-sitter functionality must be:
- Less than 50MB for typical usage patterns
- Measured during normal operation with caching
- Does not include base binary size (only runtime overhead)

## Success Criteria
- Memory profiling shows < 50MB overhead
- Tested on typical projects with moderate file counts
- Cache size controlled by max_cache_entries limit

## Reference
See specification/treesitter.md - Success Criteria section