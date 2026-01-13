---
severity: error
tags:
- operations
- references
---

# Tree-sitter: References Operation Name Matching

## Acceptance Criterion
**AC-9**: `references` operation finds all name-matching references in specified scope

## What to Check
The references operation must:
- Extract symbol name at the given line:column position
- Search for ALL references/calls with matching name
- Use name-based matching (syntactic, not semantic)
- Return all matches found in the specified scope

## Success Criteria
- Finds all function calls and references with matching name
- Does not require semantic type resolution
- Works with file/directory/project scopes
- Returns all matches, not just semantically valid ones

## Reference
See specification/treesitter.md - references operation section