---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9d80
title: '[warning] Unused dependencies in Cargo.toml'
---
**File**: code-context-cli/Cargo.toml\n\n**What**: Several dependencies are listed but never imported or used directly in any source file under `code-context-cli/src/`:\n- `async-trait` -- no `use async_trait` or `#[async_trait]` anywhere\n- `dirs` -- no `dirs::` usage\n- `swissarmyhammer-lsp` -- no direct import\n- `swissarmyhammer-project-detection` -- no direct import\n- `tracing` -- only `tracing-subscriber` is used; `tracing` itself is a transitive dep\n\n**Why**: Unnecessary dependencies increase compile time and binary size. They also create confusing signals for anyone reading the manifest to understand what the crate actually uses.\n\n**Suggestion**: Remove unused dependencies. If they are needed as transitive deps for linking, add a comment explaining why.\n\n**Verify**: `cargo check -p code-context-cli` after removing each dep." #review-finding