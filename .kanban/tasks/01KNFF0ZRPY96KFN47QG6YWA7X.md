---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe680
title: get_references returns error type inconsistently (no Result wrapper unlike other ops)
---
swissarmyhammer-code-context/src/ops/get_references.rs\n\nThe `get_references` function signature returns `ReferencesResult` directly (not `Result<ReferencesResult, CodeContextError>`):\n\n```rust\npub fn get_references(ctx: &LayeredContext, options: &GetReferencesOptions) -> ReferencesResult {\n```\n\nAll other new ops return `Result<T, CodeContextError>`. This means:\n1. LSP errors in the live layer are silently swallowed via `.ok()??` instead of propagated\n2. The MCP handler (when added) will need special-casing since it can't use `map_err(context_err)?`\n3. The function can never report a real error, only return empty results\n\nSuggestion: Change the signature to `Result<ReferencesResult, CodeContextError>` to match the pattern in all other ops." #review-finding