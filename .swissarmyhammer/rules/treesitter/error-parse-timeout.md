---
severity: error
tags:
- error-handling
- performance
---

# Tree-sitter: Parse Timeout Error

## Acceptance Criterion
**AC-25**: Parse timeout (default: 5000ms) returns error with timeout details

## What to Check
When parsing exceeds timeout, tool must:
- Enforce parse timeout (default: 5000ms, configurable)
- Return error with code `TS_PARSE_TIMEOUT`
- Include file path, file size, timeout value in details
- Provide helpful suggestion about file being too large

## Success Criteria
- Parse operations timeout after configured duration
- Error response matches specification format
- Error code is `TS_PARSE_TIMEOUT`
- Details include: file_path, file_size_bytes, timeout_ms, suggestion
- Default timeout is 5000ms
- Timeout configurable via TreeSitterConfig

## Reference
See specification/treesitter.md - Parse Timeout error section