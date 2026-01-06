---
severity: error
tags:
- operations
- hover
- doc-comments
---

# Tree-sitter: Hover Language-Specific Doc Comments

## Acceptance Criterion
**AC-13**: Supports language-specific doc comments (Rust `///`, JSDoc `/** */`, Python `"""`, etc.)

## What to Check
Doc comment extraction must support language-specific formats:
- Rust: `///`, `//!`, `/** */`, `/*! */`
- TypeScript/JavaScript: JSDoc `/** */`
- Python: `"""docstrings"""`
- Java: Javadoc `/** */`
- C/C++: Doxygen `/** */`, `///`
- C#: XML docs `///`
- And others per specification table

## Success Criteria
- Correct doc comment format recognized for each language
- Regular comments not extracted as docs
- Multi-line doc comments handled correctly
- Language-specific formatting preserved

## Reference
See specification/treesitter.md - Supported Languages table, doc comments column