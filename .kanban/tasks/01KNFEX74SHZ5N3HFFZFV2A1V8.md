---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffbe80
title: get_references hardcodes languageId to 'rust' in didOpen
---
swissarmyhammer-code-context/src/ops/get_references.rs, try_live_lsp function\n\nThe didOpen notification hardcodes `languageId: \"rust\"` and sends empty text:\n\n```rust\nctx.lsp_notify(\n    \"textDocument/didOpen\",\n    serde_json::json!({\n        \"textDocument\": {\n            \"uri\": &uri,\n            \"languageId\": \"rust\",   // hardcoded\n            \"version\": 1,\n            \"text\": \"\"              // empty, not actual file content\n        }\n    }),\n);\n```\n\nAll other ops use `language_id_from_path()` to detect the language and `std::fs::read_to_string()` for the file content. This means get_references will fail or produce wrong results for non-Rust files, and may also fail for Rust files because some LSP servers require the actual file content in didOpen to provide accurate results.\n\nSuggestion: Use `language_id_from_path(&options.file_path)` and `std::fs::read_to_string(&options.file_path).unwrap_or_default()` as the other ops do." #review-finding