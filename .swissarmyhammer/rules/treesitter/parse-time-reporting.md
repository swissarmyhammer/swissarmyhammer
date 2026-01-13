---
severity: error
tags:
- operations
- parse
- performance
---

# Tree-sitter: Parse Time Reporting

## Acceptance Criterion
**AC-16**: Returns parse time in milliseconds

## What to Check
The parse operation response must include:
- `parse_time_ms` field
- Actual measured parse time in milliseconds
- Time measured from start to end of parsing operation

## Success Criteria
- Response includes `parse_time_ms` field with numeric value
- Time is measured accurately
- Time reflects actual parse duration

## Reference
See specification/treesitter.md - Parse Response format example