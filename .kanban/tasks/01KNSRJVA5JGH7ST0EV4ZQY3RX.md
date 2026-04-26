---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffce80
title: Add tests for get_hover live LSP path and parse_hover_contents edge cases
---
ops/get_hover.rs\n\nCoverage: 67.9% (36/53 lines)\n\nUncovered lines: 68, 94, 98, 100-101, 103-105, 109-111, 116, 122-124, 223, 228\n\nTwo areas:\n1. `try_live_lsp` (lines 94-133) - sends textDocument/hover, parses contents and range, enriches\n2. `parse_hover_contents` edge cases (lines 223, 228) - empty-language MarkedString + unrecognized array items\n\nTest scenarios:\n- Mock LSP returning hover with contents → verify SourceLayer::LiveLsp, contents correct\n- Null response → falls through\n- parse_hover_contents with {language: \"\", value: \"code\"} → no backtick wrapping\n- parse_hover_contents with unrecognized item shapes in array → items skipped\n\n#coverage-gap #coverage-gap