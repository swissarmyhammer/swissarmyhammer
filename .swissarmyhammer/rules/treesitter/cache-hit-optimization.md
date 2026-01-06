---
severity: error
tags:
- caching
- performance
---

# Tree-sitter: Cache Hit Optimization

## Acceptance Criterion
**AC-21**: Cache hit avoids re-parsing (hash check before file read)

## What to Check
Cache optimization must:
- Check MD5 hash BEFORE reading file content when possible
- Avoid re-parsing if cached hash matches current hash
- Return cached symbols without tree traversal
- Significantly reduce latency on cache hit

## Success Criteria
- Cache hit path does not re-parse file
- Hash validation performed efficiently
- Cached symbols returned directly
- Performance tests show cache hit speedup

## Reference
See specification/treesitter.md - Caching strategy and performance optimization