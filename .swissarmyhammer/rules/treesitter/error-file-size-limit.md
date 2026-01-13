---
severity: error
tags:
- validation
- security
---

# Tree-sitter: File Size Limit

## Acceptance Criterion
**AC-27**: Large files (>10MB default) are rejected with configurable limit

## What to Check
File size validation must:
- Check file size before parsing
- Reject files exceeding max_file_size (default: 10MB)
- Return appropriate error for oversized files
- Make limit configurable via TreeSitterConfig

## Success Criteria
- File size checked before parse attempt
- Default max_file_size is 10MB (10485760 bytes)
- Oversized files rejected with error
- Limit configurable via config
- Error includes file size and limit in details

## Reference
See specification/treesitter.md - TreeSitterConfig and error handling sections