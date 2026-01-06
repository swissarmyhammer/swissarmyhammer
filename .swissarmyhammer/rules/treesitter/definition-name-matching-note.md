---
severity: error
tags:
- operations
- definition
- response-format
---

# Tree-sitter: Definition Name-Matching Note

## Acceptance Criterion
**AC-8**: Includes note about name-based matching (multiple definitions possible)

## What to Check
The definition response must include:
- `resolution` field with value "name_match"
- `note` field explaining that tree-sitter uses name matching, not semantic resolution
- Clear indication that multiple definitions are possible and expected

## Success Criteria
- Response includes `resolution: "name_match"` field
- Response includes note field with explanation
- Note mentions that multiple definitions are possible

## Reference
See specification/treesitter.md - Definition Response example