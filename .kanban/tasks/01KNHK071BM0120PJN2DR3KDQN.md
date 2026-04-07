---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff580
title: 'Coverage: ts_callers_of in layered_context.rs'
---
crates/code-context/src/layered_context.rs

Coverage: 0% (0/28 lines)

Test the tree-sitter caller traversal logic. This is a pure index test — populate the ts_chunks and lsp_call_edges tables with known data, then verify ts_callers_of returns the correct caller symbols with source locations. Cover cases: single caller, multiple callers, no callers found, and max_depth recursion. #coverage-gap