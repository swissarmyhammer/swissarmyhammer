---
severity: error
tags:
- operations
- definition
---

# Tree-sitter: Definition Operation Name Matching

## Acceptance Criterion
**AC-5**: `definition` operation finds all name-matching definitions in specified scope

## What to Check
The definition operation must:
- Extract symbol name at the given line:column position
- Search for ALL definitions with matching name (not just one)
- Use name-based matching (syntactic, not semantic)
- Return all matches found in the specified scope

## Success Criteria
- Finds all function/class/method definitions with matching name
- Does not require semantic type resolution
- Returns multiple definitions if name appears in multiple places
- Response includes note about name-based matching

## Reference
See specification/treesitter.md - definition operation section