---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffcd80
title: Add tests for BraveSearchClient::search and HTML parsing edge cases
---
src/search/brave.rs:64-97, 100-224\n\nCoverage: 44.7% (34/76 lines)\n\nUncovered lines: 64-96 (search method - requires HTTP), 123, 153-164, 174, 197-200, 222, 230-231, 240-241\n\nparse_html_results has untested branches:\n1. Title fallback when no .title class found (lines 153-164) — HTML with <a href> but no .title span\n2. Description fallback to <p> elements (lines 197-200)\n3. Exponential decay scoring (line 230) — test with exponential_decay=true\n4. Default impl (lines 240-241)\n5. max_results limiting (line 123)\n\nThe search() method itself requires HTTP mocking — note for future. #coverage-gap