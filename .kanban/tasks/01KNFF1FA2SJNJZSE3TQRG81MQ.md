---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffee80
title: symbol_kind_int_to_string allocates a new String on every call
---
swissarmyhammer-code-context/src/layered_context.rs, symbol_kind_int_to_string function\n\nThe function returns `String` by calling `.to_string()` on a static `&str`:\n\n```rust\npub(crate) fn symbol_kind_int_to_string(kind: i32) -> String {\n    match kind {\n        1 => \"file\",\n        ...\n    }\n    .to_string()\n}\n```\n\nThis is called for every symbol in every query result (lsp_symbol_at, lsp_symbols_in_file, lsp_symbols_by_name, ts_symbols_in_file, etc.). For large result sets, this creates many small allocations.\n\nSuggestion: Return `&'static str` instead of `String`, and let callers that need String do the conversion. Or use `Cow<'static, str>` if the unknown case needs dynamic formatting. The SymbolInfo.kind field would change to `Cow<'static, str>` or the callers would `.to_string()` at the boundary." #review-finding