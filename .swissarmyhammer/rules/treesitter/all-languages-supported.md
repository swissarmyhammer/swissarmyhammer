---
severity: error
tags:
- languages
- parsers
---

# Tree-sitter: 25 Languages Supported

## Acceptance Criterion
**AC-3**: All 25 languages from specification table are supported with compiled-in parsers

## What to Check
All these languages must be supported with compiled-in parsers:
- Rust, TypeScript, JavaScript, Python, Go, Java
- C, C++, C#, Ruby, PHP, Swift, Kotlin, Scala
- Lua, Elixir, Haskell, OCaml, Zig, Bash
- HTML, CSS, JSON, YAML, TOML, Markdown, SQL

Each language must have:
- Parser crate dependency in Cargo.toml
- Parser initialization code
- Corresponding tags.scm query file
- File extension mapping

## Success Criteria
- All 25 parser crates included as dependencies
- Parser manager can initialize parser for each language
- Query files exist for all 25 languages
- Extension detection works for all listed extensions

## Reference
See specification/treesitter.md "Supported Languages" table