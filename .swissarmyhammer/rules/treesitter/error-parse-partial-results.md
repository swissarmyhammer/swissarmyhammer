---
severity: error
tags:
- error-handling
- resilience
---

# Tree-sitter: Parse Error Partial Results

## Acceptance Criterion
**AC-24**: Parse errors return warning but continue with partial results

## What to Check
When file has syntax errors, tool must:
- Detect error nodes in parsed tree
- Return warning with code `TS_PARSE_ERRORS`
- Include error count and details in warning
- STILL return successfully extracted symbols (partial results)
- Not fail the operation completely

## Success Criteria
- Response includes warning section (not error)
- Warning code is `TS_PARSE_ERRORS`
- Both warnings AND results are returned
- Valid symbols extracted from non-error parts of tree

## Reference
See specification/treesitter.md - Parse Error handling section