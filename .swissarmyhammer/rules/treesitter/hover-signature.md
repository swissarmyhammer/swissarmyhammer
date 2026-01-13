---
severity: error
tags:
- operations
- hover
---

# Tree-sitter: Hover Symbol Signature

## Acceptance Criterion
**AC-12**: Returns symbol signature extracted syntactically from definition node

## What to Check
The hover operation must:
- Extract function/class signature from definition syntax node
- Return syntactic signature (not type-inferred)
- Include function name, parameters, return type (if present in source)

## Success Criteria
- Signature extracted directly from syntax tree
- Signature matches source code format
- No type inference or semantic analysis
- Works even if code has type errors

## Reference
See specification/treesitter.md - hover operation and response format