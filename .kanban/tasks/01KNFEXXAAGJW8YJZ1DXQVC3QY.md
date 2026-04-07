---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe580
title: get_references didClose is sent after response parsing, not guaranteed on error path
---
swissarmyhammer-code-context/src/ops/get_references.rs, try_live_lsp function\n\nThe didClose is sent after the response is received and parsed, but if the response parsing returns None (empty locations), the function returns without sending didClose:\n\n```rust\nlet response = ctx.lsp_request(\"textDocument/references\", params).ok()??;\n// didClose is here -- but if the line above returns None via ??, we skip it\nlet _ = ctx.lsp_notify(\"textDocument/didClose\", ...);\nlet locations = parse_lsp_locations(&response);\nif locations.is_empty() {\n    return None;  // didClose was sent, this path is OK\n}\n```\n\nThe `ok()??` on the response unwraps both the Result and the Option. If `lsp_request` returns `Ok(None)` (no live client), the `??` returns None early, skipping didClose. This leaks a document in the LSP server's open documents list.\n\nContrast with get_hover.rs, get_definition.rs etc. which send didClose before inspecting the response, or use a `close()` closure like get_rename_edits.rs.\n\nSuggestion: Move the didClose call before the response parsing, or wrap in an RAII guard / closure pattern as used in get_rename_edits." #review-finding