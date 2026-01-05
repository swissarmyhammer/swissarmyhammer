---
severity: error
tags:
- architecture
- treesitter
---

# Tree-sitter Crate Structure

The `swissarmyhammer-treesitter` crate must exist with the following structure:

- `Cargo.toml` with tree-sitter dependencies
- `src/lib.rs` - Public API
- `src/bridge.rs` - TreeSitterBridge core implementation
- `src/parser.rs` - Parser management and caching
- `src/query.rs` - Query loading and execution
- `src/cache.rs` - File cache with MD5 invalidation
- `src/language.rs` - Language detection (reuse from swissarmyhammer-rules)
- `src/operations/mod.rs` - Operations module
- `src/operations/definition.rs` - Definition lookup operation
- `src/operations/references.rs` - References lookup operation
- `src/operations/hover.rs` - Hover info operation
- `src/operations/parse.rs` - Parse operation
- `src/queries/rust.scm` - Rust tags query
- `src/queries/typescript.scm` - TypeScript tags query
- `src/queries/javascript.scm` - JavaScript tags query
- `src/queries/python.scm` - Python tags query

Reference: specification/complete/treesitter.md