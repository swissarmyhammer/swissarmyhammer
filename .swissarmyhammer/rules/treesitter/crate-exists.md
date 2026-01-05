---
severity: error
tags:
- treesitter
- crate
- infrastructure
---

The swissarmyhammer-treesitter crate must exist and be properly configured:

- Crate directory must exist at swissarmyhammer-treesitter/
- Cargo.toml must exist with tree-sitter dependencies
- Crate must be added to workspace members in root Cargo.toml
- src/lib.rs must exist with basic public API structure
- All required source files must exist (bridge.rs, parser.rs, query.rs, cache.rs, language.rs)
- operations/ module must exist with all operation files (definition.rs, references.rs, hover.rs, parse.rs)
- queries/ directory must exist with language-specific query files

Reference: specification/complete/treesitter.md