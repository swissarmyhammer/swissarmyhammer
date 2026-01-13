---
severity: error
tags:
- performance
- benchmarks
---

# Tree-sitter: Single File Performance

## Acceptance Criterion
**AC-17**: Single file definition lookup completes in < 50ms

## What to Check
Definition lookup on a single file (scope=file) must:
- Complete in under 50 milliseconds
- Include parsing, symbol extraction, and matching
- Meet performance target on typical source files (< 5000 lines)

## Success Criteria
- Benchmark tests confirm < 50ms for single file operations
- Performance measured on representative code samples
- Excludes initial parser loading (one-time cost)

## Reference
See specification/treesitter.md - Success Criteria section