---
severity: error
tags:
- operations
- parse
- error-handling
---

# Tree-sitter: Parse Error Reporting with Partial Results

## Acceptance Criterion
**AC-15**: Reports parse errors with line/column but continues with partial results

## What to Check
When parsing files with syntax errors, the tool must:
- Detect error nodes in syntax tree
- Report errors with line and column numbers
- Continue extracting symbols from valid parts of tree
- Return both errors and partial results (not fail completely)

## Success Criteria
- `errors` array in response contains detected parse errors
- Each error has line and column information
- Operation completes successfully even with parse errors
- Valid symbols still extracted and returned
- Warning included about incomplete results

## Reference
See specification/treesitter.md - Parse Error handling section