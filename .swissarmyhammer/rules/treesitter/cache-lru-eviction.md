---
severity: error
tags:
- caching
- memory
---

# Tree-sitter: LRU Cache Eviction

## Acceptance Criterion
**AC-22**: LRU eviction when cache exceeds configurable max entries (default: 10,000)

## What to Check
Cache management must implement:
- LRU (Least Recently Used) eviction policy
- Configurable `max_cache_entries` limit (default: 10,000)
- Automatic eviction when cache size exceeds limit
- Evict least recently used entries first

## Success Criteria
- Cache size never exceeds max_cache_entries
- LRU policy correctly implemented
- Default limit is 10,000 entries
- Configurable via TreeSitterConfig
- Tests verify eviction behavior

## Reference
See specification/treesitter.md - cache implementation section