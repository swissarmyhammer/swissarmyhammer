---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffec80
title: MCP handlers create LayeredContext with None LSP client, defeating live LSP layer
---
swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs\n\nThe three new MCP handlers (execute_get_rename_edits, execute_get_diagnostics, execute_get_inbound_calls, execute_workspace_symbol_live) all create LayeredContext with `None` for the LSP client:\n\n```rust\nlet ctx = LayeredContext::new(&db, None);\n```\n\nThis means the live LSP layer is never used. The operations will always fall back to index-only results (or return empty for LSP-only ops like get_rename_edits and get_diagnostics).\n\nThe workspace has a shared LSP client available via `ws.lsp_client()` or the global LSP_SUPERVISOR. The existing get_symbol/search_symbol handlers don't use LayeredContext (they use the old API), so this is a new gap specific to the new ops.\n\nSuggestion: Thread the SharedLspClient from the workspace or LSP supervisor into the LayeredContext constructor. This is the key architectural gap -- without it, 6 of the 10 new ops (type_definition, hover, rename_edits, diagnostics, code_actions, inbound_calls) will never produce live results." #review-finding