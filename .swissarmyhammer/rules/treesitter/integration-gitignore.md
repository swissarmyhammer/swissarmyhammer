---
severity: error
tags:
- integration
- file-discovery
---

# Tree-sitter: Gitignore Support

## Acceptance Criterion
**AC-29**: Respects `.gitignore` patterns during file discovery

## What to Check
File discovery for directory/project scope must:
- Read and parse .gitignore files in project
- Exclude files matching .gitignore patterns
- Not parse or return results from ignored files
- Use ignore crate or equivalent for .gitignore support

## Success Criteria
- Files matching .gitignore patterns excluded from results
- Standard .gitignore patterns supported (wildcards, negation, etc.)
- Tests verify ignored files are not included
- Works with nested .gitignore files

## Reference
See specification/treesitter.md - File discovery section