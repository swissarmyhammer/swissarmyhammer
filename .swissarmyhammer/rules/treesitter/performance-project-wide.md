---
severity: error
tags:
- performance
- benchmarks
---

# Tree-sitter: Project-Wide Performance

## Acceptance Criterion
**AC-18**: Project-wide definition search completes in < 500ms

## What to Check
Definition lookup with project scope must:
- Complete in under 500 milliseconds
- Include file discovery, parsing multiple files, and matching
- Meet performance target on typical projects (100-500 files)

## Success Criteria
- Benchmark tests confirm < 500ms for project-wide searches
- Performance measured on representative codebases
- Caching properly utilized to improve subsequent searches

## Reference
See specification/treesitter.md - Success Criteria section