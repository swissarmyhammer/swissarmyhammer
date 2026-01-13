---
severity: error
tags:
- caching
- performance
---

# Tree-sitter: MD5-Based Cache Validation

## Acceptance Criterion
**AC-20**: File parse results cached in-memory with MD5 content hash validation

## What to Check
Caching implementation must:
- Cache parsed trees and extracted symbols in memory
- Use MD5 hash of file content as cache key/validation
- Check content hash to determine if cache is valid
- Re-parse file if content has changed (hash mismatch)

## Success Criteria
- CachedFile struct contains MD5 content hash
- Cache lookup computes MD5 and compares with cached hash
- Cache hit avoids re-parsing when hash matches
- Cache miss triggers re-parse when hash differs

## Reference
See specification/treesitter.md - Caching strategy section