---
severity: error
tags:
- operations
- references
- response-format
---

# Tree-sitter: References Response Format

## Acceptance Criterion
**AC-10**: Returns file path, line, column, kind (call/reference), preview, and total count

## What to Check
Each reference in the response must include:
- `file_path`: Absolute path to file containing reference
- `line`: Line number (1-based)
- `column`: Column number (1-based)
- `kind`: Type of reference (call, reference, etc.)
- `preview`: Source code preview of the reference line

Response must also include:
- `total_count`: Total number of references found

## Success Criteria
- Response structure matches specification exactly
- All 5 fields present for each reference
- `total_count` field at response level
- Line/column numbers are 1-based

## Reference
See specification/treesitter.md - References Response format example