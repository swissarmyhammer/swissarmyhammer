---
severity: error
tags:
- integration
- multi-language
---

# Tree-sitter: Mixed Language Support

## Acceptance Criterion
**AC-30**: Mixed language projects fully supported (parses all recognized extensions)

## What to Check
For projects with multiple languages:
- File discovery globs for ALL supported extensions
- Each file parsed with its appropriate language parser
- No project type detection required (no Cargo.toml/package.json sniffing)
- Results include symbols from all languages

## Success Criteria
- Directory/project scope searches for all 25+ language extensions
- Each file auto-detects language from extension
- Rust, TypeScript, Python, etc. files all parsed in same project
- No assumptions about primary project language
- Test with actual mixed-language codebase

## Reference
See specification/treesitter.md - File discovery model and design decisions