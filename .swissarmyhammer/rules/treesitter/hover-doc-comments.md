---
severity: error
tags:
- operations
- hover
---

# Tree-sitter: Hover Doc Comment Extraction

## Acceptance Criterion
**AC-11**: `hover` operation extracts doc comments adjacent to definitions

## What to Check
The hover operation must:
- Find definition of symbol at given position
- Query for doc comment nodes adjacent to definition
- Extract and return doc comment text
- Handle cases where no doc comments exist

## Success Criteria
- Extracts doc comments immediately preceding definitions
- Returns empty/null documentation when none exists
- Does not return regular comments (only doc comments)
- Handles multi-line doc comments correctly

## Reference
See specification/treesitter.md - hover operation section