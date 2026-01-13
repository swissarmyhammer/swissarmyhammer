---
severity: error
tags:
- architecture
- crate
---

# Tree-sitter Crate Structure

The `swissarmyhammer-treesitter` crate must be created with the following structure:

- Must be added to workspace members in Cargo.toml
- Must have Cargo.toml with tree-sitter dependencies
- Must have src/lib.rs with public API
- Must include modules: bridge.rs, parser.rs, query.rs, cache.rs, language.rs
- Must include operations/ subdirectory with: definition.rs, references.rs, hover.rs, parse.rs
- Must include queries/ subdirectory with language-specific .scm files

Reference: specification/treesitter.md