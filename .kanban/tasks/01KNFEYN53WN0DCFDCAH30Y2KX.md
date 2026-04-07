---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffea80
title: get_implementations prefers targetRange over targetSelectionRange (inverted from get_definition)
---
swissarmyhammer-code-context/src/ops/get_implementations.rs, try_parse_location_link function\n\nThe LocationLink parser in get_implementations.rs prefers `targetRange` over `targetSelectionRange`:\n\n```rust\nlet range = value\n    .get(\"targetRange\")\n    .or_else(|| value.get(\"targetSelectionRange\"))?;\n```\n\nBut in get_definition.rs, the preference is correctly reversed (targetSelectionRange first, which is the more precise identifier range):\n\n```rust\nlet range = value\n    .get(\"targetSelectionRange\")\n    .and_then(parse_lsp_range)\n    .or_else(|| value.get(\"targetRange\").and_then(parse_lsp_range))?;\n```\n\nPer the LSP spec, `targetSelectionRange` is the precise identifier range (e.g., just the function name), while `targetRange` is the full body. Using targetRange makes the result imprecise.\n\nSuggestion: Reverse the preference to match get_definition.rs: try targetSelectionRange first, fall back to targetRange." #review-finding